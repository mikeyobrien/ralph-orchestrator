# 設定

Ralph の YAML 設定の完全なリファレンスです。

## 設定ファイル

Ralph は、最大 3 つのレイヤーから設定を構成します。

1. 存在すれば `~/.ralph/config.yml` — 自動的に読み込まれるユーザーレベルの既定値
2. 現在のワークスペースの `ralph.yml`（または `$RALPH_CONFIG` / `-c <file>`）— プロジェクト
   レベルの上書き
3. `-c core.field=value` の上書き — 最後に適用される

プロジェクト設定は、ディープマージによってユーザー設定の上に重ねられます。マッピングは
再帰的にマージされ、プロジェクト設定のスカラー値や配列がユーザーレベルの値を置き換えます。

```bash
# ワークスペース設定を使う（存在すれば ~/.ralph/config.yml も自動的にマージする）
ralph run

# プロジェクト設定のパスを上書きする
RALPH_CONFIG=/path/to/config.yml ralph run ...
ralph run -c custom-config.yml
```

### ユーザーレベルの設定（`~/.ralph/config.yml`）

共有のバックエンド設定、グローバルなライフサイクルフック、組織全体のガードレールなど、
どこでも欲しい既定値には `~/.ralph/config.yml` を使います。

よくあるパターンは、通知フックをグローバルに保ちつつ、プロジェクト固有の自動化はリポジトリ
ローカルの `ralph.yml` に残すことです。

```yaml
# ~/.ralph/config.yml
hooks:
  enabled: true
  events:
    post.loop.complete:
      - name: notify-success
        command: ["./scripts/notify.sh", "complete"]
        on_error: warn
    post.loop.error:
      - name: notify-failure
        command: ["./scripts/notify.sh", "error"]
        on_error: warn
```

```yaml
# ./ralph.yml
hooks:
  events:
    pre.loop.start:
      - name: env-guard
        command: ["./scripts/hooks/env-guard.sh"]
        on_error: block
```

この 2 つのファイルがあると、Ralph は両方を読み込み、検証と実行の前にディープマージします。

## MCP ワークスペースの解決

`ralph mcp serve` は、次の順でワークスペースルートを解決します。

1. `--workspace-root <path>`
2. `RALPH_API_WORKSPACE_ROOT`
3. 現在の作業ディレクトリ

ワークスペース/リポジトリごとに MCP サーバーインスタンスを 1 つ使います。Ralph の現在の
コントロールプレーン API はワークスペーススコープです。`config.*`、`task.*`、`loop.*`、
`planning.*`、`collection.*` はすべて、単一のルート配下で状態を読み書きします。

## CLI 設定の上書き

別の設定ファイルを作らずに、コマンドラインから特定のコアフィールドを上書きできます。次の
場合に便利です。

- 分離されたスクラッチパッドで並列の Ralph インスタンスを実行する
- 異なる specs ディレクトリでテストする
- 動的なパスを持つ CI/CD パイプライン

**構文:** `-c core.field=value`

**サポートされるフィールド:**

| フィールド | 説明 |
|-------|-------------|
| `core.scratchpad` | スクラッチパッドファイルへのパス（`scratchpad.path` の文字列の省略形） |
| `core.specs_dir` | specs ディレクトリへのパス |

**例:**

```bash
# スクラッチパッドを上書きする（ralph.yml を読み込み、上書きを適用する）
ralph run -c core.scratchpad=.ralph/agent/feature-auth/scratchpad.md

# 明示的な設定 + 上書き
ralph run -c ralph.yml -c core.scratchpad=.ralph/agent/feature-auth/scratchpad.md

# 複数の上書き
ralph run -c core.scratchpad=.runs/task-1/scratchpad.md -c core.specs_dir=./custom-specs/
```

上書きは `ralph.yml` の読み込み後に適用されるため、優先されます。スクラッチパッドの
ディレクトリは、存在しなければ自動作成されます。

## 結合設定の互換性（`-c` + `-H`）

Ralph は両方のスタイルをサポートします。
- **単一ファイルの結合設定**: 1 つのファイルにコア + ハットを持つ `-c ralph.yml`
- **分割設定**: `-c <core>` に加えて `-H <hats source>`

両方が使われた場合（`-c` にハットが含まれ、`-H` が指定された場合）、ワークフローの
セクションは `-H` が勝ちます。
- `-H` の `hats` と `events` が `-c` の `hats`/`events` を置き換える
- `-H` の `event_loop` の値が、`-c` の一致する `event_loop` キーを上書きする
- `-c core.*=...` の上書きは引き続き最後に適用される

## 完全な設定リファレンス

```yaml
# イベントループの設定
event_loop:
  completion_promise: "LOOP_COMPLETE"  # 完了を知らせる出力
  max_iterations: 100                   # 最大オーケストレーションループ
  max_runtime_seconds: 14400            # 最大実行時間 4 時間
  idle_timeout_secs: 1800               # 30 分のアイドルタイムアウト
  starting_event: "task.start"          # 最初に公開されるイベント（ハットモード）
  checkpoint_interval: 5                # Git チェックポイントの頻度
  prompt_file: "PROMPT.md"              # 既定のプロンプトファイル

# CLI バックエンドの設定
cli:
  backend: "claude"                     # バックエンド名
  prompt_mode: "arg"                    # arg または stdin

# コアの挙動
core:
  scratchpad:                            # スクラッチパッドの設定
    enabled: true                        # スクラッチパッドを有効にする（既定: true）
    path: .ralph/agent/scratchpad.md     # スクラッチパッドファイルのパス
  specs_dir: ".ralph/specs/"             # コミットされる仕様のディレクトリ
  guardrails:                            # すべてのプロンプトに注入されるルール
    - "Fresh context each iteration"
    - "Never modify production database"

# メモリ — 永続的な学習
memories:
  enabled: true                         # メモリシステムを有効にする
  inject: auto                          # auto, manual, none
  budget: 2000                          # 注入する最大トークン
  filter:
    types: []                           # メモリの種類で絞り込む
    tags: []                            # メモリのタグで絞り込む
    recent: 0                           # 日数制限（0 = 制限なし）

# タスク — ランタイムの作業追跡
tasks:
  enabled: true                         # タスクシステムを有効にする

# 任意の機能
features:
  parallel: true                        # 主ロックが保持されているときワークツリーループを許可する
  auto_merge: false                     # 完了時にワークツリーループを自動マージする
  preflight:
    enabled: false                      # `ralph run` で自動的に事前確認を実行する
    strict: false                       # 警告を失敗として扱う
    skip: []                            # 名前でチェックをスキップする（例: ["hooks"]）

# ライフサイクルフック（v1）
hooks:
  enabled: false
  defaults:
    timeout_seconds: 30
    max_output_bytes: 8192
    suspend_mode: wait_for_resume
  events:
    pre.loop.start:
      - name: env-guard
        command: ["./scripts/hooks/env-guard.sh"]
        on_error: block
        mutate:
          enabled: false

# ハット — 専門ペルソナ
hats:
  my_hat:
    name: "My Hat"                      # 表示名
    description: "Purpose"              # 任意の説明
    triggers: ["event.*"]               # 購読パターン
    publishes: ["event.done"]           # 許可されるイベント種別
    default_publishes: "event.done"     # 明示がないときの既定
    max_activations: 10                 # 起動回数の上限
    backend: "claude"                   # バックエンドの上書き
    scratchpad:                         # ハットごとのスクラッチパッド上書き
      enabled: true                     #   スクラッチパッドを有効にする（既定: true）
      path: .ralph/agent/my-hat.md      #   スクラッチパッドファイルのパス。省略時はコアを継承。
    instructions: |
      Hat-specific instructions...
```

## セクションの詳細

### event_loop

オーケストレーションループの挙動を制御します。

| オプション | 型 | 既定 | 説明 |
|--------|------|---------|-------------|
| `completion_promise` | string | `"LOOP_COMPLETE"` | ループを終わらせる出力テキスト |
| `max_iterations` | integer | `100` | 停止までの最大イテレーション |
| `max_runtime_seconds` | integer | `14400` | 最大実行時間（4 時間） |
| `idle_timeout_secs` | integer | `1800` | アイドルタイムアウト（30 分） |
| `starting_event` | string | `null` | 最初のイベント（ハットモードを有効化） |
| `checkpoint_interval` | integer | `5` | Git チェックポイントの頻度 |
| `prompt_file` | string | `"PROMPT.md"` | 既定のプロンプトファイル |

### cli

バックエンドの設定。

| オプション | 型 | 既定 | 説明 |
|--------|------|---------|-------------|
| `backend` | string | 自動検出 | バックエンド名 |
| `prompt_mode` | string | `"arg"` | プロンプトの渡し方 |

**backend の値:**
- `claude` — Claude Code
- `kiro` — Kiro
- `gemini` — Gemini CLI
- `codex` — Codex
- `forge` — Forge
- `amp` — Amp
- `copilot` — Copilot CLI
- `opencode` — OpenCode
- `pi` — Pi
- `custom` — カスタムのアダプタ/バックエンド

**prompt_mode の値:**
- `arg` — CLI 引数として渡す: `cli -p "prompt"`
- `stdin` — stdin 経由で渡す: `echo "prompt" | cli`

### core

コアの挙動、スクラッチパッド、ガードレール。

| オプション | 型 | 既定 | 説明 |
|--------|------|---------|-------------|
| `scratchpad` | string または object | `{ enabled: true, path: ".ralph/agent/scratchpad.md" }` | スクラッチパッドの設定（下記参照） |
| `scratchpad.enabled` | boolean | `true` | スクラッチパッドを有効にする |
| `scratchpad.path` | string | `".ralph/agent/scratchpad.md"` | スクラッチパッドファイルのパス |
| `specs_dir` | string | `".ralph/specs/"` | コミットされる仕様のディレクトリ |
| `guardrails` | list | `[]` | すべてのプロンプトに注入されるルール |

`scratchpad` フィールドは、素の文字列（`enabled: true` で `path` を設定する省略形）、または
`enabled` と `path` を持つ構造化オブジェクトを受け付けます。

```yaml
# 文字列の省略形 — path を設定し、enabled は既定で true
core:
  scratchpad: ".workspace/plan.md"

# 構造化オブジェクト — 完全な制御
core:
  scratchpad:
    enabled: true
    path: .ralph/agent/scratchpad.md
```

> **ソロモードの安全性:** スクラッチパッドが無効（`enabled: false`）でハットが定義されて
> いない場合、Ralph は警告とともに強制的に有効化します。スクラッチパッドは、ソロモードでの
> 唯一の連続性の仕組みです。

### memories

セッションをまたいだ永続的な学習。

| オプション | 型 | 既定 | 説明 |
|--------|------|---------|-------------|
| `enabled` | boolean | `true` | メモリシステムを有効にする |
| `inject` | string | `"auto"` | 注入モード |
| `budget` | integer | `2000` | 注入する最大トークン |
| `filter.types` | list | `[]` | メモリの種類で絞り込む |
| `filter.tags` | list | `[]` | タグで絞り込む |
| `filter.recent` | integer | `0` | 日数制限 |

**注入モード:**
- `auto` — イテレーション開始時に自動的に注入する
- `manual` — エージェントが `ralph tools memory prime` を呼ぶ必要がある
- `none` — 注入しない

### tasks

ランタイムの作業追跡。

| オプション | 型 | 既定 | 説明 |
|--------|------|---------|-------------|
| `enabled` | boolean | `true` | タスクシステムを有効にする |

### features

任意のランタイム機能。

| オプション | 型 | 既定 | 説明 |
|--------|------|---------|-------------|
| `parallel` | boolean | `true` | 別のループが主ロックを保持しているときワークツリーループを spawn する |
| `auto_merge` | boolean | `false` | 完了したワークツリーループを自動マージする |
| `preflight.enabled` | boolean | `false` | `ralph run` の前に `ralph preflight` チェックを自動実行する |
| `preflight.strict` | boolean | `false` | 事前確認の警告を失敗として扱う |
| `preflight.skip` | list | `[]` | 名前でチェックをスキップする（例: `hooks`, `git`） |

`features.preflight.enabled: true` のとき、`ralph run` は既定の事前確認スイートを使います。
`config`、`hooks`、`backend`、`telegram`、`git`、`paths`、`tools`、`specs`。

### hooks

オーケストレーターのフェーズイベント向けのライフサイクルフック（v1）。

フックは、ユーザーレベルの `~/.ralph/config.yml` またはワークスペースの `ralph.yml` の
どちらでも定義できます。Ralph はまずユーザー設定を読み込み、その上にプロジェクト設定を
重ねます。つまり、ユーザー設定のフックは、プロジェクト設定が同じイベントマッピングを
置き換えない限り、グローバルに適用されます。

| オプション | 型 | 既定 | 説明 |
|--------|------|---------|-------------|
| `enabled` | boolean | `false` | ライフサイクルイベントのフックディスパッチを有効にする |
| `defaults.timeout_seconds` | integer | `30` | フックごとの既定タイムアウト（秒） |
| `defaults.max_output_bytes` | integer | `8192` | ストリームごとの既定の stdout/stderr の上限 |
| `defaults.suspend_mode` | enum | `wait_for_resume` | `on_error: suspend` の既定のサスペンドモード |
| `events` | map | `{}` | ライフサイクルのフェーズイベントキーからフック仕様のリストへのマッピング |

`hooks.events` 配下でサポートされる v1 のライフサイクルフェーズイベントキー:

- `pre.loop.start`, `post.loop.start`
- `pre.iteration.start`, `post.iteration.start`
- `pre.plan.created`, `post.plan.created`
- `pre.human.interact`, `post.human.interact`
- `pre.loop.complete`, `post.loop.complete`
- `pre.loop.error`, `post.loop.error`

フック仕様（`HookSpec`）のフィールド:

| フィールド | 必須 | 説明 |
|-------|----------|-------------|
| `name` | はい | テレメトリ/診断で使う安定した識別子 |
| `command` | はい | コマンドの argv 配列（`command[0]` は実行可能ファイルに解決されなければならない） |
| `cwd` | いいえ | 作業ディレクトリの上書き（絶対またはワークスペース相対） |
| `env` | いいえ | フックプロセスの環境変数の上書き |
| `timeout_seconds` | いいえ | フックごとのタイムアウト上書き（> 0 でなければならない） |
| `max_output_bytes` | いいえ | ストリームごとのフックの出力上限の上書き（> 0 でなければならない） |
| `on_error` | はい | 失敗時の処置: `warn`、`block`、または `suspend` |
| `suspend_mode` | いいえ | サスペンド戦略の上書き（`wait_for_resume`, `retry_backoff`, `wait_then_retry`） |
| `mutate.enabled` | いいえ | フックの stdout 変異解析をオプトインする（既定 `false`） |
| `mutate.format` | いいえ | 任意の形式ガードレール。v1 では `json` のみ許可 |

v1 の変異のスコープは、意図的に狭く保たれています。

- 変異解析は `mutate.enabled: true` のときのみ行われる。
- フックの stdout は v1 の契約を用いた JSON でなければならない: `{"metadata": { ... }}`。
- 許可されるのは metadata 名前空間の更新のみ（`metadata.accumulated.hook_metadata.<hook_name>`）。
- プロンプト/イベント/設定の変異は v1 のスコープ外。

最小の実行可能な例:

- 設定: [`examples/hooks/minimal/ralph.hooks.yml`](https://github.com/mikeyobrien/ralph-orchestrator/blob/main/examples/hooks/minimal/ralph.hooks.yml)
- スクリプト: [`examples/hooks/scripts/env-guard.sh`](https://github.com/mikeyobrien/ralph-orchestrator/blob/main/examples/hooks/scripts/env-guard.sh), [`examples/hooks/scripts/notify.sh`](https://github.com/mikeyobrien/ralph-orchestrator/blob/main/examples/hooks/scripts/notify.sh)
- 検証: `ralph hooks validate -c examples/hooks/minimal/ralph.hooks.yml`

### hats

ハットベースモード向けの専門ペルソナ。

| オプション | 型 | 必須 | 説明 |
|--------|------|----------|-------------|
| `name` | string | はい | 表示名 |
| `description` | string | いいえ | 目的の説明 |
| `triggers` | list | はい | イベント購読パターン |
| `publishes` | list | はい | 許可されるイベント種別 |
| `default_publishes` | string | いいえ | 明示がないときの既定イベント |
| `max_activations` | integer | いいえ | 起動回数を制限する |
| `backend` | string | いいえ | バックエンドの上書き |
| `scratchpad` | string または object | いいえ | ハットごとのスクラッチパッド上書き（省略時は `core.scratchpad` を継承） |
| `instructions` | string | はい | ハット固有のプロンプト |

各ハットは、独自の `scratchpad` フィールドでグローバルなスクラッチパッドを上書きできます。
コアレベルの設定と同様に、素の文字列または構造化オブジェクトを受け付けます。

```yaml
hats:
  planner:
    scratchpad: .ralph/agent/planner.md       # 文字列の省略形
    # ...
  builder:
    scratchpad:
      path: .ralph/agent/builder.md           # カスタムパスを持つ構造化
    # ...
  validator:
    scratchpad:
      enabled: false                          # スクラッチパッドを完全に無効にする
    # ...
  reviewer:                                   # scratchpad キーなし = グローバルを継承
    # ...
```

**解決順:** ハットの上書き → `core.scratchpad` → 既定。

## 設定例

### 従来型モード（最小）

```yaml
cli:
  backend: "claude"

event_loop:
  completion_promise: "LOOP_COMPLETE"
  max_iterations: 100
```

### ハットベースモード

```yaml
cli:
  backend: "claude"

event_loop:
  completion_promise: "LOOP_COMPLETE"
  max_iterations: 100
  starting_event: "task.start"

hats:
  planner:
    name: "Planner"
    triggers: ["task.start"]
    publishes: ["plan.ready"]
    instructions: |
      Create an implementation plan.

  builder:
    name: "Builder"
    triggers: ["plan.ready"]
    publishes: ["build.done"]
    instructions: |
      Implement the plan.
      Evidence required: tests pass.
```

### メモリを無効にした場合

```yaml
cli:
  backend: "claude"

event_loop:
  completion_promise: "LOOP_COMPLETE"

memories:
  enabled: false

tasks:
  enabled: false
```

### ハットごとのスクラッチパッド付き

```yaml
cli:
  backend: "claude"

event_loop:
  completion_promise: "LOOP_COMPLETE"
  starting_event: "task.start"

core:
  scratchpad:
    enabled: true
    path: .ralph/agent/scratchpad.md

hats:
  planner:
    name: "Planner"
    scratchpad:
      path: .ralph/agent/planner.md
    triggers: ["task.start"]
    publishes: ["plan.ready"]
    instructions: |
      Create an implementation plan.

  builder:
    name: "Builder"
    triggers: ["plan.ready"]
    publishes: ["build.done"]
    instructions: |
      Implement the plan.

  reviewer:
    name: "Reviewer"
    scratchpad:
      enabled: false
    triggers: ["build.done"]
    publishes: ["review.done"]
    instructions: |
      Review the implementation. No scratchpad needed.
```

### カスタムガードレール付き

```yaml
cli:
  backend: "claude"

event_loop:
  completion_promise: "LOOP_COMPLETE"

core:
  guardrails:
    - "Always run tests before declaring done"
    - "Never modify production database"
    - "Follow existing code patterns"
```

## 環境変数

| 変数 | 説明 |
|----------|-------------|
| `RALPH_CONFIG` | 既定の設定ファイルのパス |
| `RALPH_DIAGNOSTICS` | 診断を有効にする（`1`） |
| `NO_COLOR` | 色付き出力を無効にする |

## 次のステップ

- 事前設定済みのワークフローについて [プリセット](presets.ja.md) を探る
- [CLI リファレンス](cli-reference.ja.md) について学ぶ
- [バックエンド](backends.ja.md) を理解する
```
