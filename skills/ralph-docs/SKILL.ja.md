---
name: ralph-docs
description: Introspect, explain, and improve Ralph Orchestrator using its published llms.txt doc map. Use this skill whenever the user asks questions about Ralph's behavior, wants to understand how a Ralph internal works (event loop, hats, memories, tasks, backends, presets), debug an unfamiliar failure mode, or propose a code change to the ralph-orchestrator repo. The skill teaches the agent to discover authoritative answers from the live docs via llms.txt before guessing, and to scope improvements through the published architecture rather than the local checkout alone.
---

# Ralph ドキュメント

Ralph Orchestrator を、フレームワークが賢いエージェントに期待するのと同じ方法で内省します。
すなわち、<https://mikeyobrien.github.io/ralph-orchestrator/llms.txt> の公開ドキュメント
マップを参照し、質問に関連するセクションだけを取得し、権威ある情報源から回答します。

このスキルは、まず推測するアシスタントではなく、Ralph 内部のコントリビューターのように
振る舞うために使います。

## このスキルを使う場面

- イベントループ、ハット、メモリ、タスク、プリセット、バックエンド、CLI、TUI、診断、波、
  API について「Ralph は X をどうやって行うのか？」という質問に答える。
- 観測された挙動（「なぜループが max_iterations で終了したのか？」「なぜハットが発火
  しなかったのか？」）を、パターンマッチではなくドキュメントの第一原理から説明する。
- `ralph-orchestrator` コードベースへの改善を提案し、範囲を絞る。コードを書く前に、正しい
  クレート、関連する概念ドキュメント、既存のテスト対象を特定する。
- Ralph のバグ報告をトリアージする。症状を該当しそうなサブシステムに対応づけ、その
  サブシステムの概念ドキュメントとリファレンスを取り込み、リポジトリ内のおそらくの
  ファイルパスを特定する。
- オンボーディング。新規ユーザーのセットアップ/クイックスタートの質問に、エージェントの
  古い学習データではなく公式の入門ページから答える。

## 中核原則: llms.txt がルーターである

Ralph の llms.txt は、全文ダンプではなく厳選されたマップです。ワークフローは次のとおりです。

1. **発見** — `llms.txt` を取得して、トップレベルのセクション（Getting Started /
   Concepts / User Guide / Advanced / API / Examples / Contributing / Reference）を
   把握する。
2. **絞り込み** — 質問に実際に答える 1〜3 ページを選ぶ。マップは `.md` 版へ直接
   リンクしている。これらはエージェント最適化されており、HTML をスクレイピングするより
   優先すべきである。
3. **取得** — それらのページのみを取得する。当てずっぽうに 3 ページを超えて取得しない。
   予算は閲覧ではなく回答に費やすべきである。
4. **相互確認** — コードレベルの主張については、リポジトリと照合する（クレートのパスは
   ralph-orchestrator チェックアウト内の AGENTS.md / CLAUDE.md に記載されている）。
5. **回答** — 根拠としたページを引用する。ユーザーが検証できるよう URL を含める。

## ワークフロー

1. `references/llms-txt-map.md` の分類（ハット、イベントループ、メモリ、タスク、
   バックエンド、プリセット、CLI、TUI、診断、波、API）を使って、質問のサブシステムを
   特定します。
2. `~/.cache/ralph-docs/llms.txt` が存在し、7 日以内であればそれを使います。そうでなければ
   再取得します。

   ```bash
   mkdir -p ~/.cache/ralph-docs
   curl -sSfL https://mikeyobrien.github.io/ralph-orchestrator/llms.txt \
     -o ~/.cache/ralph-docs/llms.txt
   ```

3. サブシステムに最も関連する 1〜3 ページの `.md` を選びます。マップの項目は
   `references/llms-txt-map.md` に記載されています。grep の近道として使ってください。
4. `curl -sSfL <url> -o ~/.cache/ralph-docs/<stem>.md` でそれらのページだけを取得して
   読みます。`web_fetch` や同等のツールを持つエージェントは、そちらを使ってください。
5. たった今読んだ内容に基づいてユーザーの質問に答えます。ユーザーが「Ralph は X を行うか？」
   と尋ねた場合は、監査できるよう該当の一文を引用します。
6. 回答にコード変更が必要な場合は、ralph-orchestrator のチェックアウトに切り替え、変更提案の
   ワークフローについては `references/contributing.md` に従います。

## スコープの境界

- このスキルは回答と説明を行います。ユーザーのハットの作成・変更は **ralph-hats** に
  委ねます。稼働中のループの操作（実行、再開、マージ、デバッグ）は **ralph-loop** に
  委ねます。ralph-orchestrator 自体へのコード変更について、このスキルは変更の範囲を
  絞りますが、実際の編集はエージェント本来のコード編集ツールで行います。
- 公開ドキュメントに存在しない機能を捏造しないでください。llms.txt で答えが見つからない
  場合はその旨を伝え、<https://github.com/mikeyobrien/ralph-orchestrator> のソースツリーの
  確認を提案してください。
- バージョンに依存する主張（CLI フラグ、プリセット名など）については、エージェントの
  事前学習に頼らないでください。Ralph の CLI は進化します。常に
  `guide/cli-reference.md` または `reference/changelog.md` と照合してください。

## ガードレール

- レンダリング済み HTML のスクレイピングより、llms.txt の `.md` URL を優先します。
- 取得したページは `~/.cache/ralph-docs/` にキャッシュし、7 日を陳腐化のしきい値とします。
  リネームや移動を検出するため、他のドキュメントより先に llms.txt を再取得します。
- 引用したドキュメントがローカルのチェックアウトと矛盾する場合（ドキュメントがユーザーの
  インストール済み ralph バージョンより新しい場合）は、その不一致を指摘し、どちらを
  信頼するかユーザーが判断できるよう `ralph --version` を提案します。
- 「Ralph を改善する」依頼では、まず必ず `concepts/tenets/index.md` を読みます。Ralph の
  6 つの信条は根幹をなします。これらに逆らう変更は、たいてい別の場所に属します。

## 出力に期待されること

- まず回答し、それからリンクします。冗長なツアーでユーザーを待たせないでください。
- 自明でない主張には、少なくとも 1 つのソース URL を含めます。
- コード変更を提案するときは、クレートとファイル（クレートマップは
  `references/contributing.md` を参照）、変更を正当化する概念ドキュメント、それをカバー
  すべきテストファイルを挙げます。

## 必要に応じて参照するリファレンス

- llms.txt のセクションマップと、どのページがどの質問に答えるか:
  `references/llms-txt-map.md`
- FAQ のレシピ（よくある内省パターン）:
  `references/common-questions.md`
- クレート構成や PR の慣習を含む、コード変更の提案方法:
  `references/contributing.md`
