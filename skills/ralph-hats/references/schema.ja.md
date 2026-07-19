# Ralph ハットのスキーマに関するメモ

このリファレンスは、次のように使用するユーザー作成のハットファイルを対象としています。

```bash
ralph run -c ralph.yml -H .ralph/hats/<name>.yml -p "..."
```

## サポートされるトップレベルの形

ハットファイルは次の範囲に限定して記述します。

```yaml
name: "任意: コレクション名"
description: "任意: コレクションの説明"

event_loop:
  starting_event: "work.start"
  completion_promise: "LOOP_COMPLETE"

events:
  work.start:
    description: "委譲された開始イベント"
    on_trigger: "ワークフローを開始する"
    on_publish: "コーディネーターが作業を開始すべきときに発行する"

hats:
  planner:
    name: "Planner"
    description: "作業を計画する"
    triggers: ["work.start"]
    publishes: ["plan.ready"]
    default_publishes: "plan.ready"
    instructions: |
      タスクを計画する。
```

## これらはハットファイルではなくコア設定に置く

汎用的なランタイム設定をハットファイルに入れないでください。次のような設定は、代わりに
コアの `-c` 設定に残します。

- `max_iterations`
- `max_runtime_seconds`
- `required_events`
- バックエンド全体の CLI 設定
- memories/tasks/hooks の設定

ハットファイルでは、`event_loop` オーバーレイは、現在 Ralph がハットオーバーレイから
マージするキーにのみ使用します。

- `starting_event`
- `completion_promise`

## 重要となる現在のハットフィールド

各ハットは、現在 Ralph がサポートするフィールドを使用します。

- `name`
- `description`
- `triggers`
- `publishes`
- `instructions`
- `default_publishes`
- `extra_instructions`
- `backend`
- `backend_args`
- `max_activations`
- `disallowed_tools`

メモ:

- `description` は実質的に必須であり、決して空にしてはいけません。
- `default_publishes` はリストではなく単一の文字列です。
- `backend_args` は省略形のキー `args` も受け付けます。

## 予約トリガーのルール

次のものをハットのトリガーに割り当てないでください。

- `task.start`
- `task.resume`

Ralph はこれらをコーディネーター用に予約しています。代わりに委譲された意味を持つイベントを
使い、`event_loop.starting_event` をそのイベントに設定してください。

良い例:

- `work.start`
- `review.start`
- `research.start`
- `build.task`

## イベントメタデータ

カスタムトピックに説明が必要なときは `events:` を使います。Ralph は次をサポートします。

- `description`
- `on_trigger`
- `on_publish`

これは `plan.ready`、`review.section`、`investigation.blocked` のようなカスタム
トピックで特に役立ちます。
