# Ralph Hat-Level Backend 配置失效 Bug 分析

## 问题描述

Ralph Orchestrator 中 Hat 级别的 `backend` 配置（包括 `backend.args`）**完全不起作用**。所有 Hat 都使用全局 `cli.backend` 配置，忽略了 Hat 特定的 backend 设置。

## 复现步骤

### 测试配置 (ralph.test.yml)

```yaml
cli:
  backend: "codex"
  prompt_mode: "arg"

hats:
  test_codex:
    name: "测试 Codex 参数传递"
    triggers: ["test.start"]
    backend:
      type: "custom"
      command: "codex"
      args: ["exec", "-c", "model=gpt-5.1-codex-max", "--dangerously-bypass-approvals-and-sandbox"]
      prompt_mode: "arg"
```

### 期望行为

Hat 执行时应该使用：
```bash
codex exec -c model=gpt-5.1-codex-max --dangerously-bypass-approvals-and-sandbox [prompt]
```

### 实际行为

Hat 执行时实际使用：
```bash
codex exec --full-auto [prompt]
```

**所有 Hat 级别的 `backend.args` 配置被完全忽略！**

## 源代码分析

### 1. Backend 初始化位置

**文件**: `crates/ralph-cli/src/loop_runner.rs:161`

```rust
// Create backend from config - TUI mode uses the same backend as non-TUI
// The TUI is an observation layer that displays output, not a different mode
let backend = CliBackend::from_config(&config.cli).map_err(|e| anyhow::Error::new(e))?;
```

**问题**: Ralph 在循环开始时创建**一个全局 backend**，基于 `config.cli`（全局 CLI 配置）。

### 2. Backend 执行位置

**文件**: `crates/ralph-cli/src/loop_runner.rs:500-511`

```rust
let execute_future = async {
    if use_pty {
        execute_pty(
            pty_executor.as_mut(),
            &backend,  // ← 传递全局 backend
            &config,
            &prompt,
            user_interactive,
            interrupt_rx_for_pty,
            verbosity,
            tui_lines_for_pty,
        )
        .await
    } else {
        let executor = CliExecutor::new(backend.clone());  // ← 使用全局 backend
        ...
    }
};
```

**问题**: 每次执行 Hat 时，都使用同一个全局 `backend` 对象，没有根据当前 Hat 的配置创建新的 backend。

### 3. Hat Backend 配置解析

**文件**: `crates/ralph-adapters/src/cli_backend.rs:173`

```rust
pub fn from_hat_backend(hat_backend: &HatBackend) -> Result<Self, CustomBackendError> {
    match hat_backend {
        HatBackend::Named(name) => Self::from_name(name),

        HatBackend::KiroAgent { agent, .. } =>
            Ok(Self::kiro_with_agent(agent.clone())),

        // 自定义后端：直接使用配置的command和args
        HatBackend::Custom { command, args } => Ok(Self {
            command: command.clone(),
            args: args.clone(),  // ← args 在这里被正确解析
            prompt_mode: PromptMode::Arg,
            prompt_flag: None,
            output_format: OutputFormat::Text,
        }),
    }
}
```

**发现**: `from_hat_backend` 方法已经实现，能够正确解析 Hat 的 backend 配置。

### 4. 调用情况分析

```bash
$ grep -rn "from_hat_backend" crates/ --include="*.rs" | grep -v test

crates/ralph-adapters/src/cli_backend.rs:173:    pub fn from_hat_backend(...) -> ...
crates/ralph-adapters/src/cli_backend.rs:888:   # 单元测试
crates/ralph-adapters/src/cli_backend.rs:898:   # 单元测试
crates/ralph-adapters/src/cli_backend.rs:911:   # 单元测试
```

**关键发现**: `from_hat_backend` 方法**从未在 ralph-cli 中被调用**！只在单元测试中使用。

## Root Cause（根本原因）

Ralph 的设计存在架构缺陷：

1. **Backend 生命周期**: Backend 在循环开始时创建一次，整个循环复用同一个对象
2. **Hat 配置丢失**: Hat 的 `backend` 配置被解析到 `HatConfig` 结构中，但在执行时从未被读取
3. **缺少动态 Backend 创建**: 没有代码在每个 Hat 执行前检查 `hat.backend` 并创建对应的 backend

## 修复方案

### 方案 A: 动态 Backend 创建（推荐）

在每次执行 Hat 前，检查 Hat 是否有自定义 backend 配置：

```rust
// loop_runner.rs:470 附近
// Get next hat to execute
let hat_id = event_loop.next_hat().unwrap();

// 动态创建 backend
let effective_backend = if let Some(hat_backend) = event_loop.get_hat_backend(&hat_id) {
    // Hat 有自定义 backend，使用它
    CliBackend::from_hat_backend(hat_backend)?
} else {
    // 使用全局 backend
    backend.clone()
};

// Execute the prompt with the effective backend
execute_pty(
    pty_executor.as_mut(),
    &effective_backend,  // ← 使用动态 backend
    ...
)
```

### 方案 B: Backend 缓存优化

为避免重复创建相同的 backend，可以缓存：

```rust
// 使用 HashMap 缓存每个 Hat 的 backend
let mut backend_cache: HashMap<HatId, CliBackend> = HashMap::new();

// 在执行时
let effective_backend = backend_cache.entry(hat_id.clone())
    .or_insert_with(|| {
        event_loop.get_hat_backend(&hat_id)
            .and_then(|hb| CliBackend::from_hat_backend(hb).ok())
            .unwrap_or_else(|| backend.clone())
    });
```

## 需要添加的 API

在 `EventLoop` 中添加获取 Hat backend 的方法：

```rust
// crates/ralph-core/src/event_loop/mod.rs

impl EventLoop {
    /// 获取指定 Hat 的 backend 配置
    pub fn get_hat_backend(&self, hat_id: &HatId) -> Option<&HatBackend> {
        self.registry.get(hat_id)?.backend.as_ref()
    }
}
```

## 影响范围

这个 bug 影响所有使用 Hat 级别 backend 配置的场景：

1. **多模型工作流**: 无法为不同的 Hat 指定不同的 AI 模型
2. **自定义参数**: Hat 的 `backend.args` 完全失效
3. **Kiro Agent**: Hat 级别的 Kiro agent 配置无法生效

## 测试验证

### 当前行为验证

```bash
# 运行测试配置
ralph run -c ralph.test.yml --no-tui -p "测试"

# 监控实际命令
ps aux | grep codex
# 结果: codex exec --full-auto ...
# 缺失: -c model=gpt-5.1-codex-max --dangerously-bypass-approvals-and-sandbox
```

### 修复后预期

```bash
# 相同的测试
ralph run -c ralph.test.yml --no-tui -p "测试"

# 监控实际命令
ps aux | grep codex
# 预期: codex exec -c model=gpt-5.1-codex-max --dangerously-bypass-approvals-and-sandbox ...
```

## 相关配置示例

### 有效的配置（修复后）

```yaml
cli:
  backend: "claude"  # 默认 backend
  prompt_mode: "arg"

hats:
  test_writer:
    name: "测试工程师"
    triggers: ["blueprint.parsed"]
    backend:
      type: "custom"
      command: "codex"
      args: ["exec", "-c", "model=gpt-5.1-codex-max", "--yolo"]

  backend_implementer:
    name: "后端实现者"
    triggers: ["tests.written"]
    backend:
      type: "custom"
      command: "codex"
      args: ["exec", "-c", "model=gpt-5.2-codex", "--yolo"]

  reviewer:
    name: "审查员"
    triggers: ["implementation.done"]
    # 没有 backend 配置，使用全局 claude
```

### 当前无效的配置

上述配置在当前版本中：
- `test_writer` 和 `backend_implementer` 的 backend.args 被忽略
- 所有 Hat 都使用 `claude`（全局配置）

## 优先级

**High** - 这是核心功能缺陷，阻止了多模型工作流的使用。

## 相关 Issue

- 配置文档中明确说明支持 Hat 级别的 backend
- 代码中有 `from_hat_backend` 方法但从未使用
- 单元测试通过，但集成场景失败

---

**分析日期**: 2026-01-25
**Ralph 版本**: 2.2.2
**发现者**: Claude Code + 用户测试
