# 自動 PDD 設計の例

!!! note "例のみ"
    `docs/examples/presets/auto-pdd.yml` は例のワークフローであり、サポートされる組み込み
    プリセットではありません。

## 概要

この例は、PDD の前半を自動化します。人間が発見の質問に答えるために一時停止する代わりに、
2 つのハットが要件インタビューを模擬します。

1. `Requirements Interviewer` が一度に 1 つの鋭い質問をする。
2. `Requirements Owner` が、プロンプト、リポジトリのコンテキスト、明示的な仮定から答える。

インタビューが完了すると、`PDD Author` が設計パッケージを書き、`Design Critic` が敵対的に
それをレビューします。ループは、その設計パッケージが承認された後にのみ停止します。

## 出力

このワークフローは、`.ralph/specs/{spec_name}/` の下に PDD スタイルのパッケージを書き
込みます。

- `rough-idea.md`
- `idea-honing.md`
- `requirements.md`
- `design.md`

実装タスクの生成やコードの記述は行いません。

## 使い方

```bash
ralph run --config docs/examples/presets/auto-pdd.yml --prompt "Design a resilient import pipeline for CSV uploads"
```

## なぜ使うのか

次を望むときに使います。

- おおまかなプロンプトからの自動設計パス
- ブロックされる人間の Q&A の代わりに明示的な仮定
- 実装が始まる前の敵対的な設計ゲート

実装まで続く、よりサポートされた長いワークフローが欲しい場合は、代わりに
`builtin:pdd-to-code-assist` を使ってください。
