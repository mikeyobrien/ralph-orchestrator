window.__ModuleLoader__.load({
  id: 'dsh-ralph-orchestrator',
  factory(require) {
    const React = require('react');
    const h = React.createElement;
    const NS = 'ralph-orchestrator';
    const API = '/api/ralph-orchestrator';
    const STORAGE = 'dsh-ralph-orchestrator:forms:v1';
    const activeStatuses = new Set(['preparing', 'running', 'stopping']);
    const statusKeys = new Set(['preparing', 'running', 'review', 'stopping', 'restoring', 'completed', 'exhausted', 'stopped', 'failed', 'interrupted', 'idle', 'pending', 'busy', 'cancelled']);
    const dictionaries = {
      en: {
        title: 'Ralph Orchestrator', run: 'Run', design: 'Design hats', modes: 'Mode',
        refresh: 'Refresh', refreshing: 'Refreshing…', loading: 'Loading…',
        source: 'Source workspace', selectWorkspace: 'Select workspace', taskName: 'Task short name',
        prompt: 'Prompt', requirements: 'Workflow requirements', configuration: 'Configuration',
        selectConfiguration: 'Select configuration', fromScratch: 'From scratch', iterations: 'Iteration budget',
        permissions: 'Permissions', start: 'Start', starting: 'Starting…', generate: 'Generate', generating: 'Generating…',
        inspect: 'Inspect configuration', hats: 'Hats', yaml: 'YAML', noHats: 'No named hats',
        hatId: 'ID', triggers: 'Triggers', publishes: 'Publishes', instructions: 'Instructions',
        fragments: 'Instruction fragments', none: 'None', revision: 'Revision {revision}',
        configuration: 'Configuration',
        history: 'History', noRuns: 'No runs yet', noDesigns: 'No designs yet', sessions: 'Sessions',
        noSessions: 'No sessions yet', progress: 'Iteration {iteration} / {max}', stop: 'Stop', stopping: 'Stopping…',
        workspace: 'Workspace', branch: 'Branch', path: 'Path', baseline: 'Source baseline',
        dirty: 'Starts from committed HEAD. Uncommitted changes stay in the source workspace.',
        currentDraft: 'Current validated draft', noDraft: 'No validated draft yet',
        feedback: 'Feedback', revise: 'Revise', revising: 'Revising…', configurationName: 'Configuration name',
        save: 'Save configuration', saving: 'Saving…', saved: 'Configuration saved', previous: 'Previous revisions',
        runPrompt: 'Original prompt', liveError: 'Live updates disconnected. Use Refresh to check the latest state.',
        eventError: 'Live update could not be read. Use Refresh.', requestFailed: 'Request failed ({status})',
        invalidResponse: 'The server returned an invalid response.', networkError: 'Could not reach Ralph Orchestrator.',
        storageError: 'Form storage is unavailable. Inputs are preserved until this page reloads.',
        invalidBudget: 'Enter a whole iteration budget of at least 1.', openFailed: 'Could not open the session.',
        generationStatus: 'Generation', error: 'Error',
        'status.preparing': 'Preparing', 'status.running': 'Running', 'status.review': 'Ready for review',
        'status.stopping': 'Stopping', 'status.completed': 'Completed', 'status.exhausted': 'Budget exhausted',
        'status.stopped': 'Stopped', 'status.failed': 'Failed', 'status.interrupted': 'Interrupted',
        'interruptedDesign': 'This design stopped before a validated draft was recorded. Restore a configuration you already have, or revise a recorded draft.',
        restore: 'Restore configuration', restoring: 'Restoring…',
        'status.restoring': 'Restoring',
        restoreYaml: 'Configuration YAML', restoreHint: 'Paste the complete Ralph YAML to restore. The Host validates it; acceptance is separate.',
        'status.idle': 'Idle', 'status.pending': 'Pending', 'status.busy': 'Running', 'status.cancelled': 'Cancelled',
      },
      zh: {
        title: 'Ralph Orchestrator', run: '运行', design: '设计角色', modes: '模式',
        refresh: '刷新', refreshing: '刷新中…', loading: '加载中…',
        source: '源工作区', selectWorkspace: '选择工作区', taskName: '任务简称',
        prompt: '提示词', requirements: '工作流需求', configuration: '配置',
        selectConfiguration: '选择配置', fromScratch: '从头设计', iterations: '迭代上限',
        permissions: '权限', start: '开始', starting: '启动中…', generate: '生成', generating: '生成中…',
        inspect: '查看配置', hats: '角色', yaml: 'YAML', noHats: '无命名角色',
        hatId: 'ID', triggers: '触发事件', publishes: '发布事件', instructions: '指令',
        fragments: '指令片段', none: '无', revision: '第 {revision} 版', configuration: '配置',
        history: '历史', noRuns: '暂无运行记录', noDesigns: '暂无设计记录', sessions: '会话',
        noSessions: '暂无会话', progress: '第 {iteration} / {max} 次迭代', stop: '停止', stopping: '停止中…',
        workspace: '工作区', branch: '分支', path: '路径', baseline: '源基线',
        dirty: '从已提交的 HEAD 开始。未提交的更改保留在源工作区。',
        currentDraft: '当前已验证草稿', noDraft: '暂无已验证草稿',
        feedback: '反馈', revise: '修订', revising: '修订中…', configurationName: '配置名称',
        save: '保存配置', saving: '保存中…', saved: '配置已保存', previous: '历史版本',
        runPrompt: '原始提示词', liveError: '实时更新已断开。请刷新查看最新状态。',
        eventError: '无法读取实时更新。请刷新。', requestFailed: '请求失败（{status}）',
        invalidResponse: '服务器返回了无效响应。', networkError: '无法连接 Ralph Orchestrator。',
        storageError: '表单存储不可用。输入内容将保留至页面重新加载。',
        invalidBudget: '请输入至少为 1 的整数迭代上限。', openFailed: '无法打开会话。',
        generationStatus: '生成状态', error: '错误',
        'status.preparing': '准备中', 'status.running': '运行中', 'status.review': '待审阅',
        'status.stopping': '停止中', 'status.completed': '已完成', 'status.exhausted': '迭代已用尽',
        'status.stopped': '已停止', 'status.failed': '失败', 'status.interrupted': '已中断',
        'interruptedDesign': '此设计在记录已验证草稿前已停止。请还原你已有的配置，或修订已记录的草稿。',
        restore: '还原配置', restoring: '还原中…',
        'status.restoring': '还原中',
        restoreYaml: '配置 YAML', restoreHint: '粘贴要还原的完整 Ralph YAML。主机将验证它；验收是单独的操作。',
        'status.idle': '空闲', 'status.pending': '等待中', 'status.busy': '运行中', 'status.cancelled': '已取消',
      },
    };
    const css = `
      .rpo-page{box-sizing:border-box;height:100%;overflow:auto;color:var(--dsw-alias-label-primary);padding:28px clamp(20px,4vw,48px) 48px;font-size:13px;line-height:1.5;font-family:inherit}
      .rpo-page *{box-sizing:border-box}.rpo-wrap{max-width:1120px;margin:0 auto}
      .rpo-head,.rpo-row{display:flex;align-items:center;justify-content:space-between;gap:12px;flex-wrap:wrap}
      .rpo-head{margin-bottom:24px}.rpo-page h1{font-size:20px;line-height:28px;font-weight:500;margin:0}
      .rpo-page h2{font-size:14px;line-height:22px;font-weight:600;margin:0}.rpo-page h3{font-size:13px;font-weight:600;margin:0}
      .rpo-page p{margin:0}.rpo-modes{display:flex;gap:4px;padding:3px;border:1px solid var(--dsw-alias-border-l1);border-radius:9px;width:fit-content;margin-bottom:24px;background:var(--dsw-alias-bg-layer-1)}
      .rpo-button{font:inherit;cursor:pointer;border:1px solid var(--dsw-alias-border-l2);background:var(--dsw-alias-bg-base);color:var(--dsw-alias-label-primary);border-radius:7px;min-height:34px;padding:6px 12px;display:inline-flex;align-items:center;justify-content:center;gap:6px;text-align:center}
      .rpo-button:hover:not(:disabled){background:var(--dsw-alias-bg-layer-2)}.rpo-button:disabled{opacity:.5;cursor:not-allowed}
      .rpo-button:focus-visible,.rpo-control:focus-visible,.rpo-page summary:focus-visible,.rpo-history-item:focus-visible{outline:2px solid var(--dsw-alias-brand-primary);outline-offset:3px}
      .rpo-primary{background:var(--dsw-alias-brand-primary);border-color:var(--dsw-alias-brand-primary);color:var(--dsw-alias-bg-base)}
      .rpo-primary:hover:not(:disabled){background:var(--dsw-alias-brand-primary);opacity:.9}.rpo-danger{color:var(--dsw-alias-state-error-primary)}
      .rpo-tab{border-color:transparent;background:transparent;min-width:92px}.rpo-tab[aria-selected=true]{background:var(--dsw-alias-bg-base);border-color:var(--dsw-alias-border-l1)}
      .rpo-layout{display:grid;grid-template-columns:minmax(0,1fr) 268px;gap:28px;align-items:start}.rpo-stack{display:flex;flex-direction:column;gap:20px;min-width:0}
      .rpo-card{border:1px solid var(--dsw-alias-border-l1);border-radius:10px;padding:20px;min-width:0}.rpo-form{display:flex;flex-direction:column;gap:18px}
      .rpo-grid{display:grid;grid-template-columns:repeat(2,minmax(0,1fr));gap:16px}.rpo-field{display:flex;flex-direction:column;gap:7px;min-width:0}.rpo-field>span{font-weight:500}
      .rpo-control{font:inherit;line-height:1.5;width:100%;min-width:0;border:1px solid var(--dsw-alias-border-l2);border-radius:7px;padding:8px 10px;background:var(--dsw-alias-bg-base);color:var(--dsw-alias-label-primary)}
      textarea.rpo-control{resize:vertical;min-height:136px}.rpo-review textarea{min-height:92px}.rpo-actions{display:flex;gap:10px;align-items:end;justify-content:flex-end;flex-wrap:wrap}.rpo-actions>.rpo-field{flex:1;min-width:130px}
      .rpo-note{color:var(--dsw-alias-label-secondary);font-size:12px}.rpo-alert{border:1px solid var(--dsw-alias-border-l2);border-left:3px solid var(--dsw-alias-state-error-primary);background:var(--dsw-alias-bg-layer-1);padding:10px 12px;border-radius:6px;white-space:pre-wrap;overflow-wrap:anywhere;margin-bottom:16px}
      .rpo-warning{border-left-color:var(--dsw-alias-state-warn-primary)}.rpo-success{color:var(--dsw-alias-state-success-primary)}
      .rpo-page details{min-width:0}.rpo-page summary{cursor:pointer;font-weight:500;overflow-wrap:anywhere}.rpo-detail-body{padding-top:14px;display:flex;flex-direction:column;gap:14px}
      .rpo-hats{display:flex;flex-direction:column;gap:10px}.rpo-hat{border:1px solid var(--dsw-alias-border-l1);border-radius:7px;padding:12px}
      .rpo-page pre{margin:0;white-space:pre-wrap;overflow-wrap:anywhere;font:12px/1.65 ui-monospace,SFMono-Regular,Consolas,monospace;background:var(--dsw-alias-bg-layer-1);border:1px solid var(--dsw-alias-border-l1);border-radius:7px;padding:12px;max-height:420px;overflow:auto}
      .rpo-page dl{display:grid;grid-template-columns:max-content minmax(0,1fr);gap:5px 14px;margin:0;font-size:12px}.rpo-page dt{color:var(--dsw-alias-label-secondary)}.rpo-page dd{margin:0;overflow-wrap:anywhere}
      .rpo-history{display:flex;flex-direction:column;gap:12px;min-width:0}.rpo-history-list{display:flex;flex-direction:column;gap:6px;max-height:680px;overflow:auto;padding:3px}
      .rpo-history-item{font:inherit;display:flex;flex-direction:column;gap:5px;text-align:left;width:100%;border:1px solid transparent;border-radius:8px;background:transparent;color:var(--dsw-alias-label-primary);padding:10px;cursor:pointer;min-width:0}
      .rpo-history-item:hover{background:var(--dsw-alias-bg-layer-1)}.rpo-history-item[aria-current=true]{background:var(--dsw-alias-bg-layer-1);border-color:var(--dsw-alias-border-l2)}.rpo-history-name{font-weight:500;overflow-wrap:anywhere}
      .rpo-status{font-size:12px;color:var(--dsw-alias-label-secondary);display:inline-flex;gap:6px;align-items:center}.rpo-status:before{content:'';width:6px;height:6px;border-radius:50%;background:var(--dsw-alias-state-idle-primary);flex:none}
      .rpo-status[data-status=running]:before,.rpo-status[data-status=preparing]:before{background:var(--dsw-alias-brand-primary)}.rpo-status[data-status=completed]:before{background:var(--dsw-alias-state-success-primary)}
      .rpo-status[data-status=failed]:before,.rpo-status[data-status=interrupted]:before{background:var(--dsw-alias-state-error-primary)}.rpo-status[data-status=exhausted]:before,.rpo-status[data-status=review]:before{background:var(--dsw-alias-state-warn-primary)}
      .rpo-sessions{list-style:none;padding:0;margin:0;display:flex;flex-direction:column;gap:4px}.rpo-session{display:flex;align-items:center;gap:12px;justify-content:space-between;padding:6px 0}
      .rpo-link{font:inherit;color:var(--dsw-alias-brand-primary);text-align:left;background:none;border:0;padding:2px 0;cursor:pointer;overflow-wrap:anywhere}.rpo-link:hover{text-decoration:underline}.rpo-link:focus-visible{outline:2px solid var(--dsw-alias-brand-primary);outline-offset:3px}
      .rpo-divider{border-top:1px solid var(--dsw-alias-border-l1);padding-top:18px}.rpo-feedback{white-space:pre-wrap;overflow-wrap:anywhere}.rpo-progress{width:100%;height:5px;accent-color:var(--dsw-alias-brand-primary)}
      [data-platform=darwin] .rpo-page{padding-top:calc(28px + var(--dsh-frame-top-clearance,0px))}
      @media(max-width:940px){.rpo-layout{grid-template-columns:minmax(0,1fr)}.rpo-history-list{max-height:360px}}
      @media(max-width:560px){.rpo-grid{grid-template-columns:minmax(0,1fr)}.rpo-card{padding:16px}.rpo-page{padding-left:16px;padding-right:16px}.rpo-session{align-items:flex-start}}
    `;

    function Icon(props) {
      return h('svg', { viewBox: '0 0 64 64', width: props.size || 20, height: props.size || 20, fill: 'none', 'aria-hidden': true },
        h('path', { d: 'M14 37c0-13 8-23 20-23h12m0 0-6-6m6 6-6 6M50 27c0 13-8 23-20 23H18m0 0 6 6m-6-6 6-6', stroke: 'currentColor', strokeWidth: 3.5, strokeLinecap: 'round', strokeLinejoin: 'round' }),
        h('path', { d: 'm22 36 4-13h12l4 13M18 37h28M25 30h14', stroke: 'currentColor', strokeWidth: 3.5, strokeLinecap: 'round', strokeLinejoin: 'round' }));
    }

    function blankForm() {
      return { workspaceId: '', taskName: '', taskEdited: false, prompt: '', configurationId: '', maxIterations: '10', permissionPreset: '', selectedRunId: '', requestId: '' };
    }

    // Restore only our known form fields. Run records and validated drafts always come from the Host.
    function restoreForms() {
      const result = { mode: 'run', run: blankForm(), design: blankForm(), reviews: {} };
      let storageError = false;
      try {
        const saved = JSON.parse(sessionStorage.getItem(STORAGE) || 'null');
        if (saved && saved.version === 1) {
          if (saved.mode === 'design') result.mode = 'design';
          for (const mode of ['run', 'design']) {
            if (!saved[mode] || typeof saved[mode] !== 'object') continue;
            for (const key of Object.keys(result[mode])) {
              if (typeof saved[mode][key] === typeof result[mode][key]) result[mode][key] = saved[mode][key];
            }
          }
          if (Array.isArray(saved.reviews)) {
            for (const entry of saved.reviews) {
              if (entry && typeof entry.id === 'string' && typeof entry.feedback === 'string' && typeof entry.name === 'string' && typeof entry.maxIterations === 'string') {
                Object.defineProperty(result.reviews, entry.id, { value: { feedback: entry.feedback, name: entry.name, maxIterations: entry.maxIterations }, enumerable: true, writable: true, configurable: true });
              }
            }
          }
        }
      } catch (_) { storageError = true; }
      return { forms: result, storageError };
    }

    function slug(prompt) {
      return prompt.trim().split('\n')[0].normalize('NFKD').replace(/[\u0300-\u036f]/g, '').toLowerCase().replace(/[^a-z0-9]+/g, '-').replace(/^-|-$/g, '').slice(0, 48).replace(/-$/g, '') || 'task';
    }

    function errorText(error, t) {
      if (error && typeof error.message === 'string') return (error.code ? error.code + ': ' : '') + error.message;
      if (typeof error === 'string') return error;
      return t('networkError');
    }

    function Status({ status, t }) {
      return h('span', { className: 'rpo-status', 'data-status': status }, statusKeys.has(status) ? t('status.' + status) : status);
    }

    function ConfigurationView({ configuration, t }) {
      return h('div', { className: 'rpo-detail-body' },
        h('details', { open: true }, h('summary', null, t('hats')),
          h('div', { className: 'rpo-detail-body rpo-hats' },
            configuration.hats.length ? configuration.hats.map(hat => h('details', { className: 'rpo-hat', key: hat.id },
              h('summary', null, hat.name || hat.id),
              h('div', { className: 'rpo-detail-body' },
                hat.description && h('p', null, hat.description),
                h('dl', null,
                  h('dt', null, t('hatId')), h('dd', null, hat.id),
                  h('dt', null, t('triggers')), h('dd', null, (hat.triggers || []).join(', ') || t('none')),
                  h('dt', null, t('publishes')), h('dd', null, (hat.publishes || []).join(', ') || t('none'))),
                hat.instructions && h('div', { className: 'rpo-stack' }, h('h3', null, t('instructions')), h('pre', null, hat.instructions)),
                hat.instructionFragments && h('div', { className: 'rpo-stack' }, h('h3', null, t('fragments')), h('pre', null, typeof hat.instructionFragments === 'string' ? hat.instructionFragments : JSON.stringify(hat.instructionFragments, null, 2)))
              ))) : h('p', { className: 'rpo-note' }, t('noHats')))),
        h('details', null, h('summary', null, t('yaml')), h('div', { className: 'rpo-detail-body' }, h('pre', null, configuration.yaml))));
    }

    return {
      inject: ['slots', 'locale', 'uiWorkspace', 'sessions'],
      apply(ctx) {
        ctx.effect(() => ctx.locale.register(NS, 'en', dictionaries.en));
        ctx.effect(() => ctx.locale.register(NS, 'zh', dictionaries.zh));
        const t = ctx.locale.bind(NS);
        const restored = restoreForms();
        let snapshot = { forms: restored.forms, storageError: restored.storageError, data: null, loading: true, error: null, liveError: null, pending: {}, notice: null };
        const subscribers = new Set();
        const controllers = new Set();
        let disposed = false;
        let dataEpoch = 0;
        let refreshPromise = null;
        const publish = patch => {
          if (disposed) return;
          snapshot = { ...snapshot, ...patch };
          subscribers.forEach(fn => fn());
        };
        const subscribe = fn => { subscribers.add(fn); return () => subscribers.delete(fn); };
        const getSnapshot = () => snapshot;
        const persist = forms => {
          let storageError = snapshot.storageError;
          try {
            sessionStorage.setItem(STORAGE, JSON.stringify({ version: 1, mode: forms.mode, run: forms.run, design: forms.design, reviews: Object.entries(forms.reviews).map(([id, review]) => ({ id, ...review })) }));
            storageError = false;
          } catch (_) { storageError = true; }
          publish({ forms, storageError, notice: null });
        };
        const updateForm = (mode, changes, resetRequest = true) => persist({ ...snapshot.forms, [mode]: { ...snapshot.forms[mode], ...changes, ...(resetRequest ? { requestId: '' } : {}) } });
        const updateReview = (run, changes) => persist({ ...snapshot.forms, reviews: { ...snapshot.forms.reviews, [run.id]: { feedback: '', name: run.name, maxIterations: String(run.maxIterations), ...snapshot.forms.reviews[run.id], ...changes } } });
        const applyData = data => {
          if (!data || !Array.isArray(data.workspaces) || !Array.isArray(data.configurations) || !Array.isArray(data.runs)) throw new Error(t('invalidResponse'));
          dataEpoch++;
          const forms = snapshot.forms;
          let nextForms = forms;
          for (const mode of ['run', 'design']) {
            const form = nextForms[mode];
            const changes = {};
            if (!form.workspaceId && data.workspaces.length) changes.workspaceId = data.workspaces[0].id;
            if (mode === 'run' && !form.configurationId && data.configurations.length) changes.configurationId = data.configurations[0].id;
            if (!form.permissionPreset && data.defaults && data.defaults.permissionPreset) changes.permissionPreset = data.defaults.permissionPreset;
            if (Object.keys(changes).length) nextForms = { ...nextForms, [mode]: { ...form, ...changes } };
          }
          if (nextForms !== forms) persist(nextForms);
          publish({ data });
        };
        async function request(payload) {
          const controller = new AbortController();
          controllers.add(controller);
          try {
            const response = await fetch(API, { credentials: 'same-origin', cache: 'no-store', signal: controller.signal, ...(payload ? { method: 'POST', headers: { 'Content-Type': 'application/json' }, body: JSON.stringify(payload) } : {}) });
            let envelope;
            try { envelope = await response.json(); } catch (_) { throw new Error(t('requestFailed', { status: response.status })); }
            if (!envelope || typeof envelope.ok !== 'boolean') throw new Error(t('invalidResponse'));
            if (!envelope.ok) {
              const failure = new Error(errorText(envelope.error, t));
              throw failure;
            }
            if (!response.ok) throw new Error(t('requestFailed', { status: response.status }));
            return envelope.value;
          } finally { controllers.delete(controller); }
        }
        function refresh() {
          if (refreshPromise) return refreshPromise;
          publish({ loading: true, error: null });
          const epoch = dataEpoch;
          refreshPromise = request().then(data => { if (epoch === dataEpoch) applyData(data); }).catch(error => {
            if (error.name !== 'AbortError') publish({ error: errorText(error, t) });
          }).finally(() => { refreshPromise = null; publish({ loading: false }); });
          return refreshPromise;
        }
        async function mutate(key, payload, success) {
          if (snapshot.pending[key]) return;
          publish({ pending: { ...snapshot.pending, [key]: true }, error: null, notice: null });
          try {
            const result = await request(payload);
            if (disposed) return;
            if (success) success(result);
            // A read already in flight may predate this write; always follow it with a fresh read.
            if (refreshPromise) await refreshPromise;
            if (!disposed) await refresh();
          } catch (error) {
            if (error.name !== 'AbortError') publish({ error: errorText(error, t) });
          } finally {
            const pending = { ...snapshot.pending };
            delete pending[key];
            publish({ pending });
          }
        }
        function start(mode) {
          const key = 'start:' + mode;
          if (snapshot.pending[key]) return;
          const form = snapshot.forms[mode];
          const knownRunIds = new Set(snapshot.data ? snapshot.data.runs.map(run => run.id) : []);
          const maxIterations = Number(form.maxIterations);
          if (!Number.isSafeInteger(maxIterations) || maxIterations < 1) { publish({ error: t('invalidBudget') }); return; }
          let requestId;
          try { requestId = form.requestId || crypto.randomUUID(); } catch (error) { publish({ error: errorText(error, t) }); return; }
          updateForm(mode, { requestId }, false);
          mutate(key, { action: mode === 'run' ? 'start' : 'design', requestId, workspaceId: form.workspaceId, taskName: form.taskName.trim(), prompt: form.prompt, maxIterations, ...(form.configurationId ? { configurationId: form.configurationId } : {}), ...(form.permissionPreset ? { permissionPreset: form.permissionPreset } : {}) }, result => {
            const run = result && result.id ? result : result && Array.isArray(result.runs) ? result.runs
              .filter(item => item.mode === mode && !knownRunIds.has(item.id))
              .sort((a, b) => b.createdAt - a.createdAt)[0] : null;
            updateForm(mode, { requestId: '', ...(run ? { selectedRunId: run.id } : {}) }, false);
          });
        }
        async function openSession(id) {
          try {
            // Run records can name freshly persisted sessions not yet catalogued
            // by the Client. Navigation accepts only Host-known identities.
            await ctx.sessions.refresh();
            ctx.uiWorkspace.openSession(id);
            publish({ error: null });
          } catch (error) { publish({ error: errorText(error, t) || t('openFailed') }); }
        }

        ctx.effect(() => {
          let events;
          refresh();
          try {
            events = new EventSource(API + '/events', { withCredentials: true });
            const onState = event => {
              try { applyData(JSON.parse(event.data)); publish({ liveError: null }); }
              catch (_) { publish({ liveError: t('eventError') }); }
            };
            const onError = () => publish({ liveError: t('liveError') });
            events.addEventListener('state', onState);
            events.addEventListener('error', onError);
            return () => {
              disposed = true;
              events.removeEventListener('state', onState);
              events.removeEventListener('error', onError);
              events.close();
              controllers.forEach(controller => controller.abort());
              subscribers.clear();
            };
          } catch (_) { publish({ liveError: t('liveError') }); }
          return () => { disposed = true; if (events) events.close(); controllers.forEach(controller => controller.abort()); subscribers.clear(); };
        });

        function Field({ label, children }) { return h('label', { className: 'rpo-field' }, h('span', null, label), children); }
        function RunDetails({ run, state, translate }) {
          const busy = activeStatuses.has(run.status);
          const draft = run.draft;
          const previousDrafts = (run.drafts || []).filter(item => !draft || item.revision !== draft.revision);
          const review = state.forms.reviews[run.id] || { feedback: '', name: run.name, maxIterations: String(run.maxIterations) };
          const mutating = !!state.pending['run:' + run.id];
          const reviewable = run.mode === 'design' && draft && run.status === 'review';
          const restorable = run.mode === 'design' && (run.status === 'interrupted' || run.status === 'failed' || (run.status === 'review' && !draft));
          const interrupted = run.mode === 'design' && (run.status === 'interrupted' || run.status === 'failed');
          const budget = Number(review.maxIterations);
          const validBudget = Number.isSafeInteger(budget) && budget >= 1;
          const statusLabel = run.mode === 'design' ? translate('generationStatus') : translate('run');
          const configuration = state.data ? state.data.configurations.find(item => item.id === run.configurationId) : undefined;
          const configurationName = (configuration && configuration.name) || run.configurationId;
          const configurationSummary = run.configurationId && translate('configuration') + ': ' + configurationName
            + (run.configurationRevision ? ' · ' + translate('revision', { revision: run.configurationRevision }) : '');
          const revise = event => {
            event.preventDefault();
            if (!validBudget) { publish({ error: translate('invalidBudget') }); return; }
            mutate('run:' + run.id, { action: 'revise', runId: run.id, feedback: review.feedback, maxIterations: budget });
          };
          const restore = event => {
            event.preventDefault();
            mutate('run:' + run.id, { action: 'restore', runId: run.id, yaml: review.restoreYaml });
          };
          return h('section', { className: 'rpo-card rpo-stack', 'aria-label': run.name, 'aria-busy': mutating },
            h('div', { className: 'rpo-row' }, h('h2', null, run.name),
              h('div', { className: 'rpo-row' }, h(Status, { status: run.status, t: translate }),
                busy && run.status !== 'stopping' && h('button', { type: 'button', className: 'rpo-button rpo-danger', disabled: mutating, onClick: () => mutate('run:' + run.id, { action: 'stop', runId: run.id }) }, translate('stop')))),
            h('div', { className: 'rpo-stack', style: { gap: 6 }, 'aria-label': statusLabel },
              h('p', { className: 'rpo-note', role: 'status' }, translate('progress', { iteration: run.iteration, max: run.maxIterations })),
              h('progress', { className: 'rpo-progress', value: Math.min(run.iteration, run.maxIterations), max: run.maxIterations, 'aria-label': statusLabel })),
            run.error && h('div', { className: 'rpo-alert', role: 'alert' }, errorText(run.error, translate)),
            configurationSummary && h('details', null, h('summary', null, configurationSummary),
              h(ConfigurationView, { configuration: { hats: run.hats || (configuration && configuration.hats) || [], yaml: run.executionYaml || run.yaml }, t: translate })),
            h('dl', null,
              run.workspaceId && h(React.Fragment, null, h('dt', null, translate('workspace')), h('dd', null, run.workspaceId)),
              run.path && h(React.Fragment, null, h('dt', null, translate('path')), h('dd', null, run.path)),
              run.branch && h(React.Fragment, null, h('dt', null, translate('branch')), h('dd', null, run.branch)),
              run.baseCommit && h(React.Fragment, null, h('dt', null, translate('baseline')), h('dd', null, run.baseCommit))),
            h('details', null, h('summary', null, translate('runPrompt')), h('div', { className: 'rpo-detail-body' }, h('pre', null, run.prompt))),
            run.mode === 'design' && h('section', { className: 'rpo-divider rpo-stack' },
              h('div', { className: 'rpo-row' }, h('h2', null, translate('currentDraft')), draft && h('span', { className: 'rpo-note' }, translate('revision', { revision: draft.revision }))),
              draft ? h(ConfigurationView, { configuration: draft, t: translate }) : h('p', { className: 'rpo-note' }, translate('noDraft')),
              restorable && h('div', { className: 'rpo-stack rpo-review', key: 'restore-block' },
                interrupted && h('div', { className: 'rpo-alert rpo-warning', role: 'status' }, translate('interruptedDesign')),
                h('details', { open: !draft }, h('summary', null, translate('restore')),
                  h('div', { className: 'rpo-detail-body rpo-stack' },
                    h('p', { className: 'rpo-note' }, translate('restoreHint')),
                    h('form', { className: 'rpo-form', onSubmit: restore },
                      h(Field, { label: translate('restoreYaml') }, h('textarea', { className: 'rpo-control', rows: 8, spellCheck: false, value: review.restoreYaml || '', disabled: mutating, onChange: event => updateReview(run, { restoreYaml: event.target.value }) })),
                      h('button', { className: 'rpo-button', type: 'submit', disabled: mutating || !(review.restoreYaml || '').trim() }, translate(mutating ? 'restoring' : 'restore'))))))),
              reviewable && h('div', { className: 'rpo-stack rpo-review' },
                h('form', { className: 'rpo-form', onSubmit: revise },
                  h(Field, { label: translate('feedback') }, h('textarea', { className: 'rpo-control', value: review.feedback, required: true, disabled: mutating, onChange: event => updateReview(run, { feedback: event.target.value }) })),
                  h('div', { className: 'rpo-actions' },
                    h(Field, { label: translate('iterations') }, h('input', { className: 'rpo-control', type: 'number', min: 1, step: 1, value: review.maxIterations, required: true, disabled: mutating, onChange: event => updateReview(run, { maxIterations: event.target.value }) })),
                    h('button', { className: 'rpo-button', type: 'submit', disabled: mutating || !review.feedback.trim() }, translate(mutating ? 'revising' : 'revise')))),
                h('form', { className: 'rpo-actions', onSubmit: event => {
                  event.preventDefault();
                  mutate('run:' + run.id, { action: 'accept', runId: run.id, revision: draft.revision, name: review.name.trim() }, () => publish({ notice: translate('saved') }));
                } },
                  h(Field, { label: translate('configurationName') }, h('input', { className: 'rpo-control', value: review.name, required: true, disabled: mutating, onChange: event => updateReview(run, { name: event.target.value }) })),
                  h('button', { className: 'rpo-button rpo-primary', type: 'submit', disabled: mutating || !review.name.trim() }, translate(mutating ? 'saving' : 'save')))),
              previousDrafts.length > 0 && h('details', null, h('summary', null, translate('previous')),
                h('div', { className: 'rpo-detail-body' }, previousDrafts.map(item => h('details', { key: item.revision, className: 'rpo-hat' },
                  h('summary', null, translate('revision', { revision: item.revision })),
                  h('div', { className: 'rpo-detail-body' }, item.feedback && h('div', null, h('h3', null, translate('feedback')), h('p', { className: 'rpo-feedback' }, item.feedback)), h(ConfigurationView, { configuration: item, t: translate })))))),
            h('section', { className: 'rpo-divider rpo-stack', style: { gap: 10 } }, h('h2', null, translate('sessions')),
              run.sessions.length ? h('ul', { className: 'rpo-sessions' }, run.sessions.map(session => h('li', { key: session.id, className: 'rpo-session' },
                h('button', { type: 'button', className: 'rpo-link', onClick: () => openSession(session.id) }, session.title || session.id),
                session.status && h(Status, { status: session.status, t: translate })))) : h('p', { className: 'rpo-note' }, translate('noSessions'))));
        }

        function Panel(props) {
          const translate = props.t || t;
          const state = React.useSyncExternalStore(subscribe, getSnapshot, getSnapshot);
          const mode = state.forms.mode;
          const form = state.forms[mode];
          const data = state.data;
          const workspaces = data ? data.workspaces : [];
          const configurations = data ? data.configurations : [];
          const configuration = configurations.find(item => item.id === form.configurationId);
          const source = workspaces.find(item => item.id === form.workspaceId);
          const runs = data ? data.runs.filter(item => item.mode === mode).slice().sort((a, b) => b.createdAt - a.createdAt) : [];
          const selected = runs.find(item => item.id === form.selectedRunId) || runs[0];
          const starting = !!state.pending['start:' + mode];
          const presets = data && data.defaults && data.defaults.permissionPresets || [];
          const change = changes => updateForm(mode, changes);
          const selectMode = next => persist({ ...snapshot.forms, mode: next });
          const selectRun = id => updateForm(mode, { selectedRunId: id }, false);
          const modeKeydown = event => {
            if (!['ArrowLeft', 'ArrowRight', 'Home', 'End'].includes(event.key)) return;
            event.preventDefault();
            const next = event.key === 'Home' ? 'run' : event.key === 'End' ? 'design' : mode === 'run' ? 'design' : 'run';
            selectMode(next);
            event.currentTarget.parentElement.querySelector('[data-mode="' + next + '"]').focus();
          };
          return h('section', { className: 'rpo-page', 'aria-label': translate('title') }, h('style', null, css),
            h('div', { className: 'rpo-wrap' },
              h('header', { className: 'rpo-head' }, h('h1', null, translate('title')),
                h('button', { className: 'rpo-button', type: 'button', disabled: state.loading, onClick: refresh }, translate(state.loading && data ? 'refreshing' : 'refresh'))),
              state.error && h('div', { className: 'rpo-alert', role: 'alert' }, state.error),
              state.liveError && h('div', { className: 'rpo-alert rpo-warning', role: 'status' }, state.liveError),
              state.storageError && h('div', { className: 'rpo-alert rpo-warning', role: 'status' }, translate('storageError')),
              state.notice && h('p', { className: 'rpo-success', role: 'status' }, state.notice),
              h('div', { className: 'rpo-modes', role: 'tablist', 'aria-label': translate('modes') }, ['run', 'design'].map(item => h('button', {
                key: item, type: 'button', className: 'rpo-button rpo-tab', role: 'tab', id: 'rpo-tab-' + item, 'data-mode': item,
                'aria-selected': mode === item, 'aria-controls': 'rpo-panel', tabIndex: mode === item ? 0 : -1,
                onClick: () => selectMode(item), onKeyDown: modeKeydown,
              }, translate(item)))),
              !data && state.loading ? h('p', { className: 'rpo-note', role: 'status' }, translate('loading')) : h('div', { id: 'rpo-panel', role: 'tabpanel', 'aria-labelledby': 'rpo-tab-' + mode, className: 'rpo-layout' },
                h('div', { className: 'rpo-stack' },
                  h('form', { className: 'rpo-card rpo-form', 'aria-label': translate(mode), 'aria-busy': starting, onSubmit: event => { event.preventDefault(); start(mode); } },
                    h('div', { className: 'rpo-grid' },
                      h(Field, { label: translate('source') }, h('select', { className: 'rpo-control', value: form.workspaceId, required: true, disabled: starting || !data, onChange: event => change({ workspaceId: event.target.value }) },
                        h('option', { value: '', disabled: true }, translate('selectWorkspace')),
                        form.workspaceId && !source && h('option', { value: form.workspaceId, disabled: true }, form.workspaceId),
                        workspaces.map(item => h('option', { key: item.id, value: item.id }, item.title || item.path)))),
                      h(Field, { label: translate('taskName') }, h('input', { className: 'rpo-control', value: form.taskName, required: true, disabled: starting, autoComplete: 'off', onChange: event => change({ taskName: event.target.value, taskEdited: true }) }))),
                    source && source.dirty && h('div', { className: 'rpo-alert rpo-warning', role: 'status' },
                      h('p', null, translate('dirty')),
                      source.head && h('p', { className: 'rpo-note' }, translate('baseline') + ': ', h('code', null, source.head))),
                    h(Field, { label: translate(mode === 'run' ? 'prompt' : 'requirements') }, h('textarea', { className: 'rpo-control', value: form.prompt, required: true, disabled: starting, onChange: event => {
                      const prompt = event.target.value;
                      change({ prompt, ...(!form.taskEdited ? { taskName: prompt.trim() ? slug(prompt) : '' } : {}) });
                    } })),
                    h('div', { className: 'rpo-grid' },
                      h(Field, { label: translate('configuration') }, h('select', { className: 'rpo-control', value: form.configurationId, required: mode === 'run', disabled: starting || !data, onChange: event => change({ configurationId: event.target.value }) },
                        h('option', { value: '', disabled: mode === 'run' }, translate(mode === 'run' ? 'selectConfiguration' : 'fromScratch')),
                        form.configurationId && !configuration && h('option', { value: form.configurationId, disabled: true }, form.configurationId),
                        configurations.map(item => h('option', { key: item.id, value: item.id }, item.name + ' · ' + translate('revision', { revision: item.revision }))))),
                      h(Field, { label: translate('iterations') }, h('input', { className: 'rpo-control', type: 'number', min: 1, step: 1, value: form.maxIterations, required: true, disabled: starting, onChange: event => change({ maxIterations: event.target.value }) }))),
                    configuration && h('details', null, h('summary', null, translate('inspect')), h(ConfigurationView, { configuration, t: translate })),
                    presets.length > 0 && h(Field, { label: translate('permissions') }, h('select', { className: 'rpo-control', value: form.permissionPreset, disabled: starting, onChange: event => change({ permissionPreset: event.target.value }) },
                      !form.permissionPreset && h('option', { value: '' }, translate('none')),
                      presets.map(item => h('option', { key: item.value, value: item.value }, item.name)))),
                    h('div', { className: 'rpo-actions' }, h('button', { className: 'rpo-button rpo-primary', type: 'submit', disabled: starting || !data || !source || !form.taskName.trim() || !form.prompt.trim() || (mode === 'run' && !configuration) }, translate(starting ? mode === 'run' ? 'starting' : 'generating' : mode === 'run' ? 'start' : 'generate')))),
                  selected && h(RunDetails, { run: selected, state, translate })),
                h('aside', { className: 'rpo-history', 'aria-label': translate('history') },
                  h('h2', null, translate('history')),
                  runs.length ? h('div', { className: 'rpo-history-list' }, runs.map(run => h('button', { type: 'button', key: run.id, className: 'rpo-history-item', 'aria-current': selected && run.id === selected.id ? 'true' : undefined, onClick: () => selectRun(run.id) },
                    h('span', { className: 'rpo-history-name' }, run.name), h(Status, { status: run.status, t: translate }),
                    h('span', { className: 'rpo-note' }, translate('progress', { iteration: run.iteration, max: run.maxIterations })),
                    h('time', { className: 'rpo-note', dateTime: new Date(run.createdAt).toISOString() }, new Date(run.createdAt).toLocaleString(ctx.locale.getLocale().active))))) : h('p', { className: 'rpo-note' }, translate(mode === 'run' ? 'noRuns' : 'noDesigns'))))));
        }

        ctx.slots.inject('sidebar.panellist', () => ctx.slots.register({ name: 'sidebar.panellist', id: NS, order: 15, label: () => t('title') }, Icon));
        ctx.slots.inject('main', () => ctx.slots.register({ name: 'main', key: NS, locale: NS }, Panel));
      },
    };
  },
});
