# よくある Ralph 内省レシピ

ユーザーの質問が合致するときに、これらのパターンを使います。各レシピは次の構成です。
1. トリガー — ユーザーが言ったこと
2. 取得 — どのドキュメントページを引くか
3. 確認 — その中で grep（または同等の操作）で何を探すか
4. 回答の形 — 返答をどう構成するか

## 「なぜループが終了したのか？」

1. **トリガー**: ユーザーが `Loop terminated:` バナーまたは終了コード 2 を共有する。
2. **取得**:
   - `reference/troubleshooting/index.md`
   - `concepts/hats-and-events/index.md`（完了モデルについて）
3. **確認**: 具体的な理由文字列（`max_iterations`、`max_runtime_seconds`、
   `completion_promise`、`required_events`、`error`）を探す。それぞれが文書化された
   終了経路に対応する。
4. **回答の形**: 終了理由を挙げ、その意味についてドキュメントを引用し、それを制御する
   正確な設定つまみ（ralph.yml またはプリセット内）をユーザーに伝える。作業がまだ残って
   いるのに `max_iterations` の場合は、上限を上げるか、`required_events` が実際に発行
   されたかを確認するよう提案する。

## 「なぜハットが発火しないのか？」

1. **トリガー**: ユーザー作成のハットが、期待したイベントの後に起動しない。
2. **取得**:
   - `concepts/hats-and-events/index.md`
   - `reference/troubleshooting/index.md`（曖昧なルーティングのセクション）
3. **確認**: ハットの `triggers:` が発行されたイベントと**完全に**一致するか（ワイルド
   カードなし）、そのトリガーを主張するハットが 1 つだけか（ralph は事前確認で曖昧な
   ルーティングを拒否する）を確認する。
4. **回答の形**: `ralph hats validate` と `ralph hats graph` を実行し、期待される
   イベント連鎖を示し、プリセット内の該当トリガーを指し示す。

## 「X にはどのプリセットを使うべきか？」

1. **トリガー**: 「code-assist と feature のどちらを使うべき？」
2. **取得**:
   - `guide/presets/index.md`（プリセット選択マトリクス）
3. **確認**: パターン列 + 開始イベント + 完了イベント。
4. **回答の形**: 1 行の推奨 + 理由（合致するパターンを引用）。ローカルで発見可能なもの
   （`~/.config/autoloop/presets/` の TOML プリセットを含む）をすべて見られるよう
   `ralph hats list-presets` を提案する。

## 「Ralph はどのようにバックエンドを決めるのか？」

1. **トリガー**: 「なぜ claude を選んだのか？」「kiro-acp をどう強制するのか？」
2. **取得**:
   - `guide/backends/index.md`
   - `reference/faq/index.md`（自動検出の優先順位）
3. **確認**: 解決順序は CLI フラグ（`-b`）＞設定内の `cli.backend:` ＞既定の優先度
   リストをたどる自動検出。
4. **回答の形**: 優先順位を提示し、そのケースに対する正確な上書き（フラグまたは設定）を
   ユーザーに示す。

## 「.ralph/ ディレクトリは何のためにあるのか？」

1. **取得**:
   - `advanced/memory-system/index.md`
   - `advanced/task-system/index.md`
   - `concepts/memories-and-tasks/index.md`
2. **確認**: scratchpad、memories.md、tasks.jsonl、loop.lock、loops.json、
   merge-queue.jsonl の用途。
3. **回答の形**: 1 ファイルにつき 1 文 + どの ralph コマンドがそれに触れるか。

## 「新しいバックエンドはどう追加するのか？」

1. **トリガー**: 「Ralph に X モデルの CLI を対応させたい」。
2. **取得**:
   - `guide/backends/index.md`（既存のパターン）
   - `api/ralph-adapters/index.md`（アダプタトレイト / エグゼキュータの型）
3. **確認**: バックエンドの enum、`CliBackend::<name>()` ファクトリ、エグゼキュータの型
   （PTY/Stdio/Acp）、優先度リストへの挿入位置。
4. **回答の形**: 編集すべきクレートを列挙する（バックエンドとエグゼキュータは
   `ralph-adapters`、環境変数の診断は `ralph-cli/src/doctor.rs`、ドキュメントは
   `guide/backends.md`、テスト）。PR ワークフローについては `references/contributing.md`
   をユーザーに案内する。

## 「Ralph が遅い / 動かないのはなぜか？」

1. **取得**:
   - `reference/troubleshooting/index.md`（アイドルタイムアウトのセクション）
   - `advanced/diagnostics/index.md`
2. **確認**: `idle_timeout_secs`、バックエンドのコールドスタート費用、TUI サブプロセス
   モード、診断ログのパス（`.ralph/diagnostics/logs/`）。
3. **回答の形**: 診断ログのファイル名規則を案内する。遅いバックエンドには
   `idle_timeout_secs` を上げるよう提案する（kiro-acp はコールドスタートに約 20 秒）。

## 「実行の合間に Ralph の状態をリセットするには？」

1. **取得**:
   - `guide/cli-reference/index.md`（`ralph clean`）
2. **回答の形**: `ralph clean` は `.ralph/agent/` をクリアする。ループレジストリは
   `.ralph/loops.json` を手動削除、マージキューは `.ralph/merge-queue.jsonl` を削除。

## 「Ralph は X 機能をサポートしているか？」

一般的なパターン:

1. llms.txt を `curl` する。
2. セクションタイトルとリンクの説明の中で、機能のキーワードを `grep` する。
3. あれば → そのページを取得し、確認し、ソース付きで「はい」と答える。
4. なければ → 最近の追加について `reference/changelog/index.md` を検索する。
5. それでもなければ → リポジトリの `specs/` を検索する（ドキュメントサイトにはない）。
6. 最後に → <https://github.com/mikeyobrien/ralph-orchestrator/tree/main/crates>
   のソースツリー。

「あるべきだから」という理由で機能の存在を仮定しないでください。Ralph は意図的に
最小限です。不確かなときは「文書化されていない。確認できる場所はここ」と伝えてください。

## 「最新バージョンで何が変わったか？」

1. **取得**:
   - `reference/changelog/index.md`
2. **回答**: ユーザーのローカルの `ralph --version` より新しい項目を要約する。追跡性の
   ため、リンクがあれば PR 番号（例: #316）を含める。
