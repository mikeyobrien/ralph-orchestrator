import Schema from '@deepseek-ai/schemastery';

// Preset-scoped policy applies equally to managed and later reopened sessions.
export const Config = Schema.object({ role: Schema.union(['worker', 'designer', 'summary']).default('worker') });
export const inject = ['tools', 'systemPrompt'];
export const ROLE_TOOLS = {
  worker: ['read', 'read_image', 'write', 'edit', 'glob', 'grep', 'bash', 'working_directory', 'job_list', 'job_output', 'job_kill', 'principle_read', 'ralph', 'ralph_iteration_title'],
  designer: ['read', 'glob', 'grep', 'principle_read', 'ralph_iteration_title', 'ralph_propose_configuration'],
  summary: ['read', 'glob', 'grep', 'principle_read'],
};

export function apply(ctx, { role = 'worker' } = {}) {
  const allowed = new Set(ROLE_TOOLS[role]);
  // A standing preset is an ancestor of each Agent. An empty allow-list here
  // would also hide the preset's own subsequently registered native tools.
  const denied = ctx.tools.schemas().map(schema => schema.name).filter(name => name !== 'run_code' && !allowed.has(name));
  if (denied.length) ctx.tools.restrict({ deny: denied });
  // Agent-local providers such as MCP are exempt from inherited presentation
  // filters. The ancestor guard still denies their execution for this role.
  ctx.tools.guard(execution => allowed.has(execution.name) ? undefined : `Tool is outside the Ralph ${role} role`);
  ctx.tools.presentAs('native');
  for (const [name, order] of [
    ['harness:source', 'HARNESS_SOURCE'],
    ['app:web-surface', 'WEB_SURFACE'],
  ]) {
    ctx.effect(() => ctx.systemPrompt.section({ name, order: ctx.systemPrompt.getSectionOrder(order), text: '' }));
  }
}
