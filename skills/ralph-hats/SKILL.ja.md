---
name: ralph-hats
description: Create, inspect, validate, explain, and improve Ralph hat collections. Use this skill whenever the user asks to make or refine a `.ralph/hats/*.yml` workflow, debug hat routing, explain event topology, or tune a multi-hat Ralph run.
---

# Ralph ハット

このスキルは、ユーザーが作成したハットコレクションに対して、Ralph のハットのライフサイクル
全体を操作するために使用します。

## このスキルを使う場面

- `.ralph/hats/` に新しいハットコレクションを作成する
- 既存のハットコレクションを検査し、そのトポロジを説明する
- トリガーのルーティング、イベントフロー、完了時の挙動を検証する
- 役割をより明確にし、ルーティングをより安全にするためにハットを改善・リファクタリングする
- Ralph ワークフローにより適したオーケストレーションパターンを提案する

## 前提となる想定

- コアランタイム設定はすでに `ralph.yml` などの `-c` ソースに存在する。
- ユーザーが作成したハットは別に保存され、`-H` で渡される。
- このスキルは公開ハットコレクションを操作するものであり、Ralph 組み込みのプリセットは
  対象外である。

## ワークフロー

1. ハットファイルがすでに存在する場合は、まずそれを読み、変更を提案する前に現在の
   トポロジを説明します。
2. 新しいワークフローを作成する場合は、`.ralph/hats/<name>.yml` に書き込みます。
3. ハットファイルはハット関連のデータのみに絞ります。ランタイムの上限値やその他のコア設定は
   メインの設定ファイルに残します。
4. `ralph hats validate` で検証します。
5. イベントフローが単純でない場合は、`ralph hats graph` でトポロジを可視化します。
6. 1 つのハットの実効設定を確認する必要があるときは `ralph hats show <hat>` を使います。
7. より高い確信が必要な場合は、対象を絞った `ralph run -c ... -H ... -p "..."` の試行を
   実行するか、正確なテストコマンドを提示します。

## ガードレール

- ハットファイルのトップレベルキーは、現在 Ralph が受け付けるものだけを使います。
  `name`、`description`、`events`、`event_loop`、`hats`。
- ハットファイルの `event_loop` は、`starting_event` や `completion_promise` のような
  ハットオーバーレイ用のキーにのみ使用します。
- `task.start` や `task.resume` をハットのトリガーに使わないでください。Ralph はこれらを
  調整用に予約しています。`work.start`、`review.start`、`research.start` のような意味を
  持つ委譲イベントを使ってください。
- 各トリガーはちょうど 1 つのハットにルーティングされなければなりません。
- すべてのハットで `description` を必ず記入してください。
- カスタムイベント名では意図が不明瞭になる場合は、`events:` メタデータを優先してください。
- このスキルからユーザーのワークフローを `presets/` に書き込まないでください。

## 出力に期待されること

- ハットを編集・作成する場合は、ファイルの変更内容と検証結果を提示します。
- 検査のみの場合は、簡潔なトポロジの要約、主なリスク、具体的な改善案を提示します。

## 必要に応じて参照するリファレンス

- 現在のハットスキーマとサポートされるフィールド: `references/schema.md`
- コマンドのレシピと検証ワークフロー: `references/commands.md`
- パターンとファイルの例: `references/examples.md`
