import test from 'node:test';
import assert from 'node:assert/strict';
import { createRequire } from 'node:module';
import { join } from 'node:path';
import { pathToFileURL } from 'node:url';
import * as modes from '../modes.js';

// Explicitly test the installed DSH runtime, not a second copy of its registries.
// DSH_RUNTIME_ROOT=/path/to/dsh node --test tests/native-mode-policy.mjs
if (!process.env.DSH_RUNTIME_ROOT) throw new Error('Set DSH_RUNTIME_ROOT to the DSH installation to test native preset scope behavior');
const runtimeRequire = createRequire(join(process.env.DSH_RUNTIME_ROOT, 'package.json'));
const load = name => import(pathToFileURL(runtimeRequire.resolve(name)).href);
const { Context } = await load('@deepseek-ai/cordis');
const { createScope } = await load('@deepseek-ai/dsh-scope');
const { SystemPrompt } = await load('@deepseek-ai/dsh-system-prompt');
const { ToolRuntime } = await load('@deepseek-ai/dsh-tools');
const { default: SessionProjections } = await load('@deepseek-ai/dsh-session-projection');
const { default: LocalFs } = await load('@deepseek-ai/dsh-fs-local');
const { default: WorkingDirectory } = await load('@deepseek-ai/dsh-working-directory');
const nativeFs = await load('@deepseek-ai/dsh-tool-fs');
const definition = (name, execute) => ({
  name, description: 'Native registry boundary fixture',
  parameters: { type: 'object', properties: {}, additionalProperties: false },
  output: { schema: { type: 'object', properties: {}, additionalProperties: false }, render: () => [] },
  execute,
});

for (const role of ['worker', 'designer', 'summary']) {
  test(`installed runtime preserves ${role} native tools through preset inheritance and rejects out-of-role calls`, { timeout: 10000 }, async () => {
    const root = new Context();
    try {
      await root.plugin(SystemPrompt, {});
      await root.plugin(ToolRuntime);
      await root.plugin(SessionProjections);
      await root.plugin(LocalFs, {});
      await root.plugin(WorkingDirectory, { defaultDirectory: process.cwd() });
      let exercised = false;
      await root.plugin({
        inject: ['tools', 'systemPrompt', 'fs', 'workingDirectory'],
        async apply(host) {
          let intendedCalls = 0, forbiddenCalls = 0;
          host.tools.register(definition('plugin_manager', async () => { forbiddenCalls++; return {}; }));
          host.tools.register(definition('principle_read', async () => { intendedCalls++; return {}; }));
          const generationKey = {};
          const generation = createScope(host, generationKey);
          const agent = { id: `native-role-${role}` };
          const child = createScope(host, agent, { parent: generationKey });
          try {
            // This is the real order: standing policy before its native tool rows,
            // then a separate child Agent scope binds to the preset generation.
            await generation.ctx.plugin(modes, { role });
            await generation.ctx.plugin(nativeFs, { read: true, readImage: false, write: role === 'worker', edit: role === 'worker' });
            const inherited = host.tools.schemas(agent).map(schema => schema.name);
            assert.ok(inherited.includes('read'));
            assert.equal(inherited.includes('write'), role === 'worker');
            assert.equal(inherited.includes('edit'), role === 'worker');
            assert.ok(inherited.includes('principle_read'));
            assert.ok(!inherited.includes('plugin_manager'), 'known unrelated globals must not be presented');

            // Agent-local integrations are exempt from inherited presentation
            // restrictions, so execution must be independently guarded.
            child.ctx.tools.register(definition('mcp__test__browser_click', async () => { forbiddenCalls++; return {}; }));
            host.tools.register(definition('late_unsafe', async () => { forbiddenCalls++; return {}; }));
            const intended = role === 'worker' ? 'ralph' : role === 'designer' ? 'ralph_propose_configuration' : 'principle_read';
            if (intended !== 'principle_read') child.ctx.tools.register(definition(intended, async () => { intendedCalls++; return {}; }));
            const run = name => host.tools.execute({ callId: `${role}:${name}`, name, arguments: {}, agent, signal: new AbortController().signal });
            assert.ok(host.tools.schemas(agent).some(schema => schema.name === 'mcp__test__browser_click'), 'document own-schema presentation limitation rather than hiding it in a mock');
            assert.equal((await run('mcp__test__browser_click')).isError, true);
            assert.equal((await run('late_unsafe')).isError, true);
            assert.equal((await run(intended)).isError, false);
            assert.equal(forbiddenCalls, 0);
            assert.equal(intendedCalls, 1);
            if (role !== 'worker') {
              child.ctx.tools.register(definition('write', async () => { forbiddenCalls++; return {}; }));
              assert.equal((await run('write')).isError, true);
              assert.equal(forbiddenCalls, 0);
            }
            exercised = true;
          } finally { await child.dispose(); await generation.dispose(); }
        },
      });
      assert.equal(exercised, true, 'the actual native registry fixture must activate');
    } finally { await root.fiber.dispose(); }
  });
}
