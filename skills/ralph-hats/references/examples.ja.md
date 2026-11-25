# Ralph ハットの例

## パイプラインの例

ワークフローが直線的な場合に使います。

```yaml
event_loop:
  starting_event: "work.start"
  completion_promise: "LOOP_COMPLETE"

hats:
  planner:
    name: "Planner"
    description: "作業のスコープを決める"
    triggers: ["work.start"]
    publishes: ["plan.ready"]
    default_publishes: "plan.ready"
    instructions: |
      タスクを明確な計画に分解する。

  builder:
    name: "Builder"
    description: "計画を実装する"
    triggers: ["plan.ready"]
    publishes: ["build.done"]
    default_publishes: "build.done"
    instructions: |
      承認された計画を実装する。

  reviewer:
    name: "Reviewer"
    description: "結果を検証する"
    triggers: ["build.done"]
    publishes: ["LOOP_COMPLETE"]
    default_publishes: "LOOP_COMPLETE"
    instructions: |
      実装をレビューし、実行を完了する。
```

## レビューループの例

ワークフローに反復と差し戻しが必要な場合に使います。

```yaml
event_loop:
  starting_event: "review.start"
  completion_promise: "REVIEW_COMPLETE"

events:
  review.section:
    description: "主要なレビューセクションが、より深い分析の準備が整った状態"
  analysis.complete:
    description: "現在のレビュー波における深掘り分析が完了した状態"

hats:
  reviewer:
    name: "Reviewer"
    description: "最初のレビューパスを実施する"
    triggers: ["review.start", "review.followup"]
    publishes: ["review.section"]
    default_publishes: "review.section"
    instructions: |
      次のレビューセクションを作成する。

  analyzer:
    name: "Analyzer"
    description: "最もリスクの高い発見事項を深掘りする"
    triggers: ["review.section"]
    publishes: ["analysis.complete"]
    default_publishes: "analysis.complete"
    instructions: |
      最もリスクの高いレビュー領域を分析する。

  closer:
    name: "Closer"
    description: "続行するか完了するかを判断する"
    triggers: ["analysis.complete"]
    publishes: ["review.followup", "REVIEW_COMPLETE"]
    instructions: |
      もう一度レビューの波が必要かどうかを判断する。
```
