# 並列ループ

Ralph は、ファイルシステムの分離のために git ワークツリーを使い、複数のオーケストレーション
ループを並列に実行できます。これにより、競合なく複数のタスクを同時に進められます。

## 仕組み

Ralph ループを開始すると:

1. **最初のループ**が `.ralph/loop.lock` を取得し、その場で実行する（主ループ）
2. **追加のループ**は自動的に `.worktrees/<loop-id>/` に spawn される
3. **各ループ**は分離されたイベント、タスク、スクラッチパッドを持つ
4. **メモリは共有される** — メインリポジトリの `.agent/memories.md` にシンボリックリンク
   される
5. **完了時**、ワークツリーループは既定では手動処理のために保存されるか、auto-merge が
   有効なときは merge-ralph のためにキューに入れられる

```
┌─────────────────────────────────────────────────────────────────────┐
│  Terminal 1                    │  Terminal 2                       │
│  ralph run -p "Add auth"       │  ralph run -p "Add logging"       │
│  [acquires lock, runs in-place]│  [spawns to worktree]             │
│           ↓                    │           ↓                       │
│     Primary loop               │  .worktrees/ralph-20250124-a3f2/  │
│           ↓                    │           ↓                       │
│     LOOP_COMPLETE              │     LOOP_COMPLETE → review/merge  │
└─────────────────────────────────────────────────────────────────────┘
```

## 使い方

```bash
# 最初のループがロックを取得し、その場で実行する
ralph run -p "Add authentication"

# 別のターミナルで — 自動的にワークツリーに spawn する
ralph run -p "Add logging"

# 実行中のループを確認する
ralph loops

# 特定のループのログを表示する
ralph loops logs <loop-id>
ralph loops logs <loop-id> --follow  # リアルタイムのストリーミング

# 順次実行を強制する（ロックを待つ）
ralph run --exclusive -p "Task that needs main workspace"

# auto-merge をスキップする（手動処理のためにワークツリーを残す）
ralph run --no-auto-merge -p "Experimental feature"
```

## ループの状態

| 状態 | 説明 |
|-------|-------------|
| `running` | ループが活発に実行中 |
| `queued` | 完了、マージ待ち |
| `merging` | マージ操作が進行中 |
| `merged` | main に正常にマージ済み |
| `needs-review` | マージ失敗、手動での解決が必要 |
| `crashed` | プロセスが予期せず停止した |
| `orphan` | ワークツリーは存在するが追跡されていない |
| `discarded` | ユーザーが明示的に破棄した |

## ファイル構成

```
project/
├── .ralph/
│   ├── loop.lock          # Primary loop indicator
│   ├── loops.json         # Loop registry
│   ├── merge-queue.jsonl  # Merge event log
│   └── events.jsonl       # Primary loop events
├── .agent/
│   └── memories.md        # Shared across all loops
└── .worktrees/
    └── ralph-20250124-a3f2/
        ├── .ralph/events.jsonl    # Loop-isolated
        ├── .agent/
        │   ├── memories.md → ../../.agent/memories.md  # Symlink
        │   └── scratchpad.md      # Loop-isolated
        └── [project files]
```

## ループの管理

```bash
# すべてのループを状態付きで一覧する
ralph loops list

# ループの出力を表示する
ralph loops logs <id>              # 完全な出力
ralph loops logs <id> --follow     # リアルタイムでストリーム

# イベント履歴を表示する
ralph loops history <id>           # 整形された表
ralph loops history <id> --json    # 生の JSONL

# merge-base からの変更を表示する
ralph loops diff <id>              # 完全な差分
ralph loops diff <id> --stat       # 要約のみ

# リモートレビュー用にブランチをプッシュし .ralph/reviews/<id>.md を書く
ralph loops publish-review <id> --remote origin --base origin/main

# マージせずに 1 つのループブランチを更新されたベースにリベースする
ralph loops rebase <id> --base origin/main

# キュー待ち/needs-review かつ実行中でない ralph/* ワークツリーブランチをすべてリベースする
ralph loops rebase --base origin/main

# ワークツリーでシェルを開く
ralph loops attach <id>

# 失敗したループのマージを再実行する
ralph loops retry <id>

# 実行中のループを停止する
ralph loops stop <id>              # SIGTERM
ralph loops stop <id> --force      # SIGKILL

# 一時停止したループを再開する
ralph loops resume <id>

# ループを破棄してクリーンアップする
ralph loops discard <id>           # 確認あり
ralph loops discard <id> -y        # 確認をスキップ

# 古いループ（クラッシュしたプロセス）をクリーンアップする
ralph loops prune
```

## 自動マージのワークフロー

ワークツリーループが完了すると、自身をマージのためにキューに入れます。主ループは、終了時に
このキューを処理します。

```
┌──────────────────────────────────────────────────────────────────────┐
│  Worktree Loop                         Primary Loop                  │
│  ─────────────                         ─────────────                 │
│  LOOP_COMPLETE                                                       │
│       ↓                                                              │
│  Queue for merge ─────────────────────→ [continues working]         │
│       ↓                                       ↓                      │
│  Exit cleanly                          LOOP_COMPLETE                 │
│                                              ↓                       │
│                                        Process merge queue           │
│                                              ↓                       │
│                                        Spawn merge-ralph             │
└──────────────────────────────────────────────────────────────────────┘
```

merge-ralph プロセスは、専門的な役割を持つ**ハットコレクション**を使います。

| ハット | トリガー | 用途 |
|-----|---------|---------|
| `merger` | `merge.start` | `git merge` を実行し、テストを走らせる |
| `resolver` | `conflict.detected` | 意図を理解してマージ競合を解決する |
| `tester` | `conflict.resolved` | 競合解決後にテストが通ることを検証する |
| `cleaner` | `merge.done` | ワークツリーとブランチを削除する |
| `failure_handler` | `*failed`, `unresolvable` | ループを手動レビュー用にマークする |

このワークフローは競合を賢く扱います。
1. **競合なし**: マージ → テスト実行 → クリーンアップ → 完了
2. **競合あり**: 検出 → AI が解決 → テスト実行 → クリーンアップ → 完了
3. **解決不能**: 中止 → レビュー用にマーク → 手動修正のためワークツリーを残す

## リモートレビューのワークフロー

完了したワークツリーブランチを、ベースブランチにマージせずに人間や外部の自動化でレビュー
させたい場合は、auto-merge を無効のままにし、ループブランチを公開します。

```bash
ralph loops publish-review <loop-id> --remote origin --base origin/main
```

これは `ralph/<loop-id>` をリモートにプッシュし、リモートブランチ、ベース ref、コミット、
変更されたファイル、任意のローカルの引き継ぎ/サマリ成果物のパスを伴う
`.ralph/reviews/<loop-id>.md` を書きます。

中央のブランチが進んだら、保留中のレビューブランチをマージせずにリベースします。

```bash
# 1 つのループ
ralph loops rebase <loop-id> --base origin/main

# レビュー可能なすべてのループブランチ
ralph loops rebase --base origin/main

# 履歴を書き換えた後、リモートのレビューブランチも更新する
ralph loops rebase --base origin/main --push
```

`ralph loops rebase` は実行中のレジストリエントリをスキップします。競合が起きた場合、Git は
影響を受けたワークツリーをリベース状態のまま残し、`git rebase --continue` または
`git rebase --abort` での手動解決を待ちます。

## 競合の解決

マージ競合が起きたとき、AI リゾルバは:

1. 競合マーカー（`<<<<<<<`、`=======`、`>>>>>>>`）を調べる
2. 両方の側の**意図**を理解する（コードだけでなく）
3. 可能なときは両方の意図を保存して解決する
4. 直接矛盾する場合はループの変更（新しい作業）を優先する

**`needs-review` とマークされる競合:**
- 両方の側での大きなアーキテクチャの変更
- 自動的に調和できない複雑なリファクタリング
- 人間の判断を要するビジネスロジックの矛盾

手動で解決するには:
```bash
# ワークツリーに入る
ralph loops attach <loop-id>

# 問題を修正し、コミットする
git add . && git commit -m "Manual conflict resolution"

# マージを再試行する
ralph loops retry <loop-id>

# または不要なら破棄する
ralph loops discard <loop-id>
```

## ベストプラクティス

**並列ループを使う場面:**
- ファイルの重複が最小限の独立した機能
- 機能作業が続く間のバグ修正
- コード変更と並行するドキュメント更新
- アクティブな開発と競合しないテストの追加

**`--exclusive`（順次）を使う場面:**
- 多くのファイルに触れる大規模なリファクタリング
- データベースの移行やスキーマの変更
- 共有の設定ファイルを変更するタスク
- 別の進行中のループの変更に依存する作業

**競合を減らすためのヒント:**
- ループをコードベースの別個の領域に集中させる
- 新機能を追加するときは別々のファイルを使う
- 並列ループで同じ関数を変更するのを避ける
- 競合する作業を始める前に、1 つのループを完了させる

## トラブルシューティング

### ループがオペレーター入力を待って一時停止している

```bash
# ループの状態とログを確認する
ralph loops
ralph loops logs <loop-id>

# 一時停止の境界から再開する
ralph loops resume <loop-id>
```

`ralph loops resume` は繰り返し実行しても安全です（冪等）。

### ループが `queued` 状態で止まっている

```bash
# 主ループがまだ実行中か確認する
ralph loops

# 主ループは終了したがマージが始まらない場合は、手動で起動する
ralph loops retry <loop-id>
```

### マージが失敗し続ける

```bash
# merge-ralph のログを表示する
ralph loops logs <loop-id>

# どの変更が競合するか確認する
ralph loops diff <loop-id>

# ワークツリーで手動解決する
ralph loops attach <loop-id>
```

### 孤立したワークツリー

```bash
# 孤立を一覧してクリーンアップする
ralph loops prune

# 特定のワークツリーを強制的にクリーンアップする
git worktree remove .worktrees/<loop-id> --force
git branch -D ralph/<loop-id>
```

### ロックファイルの問題

```bash
# 誰がロックを保持しているか確認する
cat .ralph/loop.lock

# プロセスが停止していれば、古いロックを削除する
rm .ralph/loop.lock
```

## 環境変数

| 変数 | 説明 |
|----------|-------------|
| `RALPH_MERGE_LOOP_ID` | どのループをマージするか識別するため auto-merge が設定する |
| `RALPH_DIAGNOSTICS=1` | 詳細な診断ロギングを有効にする |
| `RALPH_VERBOSE=1` | 詳細な出力モード |
