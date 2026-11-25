# CLI リファレンス

Ralph のコマンドラインインターフェースの完全なリファレンスです。

## グローバルオプション

これらのオプションはすべてのコマンドで受け付けられます。

| オプション | 説明 |
|--------|-------------|
| `-c, --config <SOURCE>` | 主要な設定ソース（複数指定可能）。既定は `ralph.yml`、または設定されていれば `$RALPH_CONFIG`。 |
| `-H, --hats <SOURCE>` | ハットコレクションのソース（`file`、`builtin:<name>`、または URL）。 |
| `-v, --verbose` | 詳細な出力 |
| `--color <MODE>` | 色付き出力: `auto`, `always`, `never` |
| `-h, --help` | ヘルプを表示する |
| `-V, --version` | バージョンを表示する |

### コア設定ソース（`-c`）

`-c` フラグは、**コア**設定をどこから読み込むかを指定します。指定しない場合、`ralph` は
次にフォールバックします。

1. 存在すれば `$RALPH_CONFIG`
2. `ralph.yml`

**コアソースの種類:**

| 形式 | 説明 |
|--------|-------------|
| `ralph.yml` | ローカルのファイルパス |
| `https://example.com/ralph.core.yml` | リモート URL |
| `core.field=value` | コア設定の上書き |

> `-c builtin:<name>` はサポートされなくなりました。ハットコレクションには
> `-H builtin:<name>` を使ってください。

上書きでない最初のコアソースがベース設定として使われます。後のコア上書きが、先の値を
置き換えます。

後方互換性: `-c` の設定ファイルは、`hats`/`events` を含んでいてもかまいません（単一
ファイルの結合設定）。

`-H/--hats` が指定された場合、それが `-c` のハットより優先されます。
- `-H` の `hats` と `events` が `-c` の `hats`/`events` を置き換える
- `-H` の `event_loop` の値が、`-c` の一致する `event_loop` キーを上書きする
- `-c core.*=...` の上書きは最後に適用される

**サポートされる上書きフィールド:**

| フィールド | 説明 |
|-------|-------------|
| `core.scratchpad` | スクラッチパッドファイルへのパス（`scratchpad.path` の文字列の省略形） |
| `core.specs_dir` | specs ディレクトリへのパス |

### ハットコレクションソース（`-H`）

`-H` フラグは、ハットコレクションをどこから読み込むかを指定します。

| 形式 | 説明 |
|--------|-------------|
| `hats/feature.yml` | ローカルのハットファイル |
| `builtin:code-assist` | 組み込みのハットコレクション |
| `https://example.com/hats.yml` | リモートのハットファイル |

**例:**

```bash
# コアのみ（ハットなし）
ralph run -c ralph.yml

# コア + 組み込みハットコレクション
ralph run -c ralph.yml -H builtin:code-assist

# コア + ファイルのハットコレクション
ralph run -c ralph.yml -H hats/review.yml

# コア上書き + ハット
ralph run -c ralph.yml -c core.specs_dir=./my-specs -H builtin:debug
```

## コマンド

### ralph run

オーケストレーションループを実行します。

```bash
ralph run [OPTIONS]
```

**オプション:**

| オプション | 説明 |
|--------|-------------|
| `-p, --prompt <TEXT>` | インラインのプロンプトテキスト |
| `-P, --prompt-file <FILE>` | プロンプトファイルのパス |
| `--max-iterations <N>` | 最大イテレーションを上書きする |
| `--completion-promise <TEXT>` | 完了トリガーを上書きする |
| `--dry-run` | 何が実行されるかを表示する |
| `--no-tui` | TUI モードを無効にする |
| `-a, --autonomous` | ヘッドレスモードを強制する |
| `--idle-timeout <SECS>` | TUI のアイドルタイムアウト |
| `--exclusive` | 主ループのスロットを待つ |
| `--no-auto-merge` | ワークツリーループ完了後の自動マージをスキップする |
| `--skip-preflight` | 自動の事前確認をスキップする（`features.preflight.enabled: true` でも） |
| `--record-session <FILE>` | セッションを JSONL に記録する |
| `-q, --quiet` | ストリーミング出力を抑制する |
| `--continue` | 既存の状態から再開する |

### ralph init

`ralph.yml` を初期化します。

```bash
ralph init [OPTIONS]
```

**オプション:**

| オプション | 説明 |
|--------|-------------|
| `--backend <NAME>` | バックエンド: `claude`, `kiro`, `gemini`, `codex`, `forge`, `amp`, `copilot`, `opencode`, `pi`, `custom` |
| `--preset <NAME>` | 削除済み（モノリシックなプリセットはサポートされなくなった） |
| `--list-presets` | 利用可能な組み込みハットコレクションを一覧する |
| `--force` | 既存の設定を上書きする |

### ralph preflight

事前確認のチェックスイートを実行します。

```bash
ralph preflight [OPTIONS]
```

**オプション:**

| オプション | 説明 |
|--------|-------------|
| `--format <human|json>` | 出力形式 |
| `--strict` | 警告を失敗として扱う |
| `--check <NAME>` | 1 つ以上のチェックを名前で実行する |

既定のチェック名:

- `config`
- `hooks`
- `backend`
- `telegram`
- `git`
- `paths`
- `tools`
- `specs`

メモ:

- `--check` は繰り返せる（例: `--check hooks --check config`）。
- `--strict` は警告があると失敗する（失敗だけでなく）。
- `ralph run` 中、自動事前確認は `features.preflight.skip` を使ってこれらの名前のチェックを
  スキップする。

### ralph hooks

ループの実行を開始せずに、フックの設定とコマンドの配線を検証します。

```bash
ralph hooks <COMMAND>
```

**サブコマンド:**

- `validate [--format human|json]`

`ralph hooks validate` の挙動:

- 終了コード `0`: 検証に合格。
- 終了コード `1`: 1 つ以上の診断（または設定の読み込み/解析の失敗）。
- `--format human`（既定）: 診断付きの読みやすいレポート。
- `--format json`: 構造化されたレポート（`pass`, `source`, `hooks_enabled`,
  `checked_hooks`, `diagnostics`）。

最小のサンプルフック設定に対して試せます。

- `ralph hooks validate -c examples/hooks/minimal/ralph.hooks.yml`
- 設定: [`examples/hooks/minimal/ralph.hooks.yml`](https://github.com/mikeyobrien/ralph-orchestrator/blob/main/examples/hooks/minimal/ralph.hooks.yml)
- スクリプト: [`examples/hooks/scripts/env-guard.sh`](https://github.com/mikeyobrien/ralph-orchestrator/blob/main/examples/hooks/scripts/env-guard.sh), [`examples/hooks/scripts/notify.sh`](https://github.com/mikeyobrien/ralph-orchestrator/blob/main/examples/hooks/scripts/notify.sh)

### ralph doctor

環境と初回実行の診断チェックを実行します。

```bash
ralph doctor [OPTIONS]
```

### ralph tutorial

対話的な入門のウォークスルーを実行します。

```bash
ralph tutorial [OPTIONS]
```

### ralph plan

対話的な PDD 計画セッションを開始します。

```bash
ralph plan [OPTIONS] [IDEA]
```

**オプション:**

| オプション | 説明 |
|--------|-------------|
| `<IDEA>` | 任意のおおまかなアイデア |
| `-b, --backend <BACKEND>` | バックエンドの上書き |
| `--teams` | Claude Code のエージェントチームモードを有効にする |
| `-- <ARGUMENTS>` | カスタムのバックエンド引数 |

### ralph code-task

説明または PDD 計画からコードタスクファイルを生成します。

```bash
ralph code-task [OPTIONS] [INPUT]
```

### ralph task

`ralph code-task` の非推奨のレガシーエイリアス。

```bash
ralph task [OPTIONS] [INPUT]
```

### ralph events

現在または選択した実行のイベント履歴を表示します。

```bash
ralph events [OPTIONS]
```

**オプション:**

| オプション | 説明 |
|--------|-------------|
| `--file <PATH>` | 特定のイベントファイルを使う |
| `--clear` | イベント履歴をクリアする |

### ralph emit

現在の実行のイベントファイルにイベントを発行します。

```bash
ralph emit <TOPIC> [PAYLOAD] [OPTIONS]
```

**オプション:**

| オプション | 説明 |
|--------|-------------|
| `<TOPIC>` | イベントトピック（例: `build.done`） |
| `[PAYLOAD]` | 任意のペイロード（`--json` 設定時は文字列または JSON） |
| `-j, --json` | ペイロードを JSON オブジェクトとして解析する |
| `--ts <TIMESTAMP>` | イベントのタイムスタンプを上書きする |
| `--file <PATH>` | イベントファイルのパス（`.ralph/events.jsonl`） |

### ralph clean

`.ralph/agent` のスクラッチパッドとメモリ状態をクリーンにします。

```bash
ralph clean [OPTIONS]
```

**オプション:**

| オプション | 説明 |
|--------|-------------|
| `--diagnostics` | 診断ディレクトリをクリーンにする |
| `--dry-run` | 削除をプレビューする |

### ralph loops

並列ループとワークツリーループのライフサイクルを管理します。

```bash
ralph loops [OPTIONS] [COMMAND]
```

**サブコマンド:**

- `list [--json] [--all]`
- `logs <loop-id> [--follow]`
- `history <loop-id> [--json]`
- `retry <loop-id>`
- `discard <loop-id> [--yes]`
- `stop [loop-id] [--force]`
- `resume <loop-id>`
- `prune`
- `attach <loop-id>`
- `diff <loop-id> [--stat]`
- `publish-review <loop-id> [--remote <remote>] [--remote-branch <branch>] [--base <ref>] [--summary <path>]`
- `rebase [loop-id] [--base <ref>] [--remote <remote>] [--no-fetch] [--push]`
- `merge <loop-id> [--force]`
- `process`
- `merge-button-state <loop-id>`

`ralph loops resume <loop-id>` は、一時停止したループに再開シグナルを書き込みます。冪等です。
コマンドを再実行すると、再開が既に要求済みである（またはループが一時停止していない）ことを
報告します。

`ralph loops publish-review <loop-id>` は `ralph/<loop-id>` をリモートのレビューブランチに
プッシュし、ローカルの `.ralph/reviews/<loop-id>.md` サマリを書き込みます。
`ralph loops rebase` は、1 つのループブランチ、またはキュー待ち/needs-review かつ実行中で
ない `ralph/*` ワークツリーブランチのすべてを、選択したベースにマージせずにリベースします。

### ralph hats

設定されたハットを管理・検査します。

```bash
ralph hats [OPTIONS] [COMMAND]
```

**サブコマンド:**

- `list [--format table|json]`
- `show <name>`
- `validate`
- `graph [--format unicode|ascii|compact|mermaid] [--backend <backend>]`

### ralph web

Web ダッシュボードを実行します。

```bash
ralph web [OPTIONS]
```

**オプション:**

| オプション | 説明 |
|--------|-------------|
| `--backend-port <BACKEND_PORT>` | RPC API のポート（既定: 3000） |
| `--frontend-port <FRONTEND_PORT>` | フロントエンドのポート（既定: 5173） |
| `--workspace <WORKSPACE>` | ワークスペースのルート |
| `--legacy-node-api` | Rust RPC API の代わりに非推奨の Node tRPC バックエンドを実行する |
| `--no-open` | ブラウザを開かない |

### ralph mcp

Ralph を `stdio` 上の Model Context Protocol サーバーとして実行します。

```bash
ralph mcp serve
```

メモ:

- v1 はツールのみで `stdio` のみ。
- 対話的なターミナルワークフローではなく、MCP クライアントの設定から起動する。
- サーバーは、`stream_next` のようなポーリングストリームツールを含む、Ralph の
  コントロールプレーンのメソッドを MCP ツールとして公開する。

### ralph bot

Telegram ボットのセットアップとテストを管理します。

```bash
ralph bot [OPTIONS] <COMMAND>
```

**サブコマンド:**

- `onboard [--token <TOKEN>] [--chat-id <CHAT_ID>] [--timeout <SECONDS>]`
- `status`
- `test [MESSAGE]`
- `token set <TOKEN> [--config <path>]`
- `daemon`

### ralph wave

並列ハット実行のための波イベントをディスパッチします。

```bash
ralph wave emit <TOPIC> --payloads <ITEM>...
```

**オプション:**

| オプション | 説明 |
|--------|-------------|
| `<TOPIC>` | 波対応のハットを対象とするイベントトピック |
| `--payloads <ITEM>...` | 1 つ以上のペイロード。それぞれが別個のイベントになる |

各ペイロードは、共有の `wave_id` でタグ付けされたイベントになります。ループランナーは、
対象ハットの `concurrency` 設定で上限を設けた並列のバックエンドインスタンスを spawn します。

`RALPH_WAVE_WORKER=1` のときはブロックされます（ネストした波を防ぐ）。

詳細は [エージェント波](../advanced/agent-waves.ja.md) を参照してください。

### ralph tools

メモリ、タスク、スキルのためのランタイムツール。

#### ralph tools memory

```bash
ralph tools memory <SUBCOMMAND>
```

**サブコマンド:**

| コマンド | 説明 |
|---------|-------------|
| `init` | メモリファイルを初期化する |
| `add <CONTENT>` | 新しいメモリを保存する |
| `search <QUERY>` | メモリを検索する |
| `list` | メモリを一覧する |
| `show <ID>` | メモリを表示する |
| `delete <ID>` | メモリを削除する |
| `prime` | コンテキストのメモリ出力をプライムする |

#### ralph tools task

```bash
ralph tools task <SUBCOMMAND>
```

**サブコマンド:**

| コマンド | 説明 |
|---------|-------------|
| `add <TITLE>` | タスクを作成する |
| `list` | すべてのタスクを一覧する |
| `ready` | ブロックされていないタスクを一覧する |
| `close <ID>` | タスクを完了としてマークする |
| `fail <ID>` | タスクを失敗としてマークする |
| `show <ID>` | タスクの詳細を表示する |

#### ralph tools skill

```bash
ralph tools skill <SUBCOMMAND>
```

#### ralph tools interact

Telegram の進捗/能動的なフックを通じて人間とやり取りします。

### ralph completions

シェル補完を生成します。

```bash
ralph completions <SHELL>
```

サポートされるシェル: `bash`, `elvish`, `fish`, `powershell`, `zsh`。

## 終了コード

| コード | 意味 |
|------|---------|
| 0 | 完了の約束に到達（`LOOP_COMPLETE`） |
| 1 | 失敗または停止条件（失敗/キャンセル/スロットルの状態） |
| 2 | ランタイム上限に到達（`max-iterations`, `max-runtime`, または `max-cost`） |
| 3 | ループが再起動を要求 |
| 130 | シグナルによる割り込み（Ctrl-C / SIGINT） |

## 環境変数

| 変数 | 説明 |
|----------|-------------|
| `RALPH_DIAGNOSTICS` | `1` に設定すると診断を有効にする |
| `RALPH_CONFIG` | 既定の設定ファイルのパス |
| `NO_COLOR` | 色付き出力を無効にする |
| `RALPH_WAVE_WORKER` | 波ワーカー内で `1` に設定される（ネストした波をブロックする） |
| `RALPH_WAVE_ID` | 波の相関 ID（波ワーカーで設定される） |
| `RALPH_WAVE_INDEX` | 波の中の 0 始まりのワーカーインデックス |
| `RALPH_EVENTS_FILE` | ワーカーごとのイベントファイルのパス（波ワーカーで設定される） |

## シェル補完

シェル補完を生成します。

```bash
# Bash
ralph completions bash > ~/.local/share/bash-completion/completions/ralph

# Zsh
ralph completions zsh > ~/.zfunc/_ralph

# Fish
ralph completions fish > ~/.config/fish/completions/ralph.fish
```
