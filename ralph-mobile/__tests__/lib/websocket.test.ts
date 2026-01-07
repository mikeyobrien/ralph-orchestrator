import { WebSocketClient, WebSocketEvent } from "../../lib/websocket";

describe("WebSocketClient", () => {
  let client: WebSocketClient;

  beforeEach(() => {
    jest.useFakeTimers();
  });

  afterEach(() => {
    if (client) {
      client.disconnect();
    }
    jest.useRealTimers();
  });

  describe("connection", () => {
    test("connects to WebSocket server with correct URL", () => {
      client = new WebSocketClient("ws://localhost:8000");

      expect(client.getUrl()).toBe("ws://localhost:8000/ws");
    });

    test("reports connecting state initially", () => {
      client = new WebSocketClient("ws://localhost:8000");
      client.connect();

      expect(client.isConnecting()).toBe(true);
      expect(client.isConnected()).toBe(false);
    });

    test("reports connected state after successful connection", async () => {
      client = new WebSocketClient("ws://localhost:8000");
      client.connect();

      // Fast-forward past connection delay
      jest.advanceTimersByTime(20);

      expect(client.isConnected()).toBe(true);
      expect(client.isConnecting()).toBe(false);
    });

    test("emits connected event when connection opens", async () => {
      client = new WebSocketClient("ws://localhost:8000");
      const onConnected = jest.fn();
      client.on("connected", onConnected);

      client.connect();
      jest.advanceTimersByTime(20);

      expect(onConnected).toHaveBeenCalled();
    });
  });

  describe("subscribing to sessions", () => {
    test("sends subscribe message for session", () => {
      client = new WebSocketClient("ws://localhost:8000");
      client.connect();
      jest.advanceTimersByTime(20);

      const sendSpy = jest.spyOn(client["ws"]!, "send");
      client.subscribeToSession("session-123");

      expect(sendSpy).toHaveBeenCalledWith(
        JSON.stringify({
          type: "subscribe",
          sessionId: "session-123",
        })
      );
    });

    test("sends unsubscribe message for session", () => {
      client = new WebSocketClient("ws://localhost:8000");
      client.connect();
      jest.advanceTimersByTime(20);

      const sendSpy = jest.spyOn(client["ws"]!, "send");
      client.unsubscribeFromSession("session-123");

      expect(sendSpy).toHaveBeenCalledWith(
        JSON.stringify({
          type: "unsubscribe",
          sessionId: "session-123",
        })
      );
    });
  });

  describe("receiving messages", () => {
    test("emits session_update event when receiving status update", () => {
      client = new WebSocketClient("ws://localhost:8000");
      const onSessionUpdate = jest.fn();
      client.on("session_update", onSessionUpdate);

      client.connect();
      jest.advanceTimersByTime(20);

      // Simulate incoming message
      const mockWs = client["ws"] as any;
      mockWs._simulateMessage({
        type: "session_update",
        sessionId: "session-123",
        status: "running",
        iteration: 5,
      });

      expect(onSessionUpdate).toHaveBeenCalledWith({
        type: "session_update",
        sessionId: "session-123",
        status: "running",
        iteration: 5,
      });
    });

    test("emits iteration_complete event when iteration finishes", () => {
      client = new WebSocketClient("ws://localhost:8000");
      const onIterationComplete = jest.fn();
      client.on("iteration_complete", onIterationComplete);

      client.connect();
      jest.advanceTimersByTime(20);

      const mockWs = client["ws"] as any;
      mockWs._simulateMessage({
        type: "iteration_complete",
        sessionId: "session-123",
        iteration: 5,
        tokensUsed: 1500,
        cost: 0.0045,
      });

      expect(onIterationComplete).toHaveBeenCalledWith({
        type: "iteration_complete",
        sessionId: "session-123",
        iteration: 5,
        tokensUsed: 1500,
        cost: 0.0045,
      });
    });

    test("emits log event when receiving log message", () => {
      client = new WebSocketClient("ws://localhost:8000");
      const onLog = jest.fn();
      client.on("log", onLog);

      client.connect();
      jest.advanceTimersByTime(20);

      const mockWs = client["ws"] as any;
      mockWs._simulateMessage({
        type: "log",
        sessionId: "session-123",
        level: "info",
        message: "Starting iteration 5",
        timestamp: "2026-01-07T01:56:00Z",
      });

      expect(onLog).toHaveBeenCalledWith(
        expect.objectContaining({
          type: "log",
          message: "Starting iteration 5",
        })
      );
    });
  });

  describe("disconnection", () => {
    test("emits disconnected event when connection closes", () => {
      client = new WebSocketClient("ws://localhost:8000");
      const onDisconnected = jest.fn();
      client.on("disconnected", onDisconnected);

      client.connect();
      jest.advanceTimersByTime(20);
      client.disconnect();

      expect(onDisconnected).toHaveBeenCalled();
      expect(client.isConnected()).toBe(false);
    });

    test("attempts reconnection on unexpected close", () => {
      client = new WebSocketClient("ws://localhost:8000", {
        reconnect: true,
        reconnectInterval: 1000,
      });
      const onReconnecting = jest.fn();
      client.on("reconnecting", onReconnecting);

      client.connect();
      jest.advanceTimersByTime(20);

      // Simulate unexpected close
      const mockWs = client["ws"] as any;
      mockWs.readyState = WebSocket.CLOSED;
      mockWs.onclose?.({ type: "close", wasClean: false });

      // Should emit reconnecting
      expect(onReconnecting).toHaveBeenCalled();

      // Should reconnect after interval
      jest.advanceTimersByTime(1000);
      expect(client.isConnecting()).toBe(true);
    });
  });

  describe("error handling", () => {
    test("emits error event on WebSocket error", () => {
      client = new WebSocketClient("ws://localhost:8000");
      const onError = jest.fn();
      client.on("error", onError);

      client.connect();
      jest.advanceTimersByTime(20);

      const mockWs = client["ws"] as any;
      mockWs._simulateError(new Error("Connection lost"));

      expect(onError).toHaveBeenCalledWith(expect.any(Error));
    });
  });
});
