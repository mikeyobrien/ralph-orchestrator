#!/usr/bin/env node
// Long-lived Playwright session for the Ralph dashboard.
// `serve` owns the browser. Every other invocation sends one command over
// the run's unix socket and exits. The browser process stays up across commands
// so a journey can fill a form, submit it, and screenshot the result.

import fs from "node:fs";
import net from "node:net";
import path from "node:path";
import { createRequire } from "node:module";

const repoRoot = process.env.REPO_ROOT;
const stateDir = process.env.STATE_DIR;
const dashboardUrl = (process.env.DASHBOARD_URL || "").replace(/\/$/, "");

function loadPlaywright() {
  const anchors = [
    path.join(repoRoot, "package.json"),
    path.join(repoRoot, "frontend/ralph-web/package.json"),
  ];
  const errors = [];
  for (const anchor of anchors) {
    if (!fs.existsSync(anchor)) continue;
    try {
      return createRequire(anchor)("playwright");
    } catch (error) {
      errors.push(`${anchor}: ${error.message}`);
    }
  }
  throw new Error(
    `playwright is not installed for this repo.\n${errors.join("\n")}\nFrom the repo root: npx playwright install chromium`
  );
}

function socketPath() {
  if (!stateDir) {
    throw new Error("STATE_DIR is not set. Source the run.env printed by launch.");
  }
  return path.join(stateDir, "browser.sock");
}

function parseClientArgs(argv) {
  const op = argv[0];
  if (!op || op.startsWith("--")) {
    throw new Error(
      "usage: verify-ralph browser <goto|click|fill|press|select|text|absent|value|screenshot|snapshot|url|dialog|errors> [options]"
    );
  }
  const cmd = { op };
  for (let i = 1; i < argv.length; i += 1) {
    const flag = argv[i];
    if (!flag.startsWith("--")) {
      if (op === "goto" && cmd.path === undefined) {
        cmd.path = flag;
        continue;
      }
      throw new Error(`unexpected argument ${flag}`);
    }
    const key = flag.slice(2).replace(/-([a-z])/g, (_, letter) => letter.toUpperCase());
    const value = argv[i + 1];
    if (value === undefined) {
      cmd[key] = true;
      continue;
    }
    if (value.startsWith("--")) {
      cmd[key] = true;
      continue;
    }
    cmd[key] = value;
    i += 1;
  }
  return cmd;
}

function sendCommand(cmd) {
  const sock = socketPath();
  if (!fs.existsSync(sock)) {
    console.error(`browser socket is missing (${sock}). Run launch, then doctor.`);
    process.exit(1);
  }
  const client = net.createConnection(sock);
  let buffer = "";
  client.setTimeout(90000);
  client.on("timeout", () => {
    console.error("browser command timed out");
    process.exit(1);
  });
  client.on("connect", () => {
    client.write(`${JSON.stringify(cmd)}\n`);
  });
  client.on("data", (chunk) => {
    buffer += chunk.toString();
    const newline = buffer.indexOf("\n");
    if (newline < 0) return;
    const payload = JSON.parse(buffer.slice(0, newline));
    client.end();
    appendCommandLog(cmd, payload);
    if (!payload.ok) {
      console.error(payload.error || "browser command failed");
      if (payload.url) console.error(`url: ${payload.url}`);
      if (payload.pageError) console.error(`page error: ${payload.pageError}`);
      if (payload.snapshot) console.error(payload.snapshot);
      process.exit(1);
    }
    if (payload.output) process.stdout.write(payload.output.endsWith("\n") ? payload.output : `${payload.output}\n`);
    else console.log("ok");
  });
  client.on("error", (error) => {
    console.error(`browser socket error: ${error.message}`);
    process.exit(1);
  });
}

function appendCommandLog(cmd, payload) {
  const evidenceDir = process.env.EVIDENCE_DIR;
  if (!evidenceDir) return;
  fs.mkdirSync(evidenceDir, { recursive: true });
  const line = `${new Date().toISOString()} ${cmd.op} ${JSON.stringify(cmd)} ${payload.ok ? "ok" : "fail"}\n`;
  fs.appendFileSync(path.join(evidenceDir, "commands.log"), line);
}

function locatorFor(page, cmd) {
  if (cmd.text) {
    return page.getByText(cmd.text, { exact: cmd.exact === true || cmd.exact === "true" });
  }
  if (cmd.placeholder) {
    return page.getByPlaceholder(cmd.placeholder, { exact: false });
  }
  if (cmd.label) {
    return page.getByLabel(cmd.label, { exact: false });
  }
  if (cmd.role) {
    const options = {};
    if (cmd.nameRegex) options.name = new RegExp(cmd.nameRegex);
    else if (cmd.name) options.name = cmd.name;
    return page.getByRole(cmd.role, options);
  }
  throw new Error("a locator needs --role and --name, --placeholder, --label, or --text");
}

async function failure(page, error, pageError) {
  let snapshot = "";
  let url = "";
  try {
    url = page.url();
  } catch {
    url = "";
  }
  try {
    snapshot = await page.locator("body").ariaSnapshot({ timeout: 2000 });
    if (snapshot.length > 4000) snapshot = `${snapshot.slice(0, 4000)}\n…`;
  } catch {
    snapshot = "";
  }
  return {
    ok: false,
    error: error instanceof Error ? error.stack || error.message : String(error),
    url,
    pageError: pageError || "",
    snapshot,
  };
}

async function serve() {
  if (!repoRoot || !stateDir || !dashboardUrl) {
    throw new Error("serve requires REPO_ROOT, STATE_DIR, and DASHBOARD_URL");
  }
  fs.mkdirSync(stateDir, { recursive: true });
  const { chromium } = loadPlaywright();
  const headed = process.env.VERIFY_RALPH_HEADED === "1";
  const browser = await chromium.launch({
    headless: !headed,
    args: ["--no-sandbox", "--disable-dev-shm-usage"],
  });
  const videoDir = path.join(stateDir, "video");
  fs.mkdirSync(videoDir, { recursive: true });
  const context = await browser.newContext({
    viewport: { width: 1440, height: 900 },
    recordVideo: { dir: videoDir, size: { width: 1440, height: 900 } },
  });
  const page = await context.newPage();
  page.setDefaultTimeout(15000);
  let pageError = "";
  let nextDialog = "dismiss";
  page.on("pageerror", (error) => {
    pageError = String(error && error.message ? error.message : error);
  });
  page.on("dialog", async (dialog) => {
    const action = nextDialog;
    nextDialog = "dismiss";
    if (action === "accept") await dialog.accept();
    else await dialog.dismiss();
  });

  let chain = Promise.resolve();
  function enqueue(work) {
    const run = chain.then(work, work);
    chain = run.then(
      () => {},
      () => {}
    );
    return run;
  }

  async function handle(cmd) {
    const timeout = Number(cmd.timeout || 10000);
    if (cmd.op === "dialog") {
      nextDialog = cmd.action === "accept" ? "accept" : "dismiss";
      return { ok: true, output: `dialog ${nextDialog}` };
    }
    if (cmd.op === "errors") {
      return { ok: true, output: pageError };
    }
    if (cmd.op === "url") {
      return { ok: true, output: page.url() };
    }
    if (cmd.op === "goto") {
      const target = cmd.path && cmd.path.startsWith("http")
        ? cmd.path
        : `${dashboardUrl}${cmd.path?.startsWith("/") ? cmd.path : `/${cmd.path || "/tasks"}`}`;
      pageError = "";
      await page.goto(target, { waitUntil: "domcontentloaded", timeout: 20000 });
      await page.waitForFunction(
        () => document.body && document.body.innerText.trim().length > 0,
        null,
        { timeout: 15000 }
      );
      return { ok: true, output: page.url() };
    }
    if (cmd.op === "screenshot") {
      if (!cmd.path) throw new Error("screenshot requires --path");
      fs.mkdirSync(path.dirname(cmd.path), { recursive: true });
      await page.screenshot({ path: cmd.path, fullPage: true });
      return { ok: true, output: cmd.path };
    }
    if (cmd.op === "snapshot") {
      if (!cmd.path) throw new Error("snapshot requires --path");
      fs.mkdirSync(path.dirname(cmd.path), { recursive: true });
      const yaml = await page.locator("body").ariaSnapshot({ timeout: 5000 });
      const body = `# url: ${page.url()}\n# title: ${await page.title()}\n${yaml}\n`;
      fs.writeFileSync(cmd.path, body);
      return { ok: true, output: cmd.path };
    }
    if (cmd.op === "press") {
      if (!cmd.key) throw new Error("press requires --key");
      if (cmd.role || cmd.placeholder || cmd.label) {
        await locatorFor(page, cmd).press(cmd.key);
      } else {
        await page.keyboard.press(cmd.key);
      }
      return { ok: true, output: cmd.key };
    }
    if (cmd.op === "text" || cmd.op === "absent") {
      if (!cmd.contains) throw new Error(`${cmd.op} requires --contains`);
      const found = page.getByText(cmd.contains, { exact: false }).first();
      try {
        await found.waitFor({ state: "visible", timeout });
        if (cmd.op === "absent") {
          throw new Error(`unexpected visible text: ${cmd.contains}`);
        }
        return { ok: true, output: cmd.contains };
      } catch (error) {
        if (cmd.op === "absent" && error && error.name === "TimeoutError") {
          return { ok: true, output: `absent ${cmd.contains}` };
        }
        throw error;
      }
    }
    const target = locatorFor(page, cmd);
    if (cmd.op === "click") {
      await target.click({ timeout });
      return { ok: true, output: "clicked" };
    }
    if (cmd.op === "fill") {
      let value = cmd.value ?? "";
      if (cmd.valueFile) value = fs.readFileSync(cmd.valueFile, "utf8");
      await target.fill(value, { timeout });
      return { ok: true, output: "filled" };
    }
    if (cmd.op === "select") {
      if (!cmd.value) throw new Error("select requires --value");
      await target.selectOption(cmd.value, { timeout });
      return { ok: true, output: cmd.value };
    }
    if (cmd.op === "value") {
      const value = await target.inputValue({ timeout });
      return { ok: true, output: value };
    }
    throw new Error(`unknown browser command ${cmd.op}`);
  }

  await page.goto(`${dashboardUrl}/tasks`, { waitUntil: "domcontentloaded", timeout: 20000 });
  await page.waitForFunction(
    () => document.body && document.body.innerText.includes("Tasks"),
    null,
    { timeout: 20000 }
  );

  const sock = socketPath();
  if (fs.existsSync(sock)) fs.unlinkSync(sock);
  let shuttingDown = false;
  const server = net.createServer((connection) => {
    let buffer = "";
    connection.on("data", (chunk) => {
      buffer += chunk.toString();
      let newline = buffer.indexOf("\n");
      while (newline >= 0) {
        const line = buffer.slice(0, newline);
        buffer = buffer.slice(newline + 1);
        newline = buffer.indexOf("\n");
        enqueue(async () => {
          let result;
          try {
            result = await handle(JSON.parse(line));
          } catch (error) {
            result = await failure(page, error, pageError);
          }
          if (!connection.destroyed) connection.write(`${JSON.stringify(result)}\n`);
        });
      }
    });
  });
  await new Promise((resolve, reject) => {
    server.listen(sock, () => resolve());
    server.on("error", reject);
  });
  fs.writeFileSync(path.join(stateDir, "browser.ready"), `${page.url()}\n`);
  console.log(`browser ready ${page.url()}`);

  const shutdown = async () => {
    if (shuttingDown) return;
    shuttingDown = true;
    const videoDir = path.join(stateDir, "video");
    let videoPath = "";
    try {
      const video = page.video();
      const saved = video ? video.path() : null;
      try {
        await page.close({ runBeforeUnload: false });
      } catch {
        // The page can already be gone after an overlay crash.
      }
      try {
        await context.close();
      } catch {
        // A crashed page can leave the context id stale. The webm is still on disk.
      }
      if (saved) {
        try {
          videoPath = await saved;
        } catch {
          videoPath = "";
        }
      }
    } catch (error) {
      fs.writeFileSync(path.join(stateDir, "video.error"), String(error));
    }
    if (!videoPath || !fs.existsSync(videoPath)) {
      if (fs.existsSync(videoDir)) {
        const found = fs.readdirSync(videoDir).find((name) => name.endsWith(".webm"));
        if (found) videoPath = path.join(videoDir, found);
      }
    }
    if (videoPath && fs.existsSync(videoPath)) {
      fs.writeFileSync(path.join(stateDir, "video.path"), `${videoPath}\n`);
      if (process.env.EVIDENCE_DIR) {
        fs.mkdirSync(process.env.EVIDENCE_DIR, { recursive: true });
        fs.copyFileSync(videoPath, path.join(process.env.EVIDENCE_DIR, "session.webm"));
      }
    } else if (!fs.existsSync(path.join(stateDir, "video.error"))) {
      fs.writeFileSync(path.join(stateDir, "video.error"), "no video file\n");
    }
    try {
      server.close();
    } catch {
      // already closing
    }
    try {
      await browser.close();
    } catch {
      // already closed
    }
    process.exit(0);
  };
  process.on("SIGTERM", () => {
    shutdown();
  });
  process.on("SIGINT", () => {
    shutdown();
  });
}

const mode = process.argv[2];
if (mode === "serve") {
  serve().catch((error) => {
    console.error(error instanceof Error ? error.stack : error);
    process.exit(1);
  });
} else if (mode === "client") {
  try {
    sendCommand(parseClientArgs(process.argv.slice(3)));
  } catch (error) {
    console.error(error instanceof Error ? error.message : error);
    process.exit(1);
  }
} else {
  console.error("usage: browser.mjs <serve|client> ...");
  process.exit(1);
}
