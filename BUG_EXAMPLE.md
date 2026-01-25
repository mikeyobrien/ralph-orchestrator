# Hat Backend 配置失效 - 具体示例

## 你的配置文件 (ralph.blueprint.yml)

### 全局配置

```yaml
cli:
  backend: "claude"  # ← 全局默认使用 Claude
  prompt_mode: "arg"
```

### Hat 配置（你期望的行为）

你配置了 4 个不同的 Hat，每个都有特定的 backend 设置：

#### 1. Blueprint 解析器 - 使用 Claude Opus

```yaml
blueprint_reader:
  name: "📘 Blueprint 解析器 (Opus)"
  triggers: ["blueprint.start"]
  publishes: ["blueprint.parsed"]
  backend:
    type: "custom"
    command: "claude"
    args: ["--model", "opus", "--dangerously-skip-permissions"]
    prompt_mode: "arg"
    prompt_flag: "-p"
```

**期望行为**: 执行时调用
```bash
claude --model opus --dangerously-skip-permissions -p "prompt内容"
```

#### 2. 测试工程师 - 使用 Codex GPT-5.1

```yaml
test_writer:
  name: "🧪 测试工程师 (Codex)"
  triggers: ["blueprint.parsed"]
  publishes: ["tests.written"]
  backend:
    type: "custom"
    command: "codex"
    args: ["--model", "gpt-5.1-codex-max", "--dangerously-bypass-approvals-and-sandbox"]
    prompt_mode: "arg"
    prompt_flag: "-p"
```

**期望行为**: 执行时调用
```bash
codex --model gpt-5.1-codex-max --dangerously-bypass-approvals-and-sandbox -p "prompt内容"
```

#### 3. 后端实现者 - 使用 Codex（默认模型）

```yaml
backend_implementer:
  name: "🐍 后端实现者 (Codex)"
  triggers: ["tests.written"]
  publishes: ["implementation.done"]
  backend:
    type: "custom"
    command: "codex"
    args: ["--dangerously-bypass-approvals-and-sandbox"]
    prompt_mode: "arg"
    prompt_flag: "-p"
```

**期望行为**: 执行时调用
```bash
codex --dangerously-bypass-approvals-and-sandbox -p "prompt内容"
```

#### 4. Blueprint 审查员 - 使用 Codex（默认模型）

```yaml
blueprint_reviewer:
  name: "👀 Blueprint 审查员 (Codex)"
  triggers: ["implementation.done"]
  publishes: ["review.passed"]
  backend:
    type: "custom"
    command: "codex"
    args: ["--dangerously-bypass-approvals-and-sandbox"]
    prompt_mode: "arg"
    prompt_flag: "-p"
```

**期望行为**: 执行时调用
```bash
codex --dangerously-bypass-approvals-and-sandbox -p "prompt内容"
```

---

## 实际发生的情况（Bug）

### 问题：所有 Hat 都使用全局 backend

当你运行 Ralph 时：

```bash
ralph run -c ralph.blueprint.yml -p "实现 M001-auth-service"
```

**实际执行的命令**：

#### ITERATION 1 - blueprint_reader
```bash
claude -p "prompt内容"  # ← 使用全局 backend，忽略了 Hat 配置！
```

❌ **问题**:
- 配置的 `--model opus` 参数丢失
- 配置的 `--dangerously-skip-permissions` 参数丢失
- 使用的是 Claude 的默认模型（Sonnet），而不是 Opus

#### ITERATION 2 - test_writer
```bash
claude -p "prompt内容"  # ← 仍然使用全局 backend！
```

❌ **问题**:
- 配置的 `command: "codex"` 被忽略，仍然调用 `claude`
- 配置的 `--model gpt-5.1-codex-max` 参数完全丢失
- 配置的 `--dangerously-bypass-approvals-and-sandbox` 参数丢失

#### ITERATION 3 - backend_implementer
```bash
claude -p "prompt内容"  # ← 仍然使用全局 backend！
```

❌ **问题**:
- 配置的 `command: "codex"` 被忽略
- 配置的 `--dangerously-bypass-approvals-and-sandbox` 参数丢失

#### ITERATION 4 - blueprint_reviewer
```bash
claude -p "prompt内容"  # ← 仍然使用全局 backend！
```

❌ **问题**:
- 配置的 `command: "codex"` 被忽略
- 配置的 `--dangerously-bypass-approvals-and-sandbox` 参数丢失

---

## 影响

### 1. 多模型工作流无法使用

你的设计意图：
- **Blueprint 解析器**: 使用 Claude Opus（强推理能力）
- **测试工程师**: 使用 Codex GPT-5.1（代码生成专家）
- **后端实现者**: 使用 Codex（代码实现）
- **审查员**: 使用 Codex（代码审查）

**实际情况**：所有 Hat 都使用 Claude Sonnet（全局配置）

### 2. 模型参数无法传递

你想在 `test_writer` 中使用 `gpt-5.1-codex-max` 模型：
```yaml
args: ["--model", "gpt-5.1-codex-max", ...]
```

**实际情况**：这个参数完全被忽略，即使你把全局 backend 改成 codex，也只会使用 `~/.codex/config.toml` 中的默认模型（gpt-5.2-codex）

### 3. 自动批准参数无法传递

你想在所有 Codex Hat 中自动批准：
```yaml
args: ["--dangerously-bypass-approvals-and-sandbox"]
```

**实际情况**：这个参数被忽略，Codex 会弹出批准提示，阻塞工作流

---

## 验证方法

### 方法 1: 查看日志

```bash
# 运行 Ralph
ralph run -c ralph.blueprint.yml -p "实现 M001" --no-tui 2>&1 | tee ralph-test.log

# 查看每次迭代使用的模型
grep -i "model:" ralph-test.log
```

**期望**:
```
[ITERATION 1] model: claude-opus-4-5-...
[ITERATION 2] model: gpt-5.1-codex-max
[ITERATION 3] model: gpt-5.2-codex
[ITERATION 4] model: gpt-5.2-codex
```

**实际**:
```
[ITERATION 1] model: claude-sonnet-4-5-...
[ITERATION 2] model: claude-sonnet-4-5-...
[ITERATION 3] model: claude-sonnet-4-5-...
[ITERATION 4] model: claude-sonnet-4-5-...
```

### 方法 2: 监控进程

```bash
# 在另一个终端运行
watch -n 1 'ps aux | grep -E "(claude|codex)" | grep -v grep'
```

**期望**（不同迭代看到不同命令）:
```
# ITERATION 1
claude --model opus --dangerously-skip-permissions -p ...

# ITERATION 2
codex --model gpt-5.1-codex-max --dangerously-bypass-approvals-and-sandbox -p ...

# ITERATION 3
codex --dangerously-bypass-approvals-and-sandbox -p ...
```

**实际**（所有迭代都一样）:
```
# 所有 ITERATION
claude -p ...
```

---

## 临时解决方案（Workaround）

### 如果你想所有 Hat 都用 Codex

修改全局配置：
```yaml
cli:
  backend: "codex"  # 改成 codex
  prompt_mode: "arg"
```

**缺点**：
- blueprint_reader 也会用 Codex，而不是 Claude Opus
- 所有 Hat 都用同一个模型（~/.codex/config.toml 中的默认模型）
- `--model gpt-5.1-codex-max` 参数仍然不起作用

### 如果你想指定 Codex 模型

修改 `~/.codex/config.toml`：
```toml
model = "gpt-5.1-codex-max"  # 改成你想要的模型
```

**缺点**：
- 所有使用 Codex 的地方都用这个模型
- 无法为不同的 Hat 使用不同的 Codex 模型

---

## 总结

**期望**: 4 个 Hat 使用不同的 AI 模型和参数
**实际**: 4 个 Hat 都使用全局 `cli.backend` 配置，Hat 级别的配置完全失效

这就是为什么你的多模型工作流配置写得很完美，但实际运行时行为不符合预期的原因。
