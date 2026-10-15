# メモリとタスク

Ralph は、永続的な状態のために 2 つの補完的なシステムを使います。セッションをまたいだ
学習のための「メモリ」と、ランタイムの作業追跡のための「タスク」です。

## 概要

| システム | 保存場所 | 用途 |
|--------|---------|---------|
| **メモリ** | `.ralph/agent/memories.md` | セッションをまたいで蓄積された知恵 |
| **タスク** | `.ralph/agent/tasks.jsonl` | ランタイムの作業項目 |

どちらも既定で有効であり、協働してレガシーのスクラッチパッドを置き換えます。

## メモリ

メモリは、セッションをまたいで学習を永続化します。Ralph が覚えておくべきパターン、決定、
修正、コンテキストを捉えます。

### メモリの種類

| 種類 | 用途 |
|------|---------|
| `pattern` | 発見されたコードベースの慣習 |
| `decision` | アーキテクチャの選択とその根拠 |
| `fix` | 繰り返し起きる問題への解決策 |
| `context` | プロジェクト固有の知識 |

### メモリの作成

```bash
# pattern: 発見された慣習
ralph tools memory add "All API handlers return Result<Json<T>, AppError>" \
  -t pattern --tags api,error-handling

# decision: アーキテクチャの選択
ralph tools memory add "Chose JSONL over SQLite: simpler, git-friendly" \
  -t decision --tags storage,architecture

# fix: 繰り返し起きる問題の解決策
ralph tools memory add "cargo test hangs: kill orphan postgres" \
  -t fix --tags testing,postgres

# context: プロジェクトの知識
ralph tools memory add "The /legacy folder is deprecated, use /v2" \
  -t context --tags api,migration
```

### メモリの検索

```bash
# 広い検索
ralph tools memory search "api"

# 種類で絞り込む
ralph tools memory search -t fix "error"

# タグで絞り込む
ralph tools memory search --tags api,auth

# すべてのメモリを一覧する
ralph tools memory list

# 最近の修正を一覧する
ralph tools memory list -t fix --last 10
```

### メモリの注入

メモリは各イテレーションの開始時に自動的に注入されます。

```yaml
memories:
  enabled: true
  inject: auto      # auto, manual, または none
  budget: 2000      # 注入する最大トークン数
  filter:
    types: []       # 種類で絞り込む（空 = すべて）
    tags: []        # タグで絞り込む（空 = すべて）
    recent: 0       # 日数制限（0 = 制限なし）
```

### メモリのベストプラクティス

1. **具体的にする** — 「良いパターンがある」ではなく「バレルエクスポートを使う」
2. **なぜかを含める** — 単に「X を使う」ではなく「Y だから X を選んだ」
3. **メモリごとに 1 つの概念** — 複雑な学びは分割する
4. **一貫してタグ付けする** — 既存のタグを再利用する

## タスク

タスクは、オーケストレーション中のランタイムの作業項目を追跡します。

### タスクの作成

```bash
# 基本的なタスク
ralph tools task add "Implement user authentication"

# 優先度付き（1〜5、1 = 最高）
ralph tools task add "Fix critical bug" -p 1

# 依存関係付き
ralph tools task add "Deploy to production" --blocked-by setup-infra
```

### タスクの管理

```bash
# すべてのタスクを一覧する
ralph tools task list

# ブロックされていないタスクのみを一覧する
ralph tools task ready

# 完了したタスクをクローズする
ralph tools task close task-123
```

### タスクのワークフロー

1. Ralph がプロンプト/計画からタスクを作成する
2. タスクは優先度順に処理される
3. 依存関係が尊重される（ブロックされたタスクは待つ）
4. 完了したタスクはクローズされる
5. タスクが残っていなければループが終わる

### タスクのクローズ規則

タスクは、次の場合にのみクローズしなければなりません。

1. 実装が実際に完了している
2. テストが通る
3. ビルドが成功する（該当する場合）
4. 完了の証拠が存在する

```bash
# 良い: 証拠付きでクローズ
cargo test  # 通る
ralph tools task close task-123

# 悪い: 検証なしでクローズ
ralph tools task close task-123  # テストを実行していない！
```

## メモリ vs タスク

| 観点 | メモリ | タスク |
|--------|----------|-------|
| **永続性** | セッションをまたぐ | 単一セッション |
| **目的** | 学習 | 作業追跡 |
| **作成される時** | 何かを学んだとき | 作業が特定されたとき |
| **削除される時** | まれ | 完了したとき |

## レガシースクラッチパッドモード

メモリとタスクを無効にする（レガシーモード）には:

```yaml
memories:
  enabled: false
tasks:
  enabled: false
```

このモードでは、すべての状態に `.agent/scratchpad.md` が使われます。

ハットベースの設定では、スクラッチパッドはハットごとに設定可能です。各ハットは、カスタムの
スクラッチパッドパスを設定したり、スクラッチパッドを完全に無効にしたり、グローバルな
`core.scratchpad` 設定を継承したりできます。詳細は
[ハットごとのスクラッチパッド](../guide/configuration.ja.md#with-per-hat-scratchpads)
を参照してください。

## ファイル形式

### memories.md

```markdown
# Memories

## Patterns

### mem-1737372000-a1b2
> All API handlers return Result<Json<T>, AppError>
<!-- tags: api, error-handling | created: 2024-01-20 -->

## Decisions

### mem-1737372100-c3d4
> Chose JSONL over SQLite for simplicity
<!-- tags: storage | created: 2024-01-20 -->
```

### tasks.jsonl

```json
{"id":"task-001","title":"Implement auth","priority":2,"status":"open","created":"2024-01-20T10:00:00Z"}
{"id":"task-002","title":"Add tests","priority":3,"status":"open","blocked_by":["task-001"],"created":"2024-01-20T10:01:00Z"}
```

## ハットとの統合

ハットはメモリとタスクを使えます。

```yaml
hats:
  builder:
    triggers: ["task.start"]
    instructions: |
      1. Check memories for relevant patterns
      2. Pick a task from `ralph tools task ready`
      3. Implement the task
      4. Record learnings as memories
      5. Close the task with `ralph tools task close <id>`
```

## 次のステップ

- 品質ゲートについて [バックプレッシャー](backpressure.ja.md) を学ぶ
- 完全なオプションは [設定](../guide/configuration.ja.md) を見る
- [メモリシステム](../advanced/memory-system.ja.md) を深く探る
