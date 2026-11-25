# ハットコレクション

組み込みのハットコレクションは、今では意図的に小さく保たれています。Ralph はコアとなる
実用的な既定セットを同梱し、より広範なワークフローのアイデアは、すべてのパターンを
サポート対象の組み込みとして扱うのではなく、例として文書化します。

## クイックスタート

```bash
ralph init --backend claude
ralph init --list-presets

ralph run -c ralph.yml -H builtin:code-assist -p "Add user authentication"
```

## サポートされる組み込み

| コレクション | ハット | 最適な用途 | メモ |
|---|---|---|---|
| `code-assist` | `planner`, `builder`, `critic`, `finalizer` | 既定の実装作業 | 推奨の既定。フレッシュアイのレビューと最終の完了ゲートを追加 |
| `debug` | `investigator`, `tester`, `fixer`, `verifier` | 根本原因のデバッグ | 再現と修正の検証に強い |
| `research` | `researcher`, `synthesizer` | 読み取り専用の分析 | コード変更なし |
| `review` | `reviewer`, `analyzer` | 敵対的なコードレビュー | コード変更なし |
| `pdd-to-code-assist` | 多段階の設計 + ビルドのパイプライン | アイデアからコードへ | 高度で楽しいが、より遅く予測しにくい |

## 内部プリセット

Ralph は、通常の一覧には表示しないものの、いくつかの内部/テスト用プリセットも利用可能に
しています。

- `merge-loop`
- `hatless-baseline`

## 推奨ワークフロー

- ほとんどの実装タスクには `code-assist` を使う。
- 専門的なモードが必要なときは `debug`、`research`、`review` を使う。
- エンドツーエンドの探索的ワークフローが特に欲しく、追加のイテレーションのコストを払っても
  よいときは `pdd-to-code-assist` を使う。

| コレクション | 正規のソース | ハット | 開始イベント | 完了 | 最適な用途 |
|---|---|---|---|---|---|
| `bugfix` | `presets/bugfix.yml` | `reproducer`, `fixer`, `verifier`, `committer` | `repro.start` | `LOOP_COMPLETE`（既定） | 再現/修正/検証/コミットのバグワークフロー |
| `code-assist` | `presets/code-assist.yml` | `planner`, `builder`, `validator`, `committer` | `build.start` | `LOOP_COMPLETE` | スペック/タスク/説明からの TDD 実装 |
| `debug` | `presets/debug.yml` | `investigator`, `tester`, `fixer`, `verifier` | `debug.start` | `DEBUG_COMPLETE` | 根本原因のデバッグと仮説検証 |
| `deploy` | `presets/deploy.yml` | `builder`, `deployer`, `verifier` | `task.start`（既定） | `LOOP_COMPLETE` | デプロイとリリースのワークフロー |
| `docs` | `presets/docs.yml` | `writer`, `reviewer` | `task.start`（既定） | `DOCS_COMPLETE` | ドキュメントの執筆とレビュー |
| `feature` | `presets/feature.yml` | `builder`, `reviewer` | `task.start`（既定） | `LOOP_COMPLETE` | レビューを統合した機能開発 |
| `fresh-eyes` | `presets/fresh-eyes.yml` | `builder`, `fresh_eyes_auditor`, `fresh_eyes_gatekeeper` | `fresh_eyes.start` | `LOOP_COMPLETE` | 強制された、繰り返しの懐疑的な自己レビューパス |
| `gap-analysis` | `presets/gap-analysis.yml` | `analyzer`, `verifier`, `reporter` | `gap.start` | `GAP_ANALYSIS_COMPLETE` | スペック対実装の監査 |
| `hatless-baseline` | `presets/hatless-baseline.yml` | _(なし)_ | `task.start` | `LOOP_COMPLETE` | 比較用のハットなしのベースライン挙動 |
| `merge-loop` | `crates/ralph-cli/presets/merge-loop.yml` | `merger`, `resolver`, `tester`, `cleaner`, `failure_handler` | `merge.start` | `MERGE_COMPLETE` | 内部のマージ/ワークツリー自動化 |
| `pdd-to-code-assist` | `presets/pdd-to-code-assist.yml` | `inquisitor`, `architect`, `design_critic`, `explorer`, `planner`, `task_writer`, `builder`, `validator`, `committer` | `design.start` | `LOOP_COMPLETE` | アイデア → 計画 → 実装の完全なパイプライン |
| `pr-review` | `presets/pr-review.yml` | `correctness_reviewer`, `security_reviewer`, `architecture_reviewer`, `synthesizer` | `task.start`（既定） | `LOOP_COMPLETE` | 多視点の PR レビュー |
| `refactor` | `presets/refactor.yml` | `refactorer`, `verifier` | `task.start`（既定） | `REFACTOR_COMPLETE` | 段階的で検証済みのリファクタリング |
| `research` | `presets/research.yml` | `researcher`, `synthesizer` | `research.start` | `RESEARCH_COMPLETE` | コード変更のない探索と分析 |
| `review` | `presets/review.yml` | `reviewer`, `analyzer` | `review.start` | `REVIEW_COMPLETE` | レビューのみのワークフロー |
| `spec-driven` | `presets/spec-driven.yml` | `spec_writer`, `spec_reviewer`, `implementer`, `verifier` | `spec.start` | `LOOP_COMPLETE`（既定） | 仕様駆動の実装 |
| `wave-review` | `presets/wave-review.yml` | `coordinator`, `reviewer`（×3）, `synthesizer` | `review.start` | `LOOP_COMPLETE` | 専門的な並列コードレビュー（波対応） |

## なぜ組み込みセットは小さいのか

すべての組み込みプリセットは、製品としての面積になります。

- 文書化しなければならない。
- テストし、動作を維持しなければならない。
- API と CLI の一覧で首尾一貫して見えなければならない。

Ralph は今では、より実験的でニッチなオーケストレーションパターンについては、小さなサポート
セットに加えてドキュメントの例を優先します。

## 組み込みの代わりに例で

仕様駆動開発、レッドチームレビュー、モブプログラミング、フレッシュアイのループといった
歴史的なワークフローのアイデアは、今では出荷される組み込みではなく例です。次を参照して
ください。

- [例の索引](../examples/index.ja.md)
- [仕様駆動開発の例](../examples/spec-driven.ja.md)
- [マルチハットワークフロー](../examples/multi-hat.ja.md)

## 使用例

```bash
# 既定の実装ワークフロー
ralph run -c ralph.yml -H builtin:code-assist -p "Add OAuth login"

# デバッグ
ralph run -c ralph.yml -H builtin:debug -p "Investigate why login fails on mobile"

# リサーチ
ralph run -c ralph.yml -H builtin:research -p "Map the authentication architecture"

# レビュー
ralph run -c ralph.yml -H builtin:review -p "Review the changes in src/api/"

# 高度/楽しいワークフロー
ralph run -c ralph.yml -H builtin:pdd-to-code-assist -p "Build a rate limiter"
```

## よくあるワークフローのパターン

Ralph の組み込みは、たいてい次のいずれかの形に従います。

### 1) 直線パイプライン
専門ハットの固定された連なり。

例: `feature`, `bugfix`, `deploy`, `docs`

### 2) クリティック / アクターのループ
一方のハットが提案し、もう一方が批評/検証し、反復する。

例: `spec-driven`, `review`, `fresh-eyes`

### 3) 複数レビュアー + 統合
並列の視点を 1 つの結果にまとめる。

例: `pr-review`

### 4) スキャッター・ギャザー（波）
1 つのハットがディスパッチし、並列のワーカーが実行し、アグリゲーターが統合する。

例: `wave-review`

詳細は [エージェント波](../advanced/agent-waves.ja.md) を参照してください。

### 5) 拡張されたエンドツーエンドのオーケストレーション
アイデアから実装までの大規模な多段階パイプライン。

例: `pdd-to-code-assist`

## 分割設定 vs 単一ファイル設定

推奨:
- コア/ランタイム設定は `ralph.yml` に置く
- ワークフローは `-H builtin:<name>` で選ぶ

後方互換の単一ファイルモード（引き続きサポート）:

```bash
# 1 つの結合されたプリセットファイルをメイン設定として使う
ralph run -c presets/feature.yml -p "Add OAuth login"
```

## 独自のハットコレクションを作成する

ハット関連のセクションを持つハットファイルを作成します。

```yaml
event_loop:
  starting_event: "build.start"
  completion_promise: "LOOP_COMPLETE"

hats:
  builder:
    name: "Builder"
    triggers: ["build.start"]
    publishes: ["build.done"]
    instructions: |
      Implement the requested change and verify it.

  reviewer:
    name: "Reviewer"
    triggers: ["build.done"]
    publishes: ["LOOP_COMPLETE"]
    instructions: |
      Review the change, request fixes if needed, and close when done.
```

実行します。

```bash
ralph run -c ralph.yml -H .ralph/hats/my-workflow.yml
```

## TOML 形式のプリセット（複数ファイルのディレクトリ）

Ralph は、もともと [`@mobrienv/autoloop`](https://github.com/mikeyobrien/autoloop) が
定義した複数ファイルの TOML プリセット形式も読み込みます。これは、`autoloops.toml`、
`topology.toml`、`harness.md`、および `roles/` 配下の役割ごとのプロンプトファイルを含む
ディレクトリです。これは YAML と並ぶ第一級のプリセット形式として扱われます。`-H <name>`
または `-H <path>` を同じように使ってください。

```bash
# リゾルバ経由で発見可能（下記の「プリセットの解決」を参照）
ralph run -H autocode -p "Add OAuth login"

# またはディレクトリを直接指定する
ralph run -H /path/to/autocode -p "Add OAuth login"
```

**TOML プリセットの作成** — autoloop の作成ガイドに従ってください。

- **[プリセットの作成](https://mikeyobrien.github.io/autoloop/guides/creating-presets)** —
  ディレクトリ構成、`autoloops.toml` + `topology.toml` のフィールド、役割プロンプトの
  慣習、ハーネスのルール、フェイルクローズのパターン。
- **[同梱のプリセット例](https://github.com/mikeyobrien/autoloop/tree/main/packages/presets/presets)** —
  出発点としてコピーできる 15 以上のリファレンス実装（autocode, autofix, autoreview,
  autodebug, autospec など）。

TOML 形式は、各役割が独自のプロンプトファイル、別のハーネス、または単一の YAML が許す以上の
構造を必要とするときに便利です。どちらの形式も同じ内部表現にコンパイルされるため、ralph の
その他の部分は何も変わりません。バックエンド、CLI、イベントフロー、完了のセマンティクスは
同一に動作します。

## プリセットの解決

`-H <name>` は、素のプリセット名を次のパスに対して順に解決します（最初にヒットしたものが
勝ち）。

1. `./presets/<name>(.yml|.yaml|/)` — プロジェクトローカル
2. `$XDG_CONFIG_HOME/ralph/presets/<name>/`
3. `$HOME/.config/ralph/presets/<name>/` — ユーザー、正規
4. `$HOME/.config/autoloop/presets/<name>/` — autoloop CLI と共有
5. `$RALPH_PRESETS_DIR/<name>/` — 明示的な上書き
6. `$AUTOLOOP_PRESETS_DIR/<name>/` — 後方互換のフォールバック

`ralph hats list-presets` を実行すると、システム上で発見可能なすべて（YAML と TOML の両形式を
1 つの表で）が見られます。

## 信頼できる情報源と同期

- 正規のプリセットファイル: `presets/*.yml`
- 埋め込まれた CLI ミラー: `crates/ralph-cli/presets/*.yml`
- 同期スクリプト: `./scripts/sync-embedded-files.sh`
