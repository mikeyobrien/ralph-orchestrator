/**
 * Empirical seam for docs/design/v3-on-icedrop.md.
 *
 * Hat policy speaks only the icedrop command union (submit, configure, abort, note).
 * Storage stays inside one host. No events.jsonl, no session.json, no job table.
 *
 * Run from this directory: npm install && npm run seam
 */
import { mkdtemp, rm } from "node:fs/promises";
import { tmpdir } from "node:os";
import { join } from "node:path";
import { BACKGROUND_CONTEXT } from "@earendil-works/chord/context";
import { Type } from "@earendil-works/pi-ai";
import { createModels } from "@earendil-works/pi-ai/models";
import { fauxAssistantMessage, fauxProvider, fauxText, fauxToolCall } from "@earendil-works/pi-ai/providers/faux";
import {
	InboxDoc,
	MemoryStorage,
	SystemEntry,
	createRegistry,
	defineExtension,
	defineTool,
	Harness,
	section,
} from "@earendil-works/pi-durable";
import { openNodeSqliteStorage } from "@earendil-works/pi-durable/storage/sqlite/node";

const context = BACKGROUND_CONTEXT;
const evidence = [];
function record(name, value) {
	evidence.push({ name, value });
}

const CATALOG = [
	"planner: triggers plan.needed; publishes plan.ready",
	"builder: triggers plan.ready; publishes build.done",
].join("\n");

const HATS = {
	planner: { triggers: ["run.start"], instructions: "You plan. Emit plan.ready when the plan exists." },
	builder: { triggers: ["plan.ready"], instructions: "You build the plan named in the wake. Do not switch hats yourself." },
};

/** Pure policy. No storage, no transcript, no queue. */
function route(lastTopic) {
	const hat = Object.entries(HATS).find(([, spec]) => spec.triggers.includes(lastTopic));
	if (hat === undefined) return null;
	const [id, spec] = hat;
	return {
		hat: id,
		commands: [
			{ type: "configure", change: { instructions: spec.instructions } },
			{
				type: "submit",
				text: `Hat: ${id}\nEvent: ${lastTopic}\nYou may publish through emit.`,
				requestId: `wake:${lastTopic}`,
			},
		],
	};
}

const emits = [];
const emitTool = defineTool({
	name: "emit",
	description: "Publish one workflow topic.",
	parameters: Type.Object({ topic: Type.String(), payload: Type.String() }),
	execute: async (args) => {
		emits.push({ topic: args.topic, payload: args.payload });
		return { output: [{ type: "text", text: `emitted ${args.topic}` }] };
	},
});

function registry() {
	const reg = createRegistry();
	reg.install(
		defineExtension({
			name: "ralph",
			sections: [section("catalog", () => CATALOG, { tag: false })],
			tools: [emitTool],
		}),
	);
	return reg;
}

function modelsFor(faux) {
	const models = createModels();
	models.setProvider(faux.provider);
	return models;
}

const model = { provider: "faux", modelId: "faux-1" };

async function systemPatches(conversation) {
	const page = await conversation.entries({}, 50, undefined, context);
	return [...page.items].reverse().flatMap((entry) => {
		if (!SystemEntry.is(entry)) return [];
		const message = entry.model?.[0];
		return [{ id: entry.id, sections: message?.sections ?? null, content: message?.content ?? null }];
	});
}

/** The only way a client touches the conversation. Mirrors packages/protocol Command. */
async function handle(conversation, command) {
	switch (command.type) {
		case "submit":
			return conversation.submit(
				{
					type: "input",
					content: command.text,
					requestId: command.requestId,
					whenBusy: command.whenBusy,
				},
				context,
			);
		case "configure":
			await conversation.configure(command.change, context);
			return { id: "configure", configured: command.change };
		case "abort":
			await conversation.abort(context);
			return { id: "abort" };
		case "note":
			return conversation.submit({ type: "write", entry: { kind: command.kind, data: command.data } }, context);
		default:
			throw new Error(`unknown command ${command.type}`);
	}
}

async function hatSwitch() {
	const faux = fauxProvider();
	faux.setResponses([
		fauxAssistantMessage([fauxToolCall("emit", { topic: "plan.ready", payload: "ship it" }, { id: "c1" })], {
			stopReason: "toolUse",
		}),
		fauxAssistantMessage([fauxText("plan is ready")]),
		fauxAssistantMessage([fauxText("built")]),
		fauxAssistantMessage([fauxText("checked")]),
		fauxAssistantMessage([fauxText("still the builder")]),
	]);
	const harness = await Harness.open(new MemoryStorage(), { models: modelsFor(faux), registry: registry() }, context);
	const root = await harness.root(context, { agent: { model } });

	const goal = await handle(root, { type: "submit", text: "Ship the header.", requestId: "goal:1" });
	const goalSettled = await goal.wait(context);
	record("goal.status", goalSettled.status);
	record("emits.afterGoal", emits.slice());
	const before = await systemPatches(root);
	record("system.afterGoal", before);

	const routed = route(emits.at(-1)?.topic);
	record("route", routed);
	for (const command of routed.commands) await handle(root, command).then(async (submission) => {
		if (submission?.wait) await submission.wait(context);
	});
	const afterSwitch = await systemPatches(root);
	record("system.afterHatSwitch", afterSwitch);

	const again = await handle(root, {
		type: "submit",
		text: "Same hat, new event build.done.",
		requestId: "wake:build.done",
	});
	await again.wait(context);
	const afterSame = await systemPatches(root);
	record("system.afterSameHat", afterSame);
	record(
		"hatSwitchDeltaKeys",
		afterSwitch.slice(before.length).map((patch) => Object.keys(patch.sections ?? {})),
	);
	record("sameHatAddedSystemEntries", afterSame.length - afterSwitch.length);

	await handle(root, { type: "configure", change: { instructions: HATS.builder.instructions } });
	const resubmit = await handle(root, {
		type: "submit",
		text: "Configure repeated the same instructions.",
		requestId: "wake:same-instructions",
	});
	await resubmit.wait(context);
	const afterRepeatConfigure = await systemPatches(root);
	record("repeatConfigureAddedSystemEntries", afterRepeatConfigure.length - afterSame.length);

	const wakeAgain = await handle(root, {
		type: "submit",
		text: "DIFFERENT TEXT",
		requestId: "wake:plan.ready",
	});
	record("requestId.replayIgnoresNewText", { replay: wakeAgain.id });

	await harness.close(context);
}

async function requestIdAndSteer() {
	let release;
	const held = new Promise((resolve) => {
		release = resolve;
	});
	const faux = fauxProvider();
	faux.setResponses([
		async () => {
			await held;
			return fauxAssistantMessage([fauxText("first answer")]);
		},
		fauxAssistantMessage([fauxText("after steer")]),
	]);
	const harness = await Harness.open(new MemoryStorage(), { models: modelsFor(faux), registry: createRegistry() }, context);
	const root = await harness.root(context, { agent: { model } });
	const first = await handle(root, { type: "submit", text: "Hold this.", requestId: "hold:1" });
	const steer = await handle(root, { type: "submit", text: "Steer me.", requestId: "steer:1", whenBusy: "steer" });
	const inbox = await harness.snapshot(InboxDoc, root.id, context);
	record(
		"inbox.whileBusy",
		inbox?.items.map((item) => ({ id: item.id, mode: item.mode })),
	);
	let rejected = null;
	try {
		await handle(root, { type: "submit", text: "no", requestId: "reject:1", whenBusy: "reject" });
	} catch (error) {
		rejected = error.constructor?.name ?? error.message;
	}
	record("whenBusy.reject", rejected);
	release();
	record("hold.status", (await first.wait(context)).status);
	record("steer.status", (await steer.wait(context)).status);
	const replay = await handle(root, { type: "submit", text: "OTHER", requestId: "hold:1" });
	record("requestId.dedupesSettled", { first: first.id, replay: replay.id, equal: first.id === replay.id });
	await harness.close(context);
}

async function sqliteResume() {
	const dir = await mkdtemp(join(tmpdir(), "ralph-hat-"));
	const path = join(dir, "harness.sqlite");
	const faux = fauxProvider();
	faux.setResponses([fauxAssistantMessage([fauxText("persisted")])]);
	const opened = await Harness.open(await openNodeSqliteStorage(path), { models: modelsFor(faux), registry: createRegistry() }, context);
	const root = await opened.root(context, { agent: { model } });
	const submission = await handle(root, { type: "submit", text: "Remember this.", requestId: "greeting-1" });
	const settled = await submission.wait(context);
	const before = await root.entries({}, 20, undefined, context);
	const beforeKinds = [...before.items].reverse().map((entry) => entry.kind);
	await opened.close(context);

	const reopened = await Harness.open(await openNodeSqliteStorage(path), { models: modelsFor(faux), registry: createRegistry() }, context);
	reopened.resume();
	const again = await reopened.root(context);
	const replay = await handle(again, { type: "submit", text: "Remember this.", requestId: "greeting-1" });
	const after = await again.entries({}, 20, undefined, context);
	const afterKinds = [...after.items].reverse().map((entry) => entry.kind);
	record("sqlite.resume", {
		path,
		firstStatus: settled.status,
		sameSubmission: submission.id === replay.id,
		kindsBefore: beforeKinds,
		kindsAfter: afterKinds,
		entryCountUnchanged: beforeKinds.length === afterKinds.length,
	});
	await reopened.close(context);
	await rm(dir, { recursive: true, force: true });
}

const summary = { ok: false };
try {
	await hatSwitch();
	await requestIdAndSteer();
	await sqliteResume();
	const byName = Object.fromEntries(evidence.map((item) => [item.name, item.value]));
	const failures = [];
	if (JSON.stringify(byName.hatSwitchDeltaKeys) !== JSON.stringify([["instructions"]])) {
		failures.push("hat switch was not a one-key instructions delta");
	}
	if (byName.sameHatAddedSystemEntries !== 0) failures.push("same-hat wake appended a system entry");
	if (byName.repeatConfigureAddedSystemEntries !== 0) failures.push("repeating the same instructions appended a system entry");
	if (byName["inbox.whileBusy"]?.[0]?.mode !== "steer") failures.push("busy submit did not queue as steer");
	if (byName["whenBusy.reject"] !== "ConversationBusy") failures.push("reject did not throw ConversationBusy");
	if (byName["requestId.dedupesSettled"]?.equal !== true) failures.push("requestId did not return the settled submission");
	if (byName["sqlite.resume"]?.sameSubmission !== true || byName["sqlite.resume"]?.entryCountUnchanged !== true) {
		failures.push("sqlite reopen did not dedupe greeting-1");
	}
	const firstCatalog = byName["system.afterHatSwitch"]?.[0]?.sections?.catalog;
	const switched = byName["system.afterHatSwitch"]?.[1]?.sections;
	if (!String(firstCatalog).includes("planner:") || switched?.catalog !== undefined) {
		failures.push("catalog was not a stable prefix");
	}
	if (failures.length > 0) throw new Error(failures.join("; "));
	summary.ok = true;
} catch (error) {
	summary.ok = false;
	summary.error = error?.stack ?? String(error);
}
summary.evidence = evidence;
console.log(JSON.stringify(summary, null, 2));
if (!summary.ok) process.exit(1);
