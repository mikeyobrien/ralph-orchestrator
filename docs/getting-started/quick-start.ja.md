# クイックスタート

最初の Ralph オーケストレーションを約 10 分で実行します。

## 1. Ralph をインストールする

まだ Ralph をインストールしていない場合は、完全な [インストール](installation.ja.md)
ガイドに従ってください。

手早いインストール（npm）:

```bash
npm install -g @ralph-orchestrator/ralph-cli
```

## 2. バックエンド CLI をインストールする（Claude 推奨）

Ralph には、PATH 上で利用できる AI CLI ツールが少なくとも 1 つ必要です。

```bash
# Claude Code
npm install -g @anthropic-ai/claude-code

# CLI が利用できることを確認する
claude --version
```

バックエンドが認証を必要とする場合は、プロバイダの指示に従ってそのログインフローを
完了してください。

## 3. `ralph doctor` でセットアップを確認する

doctor コマンドを実行して環境を検証します。

```bash
ralph doctor
```

続行する前に、**WARN** や **FAIL** の項目を修正してください。認証の警告が表示される場合は、
バックエンド CLI にログインしているか確認してください。

## 4. プロジェクトを初期化する

```bash
mkdir my-ralph-project
cd my-ralph-project
git init  # Ralph は git があると最もうまく動く

# 既定の設定を作成する
ralph init --backend claude
```

これにより、プロジェクトに `ralph.yml` が作成されます。

## 5. 最小限のハットコレクションを作成する

Ralph は、より構造化されたワークフローのためにハット（役割ベースのペルソナ）で実行
できます。最小限のハットコレクションファイルを作成します。

```yaml
# hats.yml
event_loop:
  starting_event: "task.start"

hats:
  builder:
    name: "Builder"
    triggers: ["task.start"]
    publishes: ["task.done"]
    instructions: |
      Implement the task from PROMPT.md.
      Run any relevant tests.
      When finished, emit task.done and print LOOP_COMPLETE.
```

## 6. タスクを定義する

タスクを記した `PROMPT.md` ファイルを作成します。

```markdown
# Task: Create a Todo List CLI (Rust)

Build a Rust command-line todo list with:
- Add tasks
- List tasks
- Mark tasks complete
- Save to a JSON file

Include error handling and unit tests.
```

## 7. Ralph を実行する

```bash
# 従来型モード（ralph.yml を使う）
ralph run

# ハットベースモード（hats.yml を使う）
ralph run --config hats.yml

# インラインプロンプトの例
ralph run -p "Add input validation to the user API endpoints"
```

## 8. 出力を理解する

実行中、Ralph は次を表示する TUI を示します。

- 現在のイテレーション番号
- 経過時間
- アクティブなハット（ハットベースの場合）
- 最近のエージェント出力

Ralph は、次のいずれかが起きると停止します。

- `LOOP_COMPLETE` が出力される（成功）
- 最大イテレーション数に達する（既定: 100）
- 最大実行時間を超える（既定: 4 時間）
- TUI を終了する

終了したら、プロジェクトディレクトリ内の生成されたファイルと `.agent/` の実行ログを
確認してください。

## コマンドラインオプション

```bash
# イテレーションを制限する
ralph run --max-iterations 50

# 別の設定ファイルを使う
ralph run -c custom-ralph.yml

# 中断したセッションを再開する
ralph run --continue

# CI 向けの静音モード
ralph run -q
```

## タスクの例

### シンプルな関数

```markdown
Write a TypeScript function that validates email addresses.
Include unit tests.
```

### Web スクレイパー

```markdown
Create a web scraper that:
1. Fetches the Hacker News homepage
2. Extracts the top 10 stories
3. Saves them to JSON

Use Node.js with a simple HTML parser.
```

### CLI ツール

```markdown
Build a markdown to HTML converter:
- Accept input/output file arguments
- Support basic markdown syntax
- Add --watch mode
```

## 次のステップ

- 詳しい手順は [最初のタスク](first-task.ja.md) を読む
- ハットやイベントなどの [概念](../concepts/index.ja.md) を理解する
- よくあるワークフロー向けの [プリセット](../guide/presets.ja.md) を探す
- [設定](../guide/configuration.ja.md) オプションについて学ぶ
