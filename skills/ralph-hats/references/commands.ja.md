# Ralph ハットのコマンド

## ハットファイルの作成・編集

ユーザーが作成したハットコレクションは `.ralph/hats/` 配下に置きます。

```bash
mkdir -p .ralph/hats
$EDITOR .ralph/hats/my-workflow.yml
```

## コレクションの検証

```bash
ralph hats validate -c ralph.yml -H .ralph/hats/my-workflow.yml
```

これにより、次の問題を検出します。

- 予約されたトリガー
- 曖昧なルーティング
- 開始イベントの購読者が存在しない
- どこにも購読されない孤立した公開イベント

## トポロジの検査

```bash
ralph hats graph -c ralph.yml -H .ralph/hats/my-workflow.yml --format ascii
ralph hats graph -c ralph.yml -H .ralph/hats/my-workflow.yml --format mermaid
ralph hats show -c ralph.yml -H .ralph/hats/my-workflow.yml planner
```

ワークフローの説明が欲しいときは `graph` を使います。1 つのハットを詳しく確認する必要が
あるときは `show` を使います。

## ワークフローの試行

```bash
ralph run -c ralph.yml -H .ralph/hats/my-workflow.yml -p "Add OAuth login"
```

実行前に手早く確認したい場合は次のようにします。

```bash
ralph run -c ralph.yml -H .ralph/hats/my-workflow.yml -p "Add OAuth login" --dry-run
```

## 改善ループ

既存のハットファイルをリファクタリングするときは、次の手順で行います。

1. 現在の YAML を読む
2. 現在のトポロジを説明する
3. 問題を解決する最小限の構造的改善を提案する
4. `ralph hats validate` を再実行する
5. 役立つ場合は `ralph hats graph` で再描画する
