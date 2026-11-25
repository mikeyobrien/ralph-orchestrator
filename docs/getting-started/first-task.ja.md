# 最初のタスク

Ralph で完全なタスクを作成し実行する流れを見ていきましょう。

## モードを選ぶ

Ralph は 2 つのモードを提供します。タスクの複雑さに応じて選んでください。

| モード | 使う場面 |
|------|-------------|
| **従来型** | 単純なタスク、手早い自動化、まず試す |
| **ハットベース** | 複雑なワークフロー、多段階のプロセス、役割の分離 |

このガイドでは、まず従来型モードを使い、次にハットベースモードを示します。

## 従来型モードの例

### 1. 初期化する

```bash
mkdir my-first-ralph-task
cd my-first-ralph-task
git init  # Ralph は git があると最もうまく動く

ralph init --backend claude
```

### 2. プロンプトを作成する

`PROMPT.md` を作成します。

```markdown
# Task: Build a Simple Calculator (Rust)

Create a Rust calculator module with:

## Requirements
- Functions: add, subtract, multiply, divide
- Handle division by zero gracefully
- Include unit tests

## Acceptance Criteria
- All functions work correctly
- Tests pass with `cargo test`
- Code is formatted with `cargo fmt`
```

### 3. Ralph を実行する

```bash
ralph run
```

Ralph は次を行います。

1. プロンプトを読む
2. AI エージェントを起動する
3. `LOOP_COMPLETE` が出力されるまで反復する
4. TUI に進捗を表示する

### 4. 結果を確認する

Ralph が完了したら、ディレクトリを確認します。

```bash
ls -la
# src/lib.rs
# tests/calculator.rs
# など

# テストを実行する
cargo test
```

## ハットベースモードの例

より複雑なタスクでは、ハットを使って関心事を分離します。

### 1. コア設定を初期化する

```bash
ralph init --backend claude
```

次に、専門的なハットコレクション（推奨: code-assist）で実行します。

```bash
ralph run -c ralph.yml -H builtin:code-assist
```

これは専門的なハットを使います。

- **Tester** - まず失敗するテストを書く
- **Implementer** - テストを通す
- **Refactorer** - コードを整える

### 2. プロンプトを作成する

```markdown
# Task: Build a URL Shortener

Create a URL shortening service with:

## Requirements
- Generate short codes for URLs
- Retrieve original URLs from short codes
- Handle invalid inputs gracefully
- Persist mappings to SQLite

## Constraints
- Short codes: 6 alphanumeric characters
- No duplicate short codes
```

### 3. ハットの連携で実行する

```bash
ralph run
```

TUI はどのハットがアクティブかを表示します。

```
[iter 3] 00:02:15 Tester
```

### 4. イベント履歴を見る

```bash
ralph events
```

ハット間のイベントフローを表示します。

```
task.start -> Tester
test.written -> Implementer
test.passed -> Refactorer
refactor.done -> Tester
...
```

## 良いプロンプトのコツ

### 具体的にする

```markdown
# 悪い例
Make a web app.

# 良い例
Create an Axum web app with:
- GET /health endpoint returning {"status": "ok"}
- POST /users accepting JSON {name, email}
- SQLite database for persistence
```

### 受け入れ基準を含める

```markdown
## Acceptance Criteria
- [ ] All endpoints respond correctly
- [ ] Invalid JSON returns 400 error
- [ ] Database persists across restarts
```

### 制約を明示する

```markdown
## Constraints
- Use Axum (not Actix)
- Rust 1.75+
- No external API calls
```

## 監視と制御

### 進捗を見る

TUI はリアルタイムの進捗を表示します。主な情報:

- **イテレーション数** - Ralph が実行したサイクル数
- **経過時間** - 総実行時間
- **アクティブなハット** - どのペルソナが作業中か（ハットベースモード）
- **エージェント出力** - AI が何をしているか

### 早めに停止する

TUI で `q` を押すとグレースフルに終了します。

### 中断したセッションを再開する

```bash
ralph run --continue
```

### メトリクスを確認する

完了後、`.agent/` で次を確認します。

- `scratchpad.md` - イテレーションの状態（ハットごとのスクラッチパッドが存在することもある）
- `memories.md` - 永続的な学習
- `tasks.jsonl` - タスク追跡

## よくある問題

### タスクが完了しない

Ralph が永遠に実行される場合:

1. プロンプトに明確な完了基準があるか確認する
2. `LOOP_COMPLETE` が合理的に出力され得るか確認する
3. テスト用に `--max-iterations` を低く設定する

### 誤ったバックエンド

```bash
# バックエンドを明示的に指定する
ralph run --backend kiro
```

### エージェントのエラー

エージェントがインストールされ認証されているか確認します。

```bash
# Claude を直接テストする
claude -p "Hello"

# Kiro をテストする
kiro -p "Hello"
```

## 次のステップ

- [ハットとイベント](../concepts/hats-and-events.ja.md) について学ぶ
- 自分のワークフロー向けの [プリセット](../guide/presets.ja.md) を探す
- [プロンプトの書き方](../guide/prompts.ja.md) を習得する
```
