import { parseDocument, stringify } from 'yaml';
import { parseAgentModel, parseHatBackend } from './backends.js';

export const BUILTINS = [
  { id: 'builtin:build-review', name: 'Build and review', revision: 1, yaml: stringify({
    event_loop: { starting_event: 'build.start', completion_promise: 'LOOP_COMPLETE', required_events: ['review.passed'], max_iterations: 10 },
    hats: {
      builder: { name: 'Builder', description: 'Implement and verify the requested change.', triggers: ['build.start', 'review.rejected'], publishes: ['review.requested'], instructions: 'Implement the objective. Run relevant checks. Publish review.requested with a concise evidence-based handoff when ready for independent review.' },
      reviewer: { name: 'Reviewer', description: 'Independently verify the change before completion.', triggers: ['review.requested'], publishes: ['review.rejected', 'review.passed', 'LOOP_COMPLETE'], instructions: 'Inspect the actual changes and run relevant checks. If incomplete, publish review.rejected with concrete feedback. If the objective is verified, publish review.passed and then LOOP_COMPLETE as the final event. Never claim checks you did not run.' },
    },
  }) },
  { id: 'builtin:solo', name: 'Solo', revision: 1, yaml: stringify({ event_loop: { max_iterations: 10, completion_promise: 'LOOP_COMPLETE' }, hats: {} }) },
];

export function readConfiguration(yaml) {
  if (typeof yaml !== 'string' || Buffer.byteLength(yaml) > 256 * 1024) throw new Error('Configuration must be YAML smaller than 256 KiB');
  const document = parseDocument(yaml, { uniqueKeys: true });
  if (document.errors.length) throw new Error(document.errors.map(error => error.message).join('\n'));
  const config = document.toJS({ maxAliasCount: 100 });
  if (!config || typeof config !== 'object' || Array.isArray(config)) throw new Error('Configuration must be a YAML mapping');
  return config;
}

/** Validate one hat's backend execution fields. The plugin, not Ralph, consumes them. */
function hatExecution(id, hat) {
  const entry = hat && typeof hat === 'object' && !Array.isArray(hat) ? hat : {};
  let backend;
  try { backend = parseHatBackend(entry.backend); } catch (error) { throw new Error(`hats.${id}.backend: ${error.message}`); }
  let model;
  if (entry.model !== undefined && entry.model !== null && entry.model !== '') {
    if (typeof entry.model !== 'string' || entry.model.trim().length === 0 || entry.model.length > 200) throw new Error(`hats.${id}.model: model must be a non-empty string of at most 200 characters`);
    model = entry.model.trim();
    if (backend === 'dsh') { try { parseAgentModel(model); } catch (error) { throw new Error(`hats.${id}.model: ${error.message}`); } }
  }
  return { backend, model };
}

export function inspectHats(yaml) {
  const config = readConfiguration(yaml);
  return Object.entries(config.hats || {}).map(([id, hat]) => {
    const entry = hat && typeof hat === 'object' && !Array.isArray(hat) ? hat : {};
    const execution = hatExecution(id, entry);
    return {
      id, name: String(entry.name || id), description: String(entry.description || ''),
      triggers: Array.isArray(entry.triggers) ? entry.triggers : [], publishes: Array.isArray(entry.publishes) ? entry.publishes : [],
      instructions: [entry.instructions, ...(entry.extra_instructions || [])].filter(Boolean).join('\n\n'),
      backend: execution.backend, ...(execution.model ? { model: execution.model } : {}),
    };
  });
}

export function executionConfiguration(yaml, { bridgePath, executable = process.execPath, maxIterations = 10 }) {
  const config = readConfiguration(yaml);
  for (const [id, hat] of Object.entries(config.hats || {})) {
    const entry = hat && typeof hat === 'object' && !Array.isArray(hat) ? hat : {};
    if (entry.backend_args !== undefined || entry.args !== undefined) throw new Error(`hats.${id}.backend_args: native backend arguments belong to a Ralph CLI backend and cannot bypass the DSH backend registry; remove the override`);
    hatExecution(id, entry);
    if ((entry.concurrency !== undefined && entry.concurrency !== 1) || entry.aggregate !== undefined) throw new Error(`hats.${id}: concurrent/aggregate hats are not supported by the DSH session bridge`);
    // Ralph must route every hat through the DSH bridge; the plugin owns dispatch,
    // so its own per-hat backend selection never reaches Ralph's backend catalog.
    delete entry.backend;
    delete entry.model;
  }
  if (config.max_cost != null || config.event_loop?.max_cost_usd != null) throw new Error('Cost budgets require backend metrics not supplied by the DSH Text bridge');
  // Native v1 aliases take precedence during normalize(); remove only the two
  // fields whose execution policy this integration explicitly owns.
  delete config.agent;
  delete config.max_iterations;
  config.cli = { ...config.cli, backend: 'custom', command: executable, args: [bridgePath], prompt_mode: 'file', prompt_flag: '--prompt-file' };
  config.event_loop = { ...config.event_loop, max_iterations: maxIterations };
  config.features = { ...config.features, auto_merge: false };
  return stringify(config);
}

export function shortName(text) {
  return String(text || '').normalize('NFKD').toLowerCase().replace(/[^a-z0-9]+/g, '-').replace(/^-|-$/g, '').slice(0, 48).replace(/-$/, '');
}

export function iterationTitle(iteration, maximum, hat, name) {
  const clean = value => String(value).replace(/[\x00-\x1f\x7f]/g, ' ').replace(/\s+/g, ' ').trim();
  return `${iteration}/${maximum} [${clean(hat).slice(0, 36)}] - ${clean(name).slice(0, 72) || 'Work on objective'}`;
}
