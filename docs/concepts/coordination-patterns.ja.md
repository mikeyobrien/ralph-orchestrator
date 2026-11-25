# 連携パターン

Ralph のハットシステムは、イベント駆動の連携を通じて高度なマルチエージェントワークフローを
可能にします。このセクションでは、アーキテクチャのパターン、イベントルーティングの仕組み、
組み込みのワークフローテンプレートを扱います。

## ハットベースのオーケストレーションの仕組み

### イベント駆動のモデル

ハットは **pub/sub イベントシステム** を通じて通信します。

1. **Ralph が開始イベントを公開する**（例: `task.start`）
2. **一致するハットが起動する** — そのイベントを購読しているハットが引き継ぐ
3. **ハットが作業を行い**、完了時にイベントを公開する
4. **次のハットが起動する** — 新しいイベントによって起動される
5. **サイクルが続く** — 終了イベントまたは `LOOP_COMPLETE` まで

```
┌─────────────────────────────────────────────────────────────────┐
│  task.start → [Test Writer] → test.written → [Implementer] →   │
│  test.passing → [Refactorer] → refactor.done ──┐                │
│                                                │                │
│  ┌─────────────────────────────────────────────┘                │
│  └──→ (次のテストのために Test Writer に戻る)                    │
└─────────────────────────────────────────────────────────────────┘
```

### 恒常的なコーディネーターとしての Ralph

ハットベースモードでは、**Ralph は常に存在します**。

- Ralph は削除も置換もできない
- カスタムハットは**トポロジ**（誰が誰を起動するか）を定義する
- Ralph は**トポロジを認識して**実行する — どのハットが存在し、その関係がどうなっているかを
  知っている
- Ralph は**普遍的なフォールバック**として機能する — 孤立したイベントは自動的に Ralph に
  ルーティングされる

つまり、カスタムハットは直接実行されるわけではありません。代わりに、Ralph はすべてのハットに
またがる保留中のイベントをすべて読み、定義されたトポロジに基づいて何をするかを決めます。
その後 Ralph は次のいずれかを行います。

- イベントを公開して適切なハットに委譲する
- 適したハットがなければ、作業を直接処理する

### イベントルーティングとトピックのマッチング

イベントは、**glob スタイルのパターンマッチング** を使ってハットにルーティングされます。

| パターン | 一致するもの |
|---------|---------|
| `task.start` | ちょうど `task.start` |
| `build.*` | `build.done`, `build.blocked`, `build.task` など |
| `*.done` | `build.done`, `review.done`, `test.done` など |
| `*` | すべて（グローバルワイルドカード — Ralph がフォールバックとして使う） |

**優先順位のルール:**

- 具体的なパターンがワイルドカードより優先される
- 複数のハットが具体的な購読を持つ場合はエラー（曖昧なルーティング）
- グローバルワイルドカード（`*`）は、具体的なハンドラが存在しない場合にのみ起動する

## 連携パターン

Ralph のプリセットは、実証済みのいくつかの連携パターンを実装しています。

### 1. 直線パイプライン

最も単純なパターン。作業が一連の専門家を通じて流れます。

```
入力 → Hat A → イベント → Hat B → イベント → Hat C → 出力
```

**例: TDD レッド・グリーン・リファクタ**（`tdd-red-green.yml`）

```yaml
hats:
  test_writer:
    triggers: ["tdd.start", "refactor.done"]
    publishes: ["test.written"]

  implementer:
    triggers: ["test.written"]
    publishes: ["test.passing"]

  refactorer:
    triggers: ["test.passing"]
    publishes: ["refactor.done", "cycle.complete"]
```

```
tdd.start → 🔴 Test Writer → test.written → 🟢 Implementer →
test.passing → 🔵 Refactorer → refactor.done ─┐
                                              │
              ┌───────────────────────────────┘
              └──→ (Test Writer に戻る)
```

**使う場面:** 各ステップが前のステップの上に積み上がる、明確な順次フェーズを持つワークフロー。

### 2. コントラクトファーストのパイプライン

進む前に作業が検証ゲートを通過しなければならないバリエーション。

**例: スペック駆動開発**（`spec-driven.yml`）

```yaml
hats:
  spec_writer:
    triggers: ["spec.start", "spec.rejected"]
    publishes: ["spec.ready"]

  spec_reviewer:
    triggers: ["spec.ready"]
    publishes: ["spec.approved", "spec.rejected"]

  implementer:
    triggers: ["spec.approved", "spec.violated"]
    publishes: ["implementation.done"]

  verifier:
    triggers: ["implementation.done"]
    publishes: ["task.complete", "spec.violated"]
```

```
spec.start → 📋 Spec Writer ──→ spec.ready ──→ 🔎 Spec Critic
                 ↑                                   │
                 └────── spec.rejected ──────────────┤
                                                     ↓
                                               spec.approved
                                                     │
                                                     ↓
task.complete ←── ✅ Verifier ←── impl.done ←── ⚙️ Implementer
                       │                              ↑
                       └──── spec.violated ───────────┘
```

**使う場面:** 実装を始める前にスペックが盤石でなければならない、リスクの高い変更。

### 3. 循環ローテーション

複数の役割が交代で担い、それぞれ異なる視点をもたらします。

**例: モブプログラミング**（`mob-programming.yml`）

```yaml
hats:
  navigator:
    triggers: ["mob.start", "observation.noted"]
    publishes: ["direction.set", "mob.complete"]

  driver:
    triggers: ["direction.set"]
    publishes: ["code.written"]

  observer:
    triggers: ["code.written"]
    publishes: ["observation.noted"]
```

```
mob.start → 🧭 Navigator → direction.set → ⌨️ Driver →
code.written → 👁️ Observer → observation.noted ─┐
                                                │
              ┌─────────────────────────────────┘
              └──→ (Navigator に戻る)
```

**使う場面:** 複数の視点と継続的なフィードバックから恩恵を受ける複雑な機能。

### 4. 敵対的レビュー

対立する目的を持つ 2 つの役割が堅牢性を確保します。

**例: レッドチーム / ブルーチーム**（`adversarial-review.yml`）

```yaml
hats:
  builder:
    name: "🔵 Blue Team (Builder)"
    triggers: ["security.review", "fix.applied"]
    publishes: ["build.ready"]

  red_team:
    name: "🔴 Red Team (Attacker)"
    triggers: ["build.ready"]
    publishes: ["vulnerability.found", "security.approved"]

  fixer:
    triggers: ["vulnerability.found"]
    publishes: ["fix.applied"]
```

```
security.review → 🔵 Blue Team → build.ready → 🔴 Red Team
                      ↑                            │
                      │                            ├─→ security.approved ✓
                      │                            │
                      │                            └─→ vulnerability.found
                      │                                        │
                      └────── fix.applied ←── 🛡️ Fixer ←──────┘
```

**使う場面:** セキュリティに敏感なコード、認証システム、または敵対的な思考が品質を高める
あらゆるコード。

### 5. 仮説駆動の調査

科学的手法をデバッグに適用したもの。

**例: 科学的手法**（`scientific-method.yml`）

```yaml
hats:
  observer:
    triggers: ["science.start", "hypothesis.rejected"]
    publishes: ["observation.made"]

  theorist:
    triggers: ["observation.made"]
    publishes: ["hypothesis.formed"]

  experimenter:
    triggers: ["hypothesis.formed"]
    publishes: ["hypothesis.confirmed", "hypothesis.rejected"]

  fixer:
    triggers: ["hypothesis.confirmed"]
    publishes: ["fix.applied"]
```

```
science.start → 🔬 Observer → observation.made → 🧠 Theorist →
hypothesis.formed → 🧪 Experimenter ──┬─→ hypothesis.confirmed → 🔧 Fixer
                                      │
                                      └─→ hypothesis.rejected ─┐
                                                               │
              ┌────────────────────────────────────────────────┘
              └──→ (新しいデータとともに Observer に戻る)
```

**使う場面:** 根本原因が明らかでない複雑なバグ。行き当たりばったりの修正より、体系的な調査を
強制します。

### 6. コーディネーター・スペシャリスト（ファンアウト）

コーディネーターが作業の種類に基づいて専門家に委譲します。

**例: ギャップ分析**（`gap-analysis.yml`）

```yaml
hats:
  analyzer:
    triggers: ["gap.start", "verify.complete", "report.complete"]
    publishes: ["analyze.spec", "verify.request", "report.request"]

  verifier:
    triggers: ["analyze.spec", "verify.request"]
    publishes: ["verify.complete"]

  reporter:
    triggers: ["report.request"]
    publishes: ["report.complete"]
```

```
                    ┌─→ analyze.spec ──→ 🔍 Verifier ──┐
                    │                                  │
gap.start → 📊 Analyzer ←── verify.complete ──────────┘
                    │
                    └─→ report.request ──→ 📝 Reporter ──→ report.complete
```

**使う場面:** 独立した専門タスク（分析、検証、報告）に自然に分解される作業。

### 7. 適応的なエントリポイント

ブートストラップ用のハットが入力の種類を検出し、適切なワークフローにルーティングします。

**例: Code-Assist**（`code-assist.yml`）

```yaml
hats:
  planner:
    triggers: ["build.start", "task.complete"]
    publishes: ["tasks.ready"]
    # リクエストを次の builder が着手できる作業項目に分解する

  builder:
    triggers: ["tasks.ready", "review.rejected", "finalization.failed"]
    publishes: ["review.ready", "build.blocked"]

  critic:
    triggers: ["review.ready"]
    publishes: ["review.passed", "review.rejected"]

  finalizer:
    triggers: ["review.passed"]
    publishes: ["task.complete", "finalization.failed", "LOOP_COMPLETE"]
```

```
build.start → 📋 Planner ─── (次の作業項目を選ぶ) ───→ tasks.ready
                                                            │
    ┌───────────────────────────────────────────────────────┘
    │
    ↓
⚙️ Builder ←───────────── review.rejected / finalization.failed ─────┐
    │                                                                │
    └── review.ready ──→ 🧪 Critic ──→ review.passed ──→ 🏁 Finalizer ┤
                                                          │           │
                                                          ├─→ task.complete
                                                          │      │
                                                          │      └──→ 📋 Planner が次の作業項目を選ぶ
                                                          └─→ LOOP_COMPLETE
```

**使う場面:** 複数の入力形式を扱う必要がある、またはコンテキストに応じて挙動を適応させる
必要があるワークフロー。

## カスタムハットコレクションの設計

### ハット設定のスキーマ

```yaml
hats:
  my_hat:
    name: "🎯 Display Name"      # TUI とログに表示される
    description: "What this hat does"  # 必須 — Ralph が委譲に使う
    triggers: ["event.a", "event.b"]   # このハットを起動するイベント
    publishes: ["event.c", "event.d"]  # このハットが発行できるイベント
    default_publishes: "event.c"       # ハットが emit を忘れた場合のフォールバック
    max_activations: 10                # 任意: 起動回数の上限
    backend: "claude"                  # 任意: バックエンドの上書き
    instructions: |
      Prompt injected when this hat is active.
      Tell the hat what to do, not how to do it.
```

### 設計の原則

1. **description は極めて重要** — Ralph はいつ委譲するかを決めるのにハットの description を
   使います。明確で具体的にしてください。

2. **1 ハット、1 責務** — 各ハットは明確で焦点の絞られた目的を持つべきです。description に
   「〜と〜」と書いているなら、分割を検討してください。

3. **イベントはルーティングの信号であり、データではない** — ペイロードは簡潔に保ちます。
   詳細な出力はファイルに保存し、イベントではそれを参照します。

4. **復旧を見据えて設計する** — ハットが失敗したり公開を忘れたりしても、Ralph が孤立した
   イベントを捕捉します。トポロジは予期しない状態をグレースフルに扱うべきです。

5. **まず単純なプロンプトでテストする** — 複雑なトポロジには創発的な挙動があり得ます。
   単純に始め、フローを検証してから複雑さを加えます。

### 検証のルール

Ralph はハットの設定を検証します。

- **description は必須**: すべてのハットは description を持たなければならない（Ralph が委譲の
  コンテキストに必要とする）
- **予約トリガー**: `task.start` と `task.resume` は Ralph 用に予約されている
- **曖昧なルーティングの禁止**: 各トリガーパターンはちょうど 1 つのハットに対応しなければ
  ならない

```
ERROR: Ambiguous routing for trigger 'build.done'.
Both 'planner' and 'reviewer' trigger on 'build.done'.
```

## イベントの発行

ハットは、完了を知らせたり作業を引き継いだりするためにイベントを発行します。

```bash
# ペイロード付きの単純なイベント
ralph emit "build.done" "tests: pass, lint: pass, typecheck: pass, audit: pass, coverage: pass"

# JSON ペイロード付きのイベント
ralph emit "review.done" --json '{"status": "approved", "issues": 0}'

# 特定のハットへの直接の引き継ぎ（ルーティングを迂回する）
ralph emit "handoff" --target reviewer "Please review the changes"
```

**エージェントの出力内では**、イベントは XML タグとして埋め込まれます。

```xml
<event topic="impl.done">Implementation complete</event>
<event topic="handoff" target="reviewer">Please review</event>
```

## パターンの選び方

| シナリオ | 推奨パターン | プリセット |
|----------|---------------------|--------|
| 明確なフェーズを持つ順次ワークフロー | 直線パイプライン | `tdd-red-green` |
| コーディング前にスペックの承認が必要 | コントラクトファースト | `spec-driven` |
| 複数の視点が必要 | 循環ローテーション | `mob-programming` |
| セキュリティレビューが必要 | 敵対的 | `adversarial-review` |
| 複雑な問題のデバッグ | 仮説駆動 | `scientific-method` |
| 作業が専門タスクに分解される | コーディネーター・スペシャリスト | `gap-analysis` |
| 複数の入力形式 | 適応的なエントリ | `code-assist` |
| 標準的な機能開発 | 基本的な委譲 | `feature` |

## ハットを使わない方がよいとき

ハットベースのオーケストレーションは複雑さを増やします。次の場合は**従来型モード**（ハット
なし）を使ってください。

- タスクが単純で、単一の焦点である
- 役割の分離や引き継ぎが不要である
- プロトタイピング中で、設定を最小限にしたい
- 作業が別個のフェーズに自然に分解されない

従来型モードは、単に完了までループする Ralph です。よりシンプルで、セットアップが速く、
多くの場合それで十分です。

## 次のステップ

- [ハットとイベント](hats-and-events.ja.md) の基本を学ぶ
- 既成のワークフローについて [プリセット](../guide/presets.ja.md) を探す
- 実装の詳細については [カスタムハットの作成](../advanced/custom-hats.ja.md) を見る
