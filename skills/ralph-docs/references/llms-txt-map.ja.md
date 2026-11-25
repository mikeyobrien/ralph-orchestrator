# Ralph llms.txt セクションマップ

権威あるドキュメント索引は
<https://mikeyobrien.github.io/ralph-orchestrator/llms.txt> にあります。

このファイルは**ルーティングの近道**です。ユーザーの質問やバグ報告に対して、どの 1〜3
ページを取得すべきかを教えてくれます。他のどのページよりも先に、常に llms.txt 自体を
再取得してください。URL はリネームされることがあります。

## トップレベルのセクション

1. **Getting Started** — セットアップ、インストール、最初のタスク。オンボーディングに使う。
2. **Concepts** — メンタルモデル。「なぜ Ralph はこう動くのか？」に使う。
3. **User Guide** — 実践的な CLI の使い方。「どうやって〜する？」に使う。
4. **Advanced** — アーキテクチャ、サブシステム。深い質問に使う。
5. **API Reference** — クレートレベルの rustdoc。コード変更に使う。
6. **Examples** — 動作する設定パターン。テンプレートとして使う。
7. **Contributing** — 開発環境、PR の慣習。変更を出す前に使う。
8. **Reference** — 変更履歴、FAQ、用語集、トラブルシューティング。バージョン固有の主張と
   エラーのトリアージに使う。

## 質問 → ページ の対応表

当てずっぽうに取得する前に、これを使います。正規の URL パターンは
`https://mikeyobrien.github.io/ralph-orchestrator/<path>/index.md` です。

### イベントループの挙動

- 「なぜループが終了したのか？」→ `reference/troubleshooting/index.md`
- 「開始イベントとは？」→ `concepts/hats-and-events/index.md`
- 「各イテレーションのフレッシュコンテキストはどう動くのか？」→
  `concepts/tenets/index.md`, `concepts/ralph-wiggum-technique/index.md`
- 「完了イベントはどう動くのか？」→ `advanced/event-system/index.md`

### ハット（ユーザー作成のワークフロー）

- 「ハットはどう書くのか？」→ `advanced/custom-hats/index.md`
- 「トリガーと publish の違いは？」→ `concepts/hats-and-events/index.md`
- 「どの組み込みプリセットを使うべきか？」→ `guide/presets/index.md`
- 「なぜハットが発火しないのか？」→ `reference/troubleshooting/index.md` +
  `concepts/hats-and-events/index.md`

### メモリ + タスク

- 「メモリはどう永続化されるのか？」→ `advanced/memory-system/index.md`
- 「タスクはどこに保存されるのか？」→ `advanced/task-system/index.md`,
  `concepts/memories-and-tasks/index.md`
- 「どうリセットするのか？」— CLI を確認: `guide/cli-reference/index.md`（`ralph clean`）

### バックエンド

- 「どのバックエンドがサポートされているのか？」→ `guide/backends/index.md`
- 「バックエンド選択はどう動くのか？」→ `guide/backends/index.md` +
  `reference/faq/index.md`（自動検出の順序）
- 「なぜバックエンドがタイムアウトするのか？」→ `reference/troubleshooting/index.md`
- kiro-acp, claude, gemini, codex, pi, roo, copilot, opencode, amp —
  すべて `guide/backends/index.md` でカバーされている

### プリセット

- 「プリセットを一覧する」→ `guide/presets/index.md`（+ ユーザーのシステム上で発見
  可能なものは `ralph hats list-presets`）
- 「YAML プリセットの作成」→ `guide/presets/index.md`
- 「TOML プリセットの作成」→ 外部リンク
  <https://mikeyobrien.github.io/autoloop/guides/creating-presets>
- プリセットのリゾルバパス → `guide/presets/index.md`（PR #316 以降）

### CLI + TUI

- フラグの意味 → `guide/cli-reference/index.md`
- 自律 vs 対話 → `guide/cli-reference/index.md`
- RPC モード / サブプロセス TUI → `api/ralph-cli/index.md`
- Web ダッシュボード → `advanced/diagnostics/index.md`

### 並列ループ + 波

- ワークツリー、マージキュー → `advanced/parallel-loops/index.md`
- 波のディスパッチ → `advanced/agent-waves/index.md`

### コードへの貢献

- クレート構成 → リポジトリルートの AGENTS.md（ドキュメントサイトにはない）
- 開発環境 → `contributing/setup/index.md`
- スタイル → `contributing/style/index.md`
- テスト → `contributing/testing/index.md`
- PR → `contributing/pull-requests/index.md`

## 鮮度

llms.txt はドキュメントのデプロイ時に再生成されます。キャッシュは最大 7 日にします。
そこからリンクされているどのドキュメントページも、そのトピックの正規の情報源です。遅れて
いることがあるリポジトリ内の README の断片より、そちらを優先してください。

## llms.txt がカバーしていないとき

- API ドキュメントに現れない**クレート内部の詳細**: ソースを読む
  <https://github.com/mikeyobrien/ralph-orchestrator/tree/main/crates>。
- **最近の変更**: `reference/changelog/index.md` と GitHub のリリース/コミットログを
  参照する。
- **実験的な機能**: ドキュメントサイトではなく、リポジトリの `specs/` にあることがある。
