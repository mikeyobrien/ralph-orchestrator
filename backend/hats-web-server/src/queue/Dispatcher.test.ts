import { describe, it } from "node:test";
import assert from "node:assert/strict";
import { Dispatcher } from "./Dispatcher";
import { TaskQueueService } from "./TaskQueueService";
import { EventBus } from "./EventBus";

describe("Dispatcher", () => {
  it("should have a default task timeout of 2 hours", () => {
    const queue = new TaskQueueService();
    const eventBus = new EventBus();
    const dispatcher = new Dispatcher(queue, eventBus);

    let capturedConfig: any;
    eventBus.subscribe("dispatcher.started", (event) => {
      capturedConfig = (event.payload as any).config;
    });

    dispatcher.start();
    dispatcher.stop();

    assert.ok(capturedConfig, "dispatcher.started event should be published");
    assert.equal(
      capturedConfig.taskTimeoutMs,
      7200000,
      "Task timeout should be 2 hours (7200000ms)"
    );
  });

  it("should cancel a pending task", async () => {
    const queue = new TaskQueueService();
    const eventBus = new EventBus();
    const dispatcher = new Dispatcher(queue, eventBus);

    const task = queue.enqueue({ taskType: "test.task" });

    let cancelledEvent: any;
    eventBus.subscribe("task.cancelled", (event) => {
      cancelledEvent = event;
    });

    const result = await dispatcher.cancelTask(task.id);

    assert.equal(result, true, "cancelTask should return true");
    assert.equal(task.state, "CANCELLED", "Task state should be CANCELLED");
    assert.ok(cancelledEvent, "task.cancelled event should be published");
    assert.equal((cancelledEvent.payload as any).taskId, task.id);
  });

  it("should cancel a running task", async () => {
    const queue = new TaskQueueService();
    const eventBus = new EventBus();
    const dispatcher = new Dispatcher(queue, eventBus, { pollIntervalMs: 10 });

    let resolveHandler: () => void;
    const handlerPromise = new Promise<void>((resolve) => {
      resolveHandler = resolve;
    });

    dispatcher.registerHandler("test.long_task", async (task, context) => {
      // Wait indefinitely until cancelled or manually resolved
      await new Promise<void>((resolve, reject) => {
        if (context.signal.aborted) {
          reject(context.signal.reason);
          return;
        }
        context.signal.addEventListener("abort", () => {
          reject(context.signal.reason);
        });
        // Keep checking periodically if needed, or just wait
        setTimeout(resolve, 10000);
      });
    });

    const task = queue.enqueue({ taskType: "test.long_task" });

    let startedEvent: any;
    eventBus.subscribe("task.started", (event) => {
      if ((event.payload as any).taskId === task.id) {
        startedEvent = event;
      }
    });

    let cancelledEvent: any;
    eventBus.subscribe("task.cancelled", (event) => {
      if ((event.payload as any).taskId === task.id) {
        cancelledEvent = event;
      }
    });

    dispatcher.start();

    // Wait for task to start
    while (!startedEvent) {
      await new Promise((resolve) => setTimeout(resolve, 10));
    }

    const result = await dispatcher.cancelTask(task.id);

    assert.equal(result, true, "cancelTask should return true");

    // Wait for cancellation to process
    while (!cancelledEvent) {
      await new Promise((resolve) => setTimeout(resolve, 10));
    }

    const updatedTask = queue.getTask(task.id);
    assert.equal(updatedTask?.state, "CANCELLED", "Task state should be CANCELLED");
    assert.equal((cancelledEvent.payload as any).reason, "cancelled by user");

    await dispatcher.stop();
  });
});
