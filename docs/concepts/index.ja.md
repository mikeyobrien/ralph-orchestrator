# 概念

Ralph の中核となる概念を理解すると、効果的に使えるようになります。

## 概要

Ralph は、いくつかの重要な考え方を軸に構築されています。

1. **[Ralph Wiggum テクニック](ralph-wiggum-technique.ja.md)** — 成功するまでの継続的な反復
2. **[6 つの信条](tenets.ja.md)** — オーケストレーションの指針となる原則
3. **[ハットとイベント](hats-and-events.ja.md)** — 型付きイベントを通じて連携する専門ペルソナ
4. **[連携パターン](coordination-patterns.ja.md)** — マルチエージェントのワークフローアーキテクチャ
5. **[メモリとタスク](memories-and-tasks.ja.md)** — 永続的な学習とランタイムの作業追跡
6. **[バックプレッシャー](backpressure.ja.md)** — 不完全な作業を拒否する品質ゲート

## 中核となる哲学

> 「オーケストレーターは薄い調整レイヤーであり、プラットフォームではない。Ralph は賢い。
> 仕事は Ralph にさせよう。」

Ralph は意図的に単純です。複雑な機能をオーケストレーターに組み込むのではなく、Ralph は
次を行います。

- 実際の作業は**エージェントに委ねる**
- ハットとイベントを通じて**構造を提供する**
- バックプレッシャーゲートを通じて**品質を強制する**
- ディスク上のファイルを通じて**状態を維持する**

## 従来型 vs ハットベースモード

Ralph は 2 つのオーケストレーションスタイルをサポートします。

### 従来型モード

完了まで実行される単純なループ:

```yaml
cli:
  backend: "claude"

event_loop:
  completion_promise: "LOOP_COMPLETE"
  max_iterations: 100
```

エージェントは `LOOP_COMPLETE` を出力するか、上限に達するまで反復します。

### ハットベースモード

専門ペルソナがイベントを通じて連携します。

```yaml
cli:
  backend: "claude"

event_loop:
  starting_event: "task.start"
  completion_promise: "LOOP_COMPLETE"

hats:
  planner:
    triggers: ["task.start"]
    publishes: ["plan.ready"]
    instructions: "Create a plan..."

  builder:
    triggers: ["plan.ready"]
    publishes: ["build.done"]
    instructions: "Implement the plan..."
```

イベントがハット間を流れ、それぞれがタスクに寄与します。

## 主要概念の要約

| 概念 | 説明 |
|---------|-------------|
| **イテレーション** | オーケストレーションループの 1 サイクル |
| **完了の約束（Completion Promise）** | ループを終わらせるシグナル（既定: `LOOP_COMPLETE`） |
| **ハット** | 特定のトリガーと挙動を持つ専門の Ralph ペルソナ |
| **イベント** | ハットを起動し状態を運ぶ、型付きメッセージ |
| **バックプレッシャー** | 悪い作業を拒否する品質ゲート（テスト、lint、型チェック） |
| **メモリ** | `.ralph/agent/memories.md` に保存される永続的な学習 |
| **タスク** | `.ralph/agent/tasks.jsonl` に保存されるランタイムの作業項目 |

## 次のステップ

- [Ralph Wiggum テクニック](ralph-wiggum-technique.ja.md) を理解する
- Ralph の設計を導く [6 つの信条](tenets.ja.md) を学ぶ
- 複雑なワークフローのために [ハットとイベント](hats-and-events.ja.md) を習得する
