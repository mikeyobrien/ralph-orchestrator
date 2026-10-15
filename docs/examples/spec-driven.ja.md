# 仕様駆動開発の例

!!! note "例のみ"
    仕様駆動開発は現在、組み込みプリセットとして出荷されるのではなく、例のパターンとして
    文書化されています。

## 概要

この例は、実装が始まる前に要件を形式化する、仕様優先のワークフローを示します。

今日サポートされている組み込みが欲しい場合は、実装作業には `builtin:code-assist`、
アイデアからコードへの長いフローには `builtin:pdd-to-code-assist` から始めてください。

例のみの自動設計ワークフローが特に欲しい場合は、[自動 PDD 設計](pdd-design.ja.md) と、
その例プリセット `docs/examples/presets/auto-pdd.yml` を参照してください。

## ワークフロー

1. `.ralph/specs/{spec_name}/` に仕様/設計の成果物を作成する
2. スペックをレビューし承認する
3. 実装タスクを生成する
4. Ralph のオーケストレーションで実行する

## スペックの例

```markdown
# Feature: User Authentication

## Given
- User registration system exists

## When
- User provides valid credentials

## Then
- User receives authentication token
- Session is established
```

## 関連項目

- [TDD ワークフロー](tdd-workflow.ja.md) - テスト優先のアプローチ
- [シンプルなタスク](simple-task.ja.md) - 基本的な例
- [プロンプトの書き方](../guide/prompts.ja.md) - プロンプトのベストプラクティス
