# Roo Code CLI で Ralph Orchestrator を使う

## クイックスタート

### 前提条件

1. **Ralph** をインストール済み（このリポジトリで `cargo build`）
2. **Roo CLI** をインストール済み（`roo --version` が 0.1.15 以降を返すこと）
3. **AWS Bedrock** のアクセスを設定済み（または他のサポートされるプロバイダ）

### 最初のループを実行する

```bash
# 単純な 1 イテレーションのテスト
ralph run -b roo --max-iterations 1 \
  -- --provider bedrock --aws-profile roo-bedrock --aws-region us-east-1 \
     --model anthropic.claude-sonnet-4-6 --max-tokens 64000 \
  -p "Create a hello.txt file with 'Hello World'"
```

## 設定

### オプション 1: CLI フラグ（手早い）

roo 固有のフラグを `--` の後に渡します。

```bash
ralph run -b roo -- \
  --provider bedrock \
  --aws-profile roo-bedrock \
  --aws-region us-east-1 \
  --model anthropic.claude-sonnet-4-6 \
  --max-tokens 64000
```

### オプション 2: 設定ファイル（推奨）

`ralph.roo.yml` を作成します。

```yaml
# Ralph + Roo Configuration
event_loop:
  completion_promise: "LOOP_COMPLETE"
  max_iterations: 100
  max_runtime_seconds: 14400      # 4 hours
  max_consecutive_failures: 5

cli:
  backend: "roo"
  prompt_mode: "arg"
  pty_mode: false
  idle_timeout_secs: 30
  args:
    - "--provider"
    - "bedrock"
    - "--aws-profile"
    - "roo-bedrock"
    - "--aws-region"
    - "us-east-1"
    - "--model"
    - "anthropic.claude-sonnet-4-6"
    - "--max-tokens"
    - "100000"
    - "--reasoning-effort"
    - "medium"

core:
  specs_dir: ".ralph/specs/"
  guardrails:
    - "Fresh context each iteration - save learnings to memories for next time"
    - "Don't assume 'not implemented' - search first"
    - "Verification is mandatory - tests/typecheck/lint/audit must pass"
    - "Confidence protocol: score decisions 0-100. >80 proceed autonomously; 50-80 proceed + document; <50 choose safe default + document."

hats:
  builder:
    name: "Builder"
    description: "Implements code, creates files, runs tests. Does the actual work."
    triggers: ["build.task"]
    publishes: ["build.done", "build.blocked"]
    instructions: |
      ## WORKFLOW
      You are Builder. Your job is to IMPLEMENT - write code, create files, run tests.
      1. Read the build.task event payload - that's your task
      2. IMPLEMENT: Create files, write code, run commands
      3. VERIFY: Run tests/builds to confirm it works
      4. COMPLETE: Emit build.done when verified, or build.blocked if stuck
      RULES:
      - Do the actual work - don't just plan or delegate
      - Never emit build.task (that's for coordination, not you)
```

そして実行します。

```bash
ralph run -c ralph.roo.yml -p "Build feature X"
```

### オプション 3: Roo での PDD-to-Code-Assist

Claude Opus 4.6 を使う Roo で完全な PDD → Code Assist ワークフローを行うには:

`ralph.roo.pdd.yml` を作成します。

```yaml
# PDD-to-Code-Assist with Roo Code CLI
# Uses Claude Opus 4.6 via Bedrock with medium reasoning effort

event_loop:
  prompt_file: "PROMPT.md"
  completion_promise: "LOOP_COMPLETE"
  starting_event: "design.start"
  max_iterations: 150
  max_runtime_seconds: 14400
  checkpoint_interval: 5

cli:
  backend: "roo"
  prompt_mode: "arg"
  pty_mode: false
  idle_timeout_secs: 60
  args:
    - "--provider"
    - "bedrock"
    - "--aws-profile"
    - "roo-bedrock"
    - "--aws-region"
    - "us-east-1"
    - "--model"
    - "anthropic.claude-opus-4-6"
    - "--max-tokens"
    - "100000"
    - "--reasoning-effort"
    - "medium"

core:
  specs_dir: ".ralph/specs/"
  guardrails:
    - "Fresh context each iteration — save learnings to memories for next time"
    - "Verification is mandatory — tests/typecheck/lint/audit must pass"
    - "YAGNI ruthlessly — no speculative features"
    - "KISS always — simplest solution that works"
    - "Preserve primary sources — all referenced files, research findings, code snippets, and external docs must be captured with source attribution"
    - "Confidence protocol: score decisions 0-100. >80 proceed autonomously; 50-80 proceed + document in .ralph/agent/decisions.md; <50 choose safe default + document."

# Copy hats from presets/pdd-to-code-assist.yml
# (inquisitor, architect, design_critic, explorer, planner, task_writer, builder, validator, committer)
```

そして実行します。

```bash
ralph run -c ralph.roo.pdd.yml -p "Build a CLI tool for managing tasks"
```

または、組み込みプリセットを roo の引数とともに使います。

```bash
ralph run -c presets/pdd-to-code-assist.yml \
  -c cli.backend=roo \
  -- --provider bedrock --aws-profile roo-bedrock --aws-region us-east-1 \
     --model anthropic.claude-opus-4-6 --max-tokens 100000 \
     --reasoning-effort medium \
  -p "Build a CLI tool for managing tasks"
```

## Roo 固有の設定オプション

### モデルの選択

モデルのフラグを `cli.args` または `--` で渡します。

| フラグ | 説明 | 例 |
|------|-------------|---------|
| `--provider` | LLM プロバイダ | `bedrock`, `anthropic`, `openai`, `openrouter` |
| `--model` | モデル識別子 | `anthropic.claude-opus-4-6`, `anthropic.claude-sonnet-4-6` |
| `--max-tokens` | 最大出力トークン | `100000` |
| `--reasoning-effort` | 思考の努力度 | `medium`, `high`, `xhigh` |
| `--aws-profile` | AWS 認証情報プロファイル | `roo-bedrock` |
| `--aws-region` | AWS Bedrock リージョン | `us-east-1` |

### Roo のモード

Roo には組み込みのモード（`code`, `architect`, `ask`, `debug`）があります。既定では、
すべてのツール（read, edit, command, mcp）を持つ `code` モードを使います。次で上書き
します。

```yaml
cli:
  args:
    - "--mode"
    - "architect"  # For planning-focused hats
```

### 対話的な計画

Roo の TUI との対話的セッションには `ralph plan` を使います。

```bash
ralph plan -b roo -- --provider bedrock --aws-profile roo-bedrock \
  --aws-region us-east-1 --model anthropic.claude-opus-4-6 \
  --max-tokens 100000 \
  -p "Design the auth system architecture"
```

## 仕組み

### アーキテクチャ

```
Ralph ループ（各イテレーション）:
1. Ralph がプロンプトを構築する（コンテキスト + イベント + メモリ + 指示）
2. プロンプトを一時ファイルに書き込む
3. spawn: roo --print --ephemeral --prompt-file /tmp/xxx [user args]
4. Roo がプロンプトを読み、ツールを実行し、テキスト出力を生成する
5. Ralph が出力からイベント（<event topic="...">）と LOOP_COMPLETE を解析する
6. 更新されたコンテキストで次のイテレーション
```

### 主な挙動

| 観点 | 挙動 |
|--------|----------|
| **コンテキスト** | 各イテレーションでフレッシュ — roo はイテレーション間の記憶を持たない |
| **ツール承認** | 既定で自動承認（フラグ不要） |
| **ディスク状態** | `--ephemeral` がイテレーション間でディスクをクリーンに保つ |
| **プロンプトの受け渡し** | 常に `--prompt-file` 経由（任意のプロンプトサイズを扱う） |
| **エラー検出** | LOOP_COMPLETE の有無 + 連続失敗カウンタ |
| **終了コード** | 設定エラー → exit 1; API エラー → 無限リトライ（アイドルタイムアウトが処理） |

## トラブルシューティング

### Bedrock のクロスリージョンエラー

「Try enabling cross-region inference」と表示される場合:
1. `--aws-region` が Bedrock のセットアップと一致することを確認する
2. `--aws-profile` に正しい認証情報があることを確認する
3. モデルがそのリージョンで利用可能か検証する

### Roo が無限にリトライする

Roo は API エラーを指数バックオフでリトライします。Ralph の `idle_timeout_secs`（既定
30 秒）がプロセスを終了させます。モデルが遅い場合は増やします。

```yaml
cli:
  idle_timeout_secs: 60  # 大きなプロンプトではさらに大きく
```

### CustomModesManager の警告

```
[CustomModesManager] Failed to load modes from .../custom_modes.yaml: ENOENT
```

これは無害です。`--ephemeral` モードは、カスタムモードが存在しない一時ディレクトリを
使います。対応は不要です。
