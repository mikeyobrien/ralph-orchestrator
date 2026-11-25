# コスト管理ガイド

AI オーケストレーションを大規模に実行するとき、効果的なコスト管理は極めて重要です。この
ガイドは、タスクの品質を保ちながら支出を最適化するのに役立ちます。

## コストを理解する

### トークンの価格

100 万トークンあたりの現在の価格:

| エージェント | 入力コスト | 出力コスト | タスクあたりの平均コスト |
|-------|------------|-------------|---------------|
| **Claude** | $3.00 | $15.00 | $5-50 |
| **Q Chat** | $0.50 | $1.50 | $1-10 |
| **Gemini** | $0.50 | $1.50 | $1-10 |

### コストの計算

```python
total_cost = (input_tokens / 1_000_000 * input_price) + 
             (output_tokens / 1_000_000 * output_price)
```

**例:**
- タスクが入力 10 万トークン、出力 5 万トークンを使う
- Claude の場合: (0.1 × $3) + (0.05 × $15) = $1.05
- Q Chat の場合: (0.1 × $0.50) + (0.05 × $1.50) = $0.125

## コスト制御の仕組み

### 1. ハード上限

支出の最大上限を設定します。

```bash
# 厳密な $10 の上限
python ralph_orchestrator.py --max-cost 10.0

# 保守的なトークン上限
python ralph_orchestrator.py --max-tokens 100000
```

### 2. コンテキスト管理

賢いコンテキスト処理でトークン使用を削減します。

```bash
# 積極的なコンテキスト管理
python ralph_orchestrator.py \
  --context-window 50000 \
  --context-threshold 0.6  # 60% で要約する
```

### 3. エージェントの選択

費用対効果の高いエージェントを選びます。

```bash
# 開発: より安価なエージェントを使う
python ralph_orchestrator.py --agent q --max-cost 5.0

# 本番: 上限付きで品質の高いエージェントを使う
python ralph_orchestrator.py --agent claude --max-cost 50.0
```

## 最適化の戦略

### 1. 階層的なエージェント戦略

タスクのフェーズごとに異なるエージェントを使います。

```bash
# フェーズ 1: Q でリサーチ（安価）
echo "Research the problem" > research.md
python ralph_orchestrator.py --agent q --prompt research.md --max-cost 2.0

# フェーズ 2: Claude で実装（品質）
echo "Implement the solution" > implement.md
python ralph_orchestrator.py --agent claude --prompt implement.md --max-cost 20.0

# フェーズ 3: Q でテスト（安価）
echo "Test the solution" > test.md
python ralph_orchestrator.py --agent q --prompt test.md --max-cost 2.0
```

### 2. プロンプトの最適化

効率的なプロンプトでトークン使用を削減します。

#### 変更前（高価）
```markdown
Please create a comprehensive web application with the following features:
- User authentication system with registration, login, password reset
- Dashboard with charts and graphs
- API with full CRUD operations
- Complete test suite
- Detailed documentation
[... 5000 tokens of requirements ...]
```

#### 変更後（最適化）
```markdown
Build user auth API:
- Register/login endpoints
- JWT tokens
- PostgreSQL storage
- Basic tests
See spec.md for details.
```

### 3. コンテキストウィンドウの管理

#### 自動要約

```bash
# トークンを節約するため早めに要約を起動する
python ralph_orchestrator.py \
  --context-window 100000 \
  --context-threshold 0.5  # 50% で要約する
```

#### 手動のコンテキスト制御

```markdown
## Context Management
When context reaches 50%, summarize:
- Keep only essential information
- Remove completed task details
- Compress verbose outputs
```

### 4. イテレーションの最適化

より少なく、より賢いイテレーションが費用を節約します。

```bash
# 多くの手早いイテレーション（高価）
python ralph_orchestrator.py --max-iterations 100  # ❌

# より少なく、焦点を絞ったイテレーション（経済的）
python ralph_orchestrator.py --max-iterations 20   # ✅
```

## コストの監視

### リアルタイムの追跡

実行中にコストを監視します。

```bash
# 詳細なコスト報告
python ralph_orchestrator.py \
  --verbose \
  --metrics-interval 1
```

**出力:**
```
[INFO] Iteration 5: Tokens: 25,000 | Cost: $1.25 | Remaining: $48.75
```

### コストレポート

詳細なコストの内訳にアクセスします。

```python
import json
from pathlib import Path

# メトリクスを読み込む
metrics_dir = Path('.agent/metrics')
total_cost = 0

for metric_file in metrics_dir.glob('metrics_*.json'):
    with open(metric_file) as f:
        data = json.load(f)
        total_cost += data.get('cost', 0)

print(f"Total cost: ${total_cost:.2f}")
```

### コストダッシュボード

監視用のダッシュボードを作成します。

```python
#!/usr/bin/env python3
import json
import matplotlib.pyplot as plt
from pathlib import Path

costs = []
iterations = []

for metric_file in sorted(Path('.agent/metrics').glob('*.json')):
    with open(metric_file) as f:
        data = json.load(f)
        costs.append(data.get('total_cost', 0))
        iterations.append(data.get('iteration', 0))

plt.plot(iterations, costs)
plt.xlabel('Iteration')
plt.ylabel('Cumulative Cost ($)')
plt.title('Ralph Orchestrator Cost Progression')
plt.savefig('cost_report.png')
```

## 予算計画

### タスクのコスト見積もり

| タスク種別 | 複雑さ | 推奨予算 | エージェント |
|-----------|------------|-------------------|--------|
| 単純なスクリプト | 低 | $0.50 - $2 | Q Chat |
| Web API | 中 | $5 - $20 | Gemini/Claude |
| 完全なアプリケーション | 高 | $20 - $100 | Claude |
| データ分析 | 中 | $5 - $15 | Gemini |
| ドキュメント | 低〜中 | $2 - $10 | Q/Claude |
| デバッグ | 可変 | $5 - $50 | Claude |

### 月次予算の計画

```python
# 月次予算の必要額を計算する
tasks_per_month = 50
avg_cost_per_task = 10.0
safety_margin = 1.5

monthly_budget = tasks_per_month * avg_cost_per_task * safety_margin
print(f"Recommended monthly budget: ${monthly_budget}")
```

## コスト最適化プロファイル

### 最小コストプロファイル

最大の節約、許容できる品質:

```bash
python ralph_orchestrator.py \
  --agent q \
  --max-tokens 50000 \
  --max-cost 2.0 \
  --context-window 30000 \
  --context-threshold 0.5 \
  --checkpoint-interval 10
```

### バランスプロファイル

良い品質、妥当なコスト:

```bash
python ralph_orchestrator.py \
  --agent gemini \
  --max-tokens 200000 \
  --max-cost 10.0 \
  --context-window 100000 \
  --context-threshold 0.7 \
  --checkpoint-interval 5
```

### 品質プロファイル

最良の結果、制御された支出:

```bash
python ralph_orchestrator.py \
  --agent claude \
  --max-tokens 500000 \
  --max-cost 50.0 \
  --context-window 200000 \
  --context-threshold 0.8 \
  --checkpoint-interval 3
```

## 高度なコスト管理

### 動的なエージェント切り替え

残り予算に基づいてエージェントを切り替えます。

```python
# 動的切り替えの擬似コード
if remaining_budget > 20:
    agent = "claude"
elif remaining_budget > 5:
    agent = "gemini"
else:
    agent = "q"
```

### コストを意識したプロンプト

プロンプトにコストの考慮を含めます。

```markdown
## Budget Constraints
- Maximum budget: $10
- Optimize for efficiency
- Skip non-essential features if approaching limit
- Prioritize core functionality
```

### バッチ処理

複数の小さなタスクをまとめます。

```bash
# 非効率: 複数のオーケストレーション
python ralph_orchestrator.py --prompt task1.md  # $5
python ralph_orchestrator.py --prompt task2.md  # $5
python ralph_orchestrator.py --prompt task3.md  # $5
# 合計: $15

# 効率的: バッチ化したオーケストレーション
cat task1.md task2.md task3.md > batch.md
python ralph_orchestrator.py --prompt batch.md  # $10
# 合計: $10（33% の節約）
```

## コストアラート

### アラートの設定

```bash
#!/bin/bash
# cost_monitor.sh

COST_LIMIT=25.0
CURRENT_COST=$(python -c "
import json
with open('.agent/metrics/state_latest.json') as f:
    print(json.load(f)['total_cost'])
")

if (( $(echo "$CURRENT_COST > $COST_LIMIT" | bc -l) )); then
    echo "ALERT: Cost exceeded $COST_LIMIT" | mail -s "Ralph Cost Alert" admin@example.com
fi
```

### 自動停止

サーキットブレーカーを実装します。

```python
# cost_breaker.py
import json
import sys

with open('.agent/metrics/state_latest.json') as f:
    state = json.load(f)
    
if state['total_cost'] > state['max_cost'] * 0.9:
    print("WARNING: 90% of budget consumed")
    sys.exit(1)
```

## ROI 分析

### ROI の計算

```python
# ROI の計算
hours_saved = 10  # 節約された手作業の時間
hourly_rate = 50  # 開発者の時給
ai_cost = 25  # AI オーケストレーションのコスト

value_created = hours_saved * hourly_rate
roi = (value_created - ai_cost) / ai_cost * 100

print(f"Value created: ${value_created}")
print(f"AI cost: ${ai_cost}")
print(f"ROI: {roi:.1f}%")
```

### コスト便益マトリクス

| タスク | 手作業の時間 | 手作業のコスト | AI コスト | 節約 |
|------|-------------|-------------|---------|---------|
| API 開発 | 40h | $2000 | $50 | $1950 |
| ドキュメント | 20h | $1000 | $20 | $980 |
| テストスイート | 30h | $1500 | $30 | $1470 |
| バグ修正 | 10h | $500 | $25 | $475 |

## ベストプラクティス

### 1. 小さく始める

まず最小の予算でテストします。

```bash
# テスト実行
python ralph_orchestrator.py --max-cost 1.0 --max-iterations 5

# うまくいったら規模を拡大する
python ralph_orchestrator.py --max-cost 10.0 --max-iterations 50
```

### 2. 継続的に監視する

コストをリアルタイムで追跡します。

```bash
# ターミナル 1: オーケストレーションを実行する
python ralph_orchestrator.py --verbose

# ターミナル 2: コストを監視する
watch -n 5 'tail -n 20 .agent/metrics/state_latest.json'
```

### 3. 反復的に最適化する

- コストレポートを分析する
- 高価な操作を特定する
- プロンプトと設定を洗練する
- 最適化をテストする

### 4. 現実的な予算を設定する

- 開発: 本番予算の 50%
- テスト: 本番予算の 25%
- 本番: 安全マージン付きの全予算

### 5. コストを記録する

分析のために記録を残します。

```bash
# 各実行の後にコストレポートを保存する
python ralph_orchestrator.py && \
  cp .agent/metrics/state_latest.json "reports/run_$(date +%Y%m%d_%H%M%S).json"
```

## トラブルシューティング

### よくある問題

1. **予想外の高コスト**
   - メトリクスのトークン使用を確認する
   - プロンプトの効率を見直す
   - コンテキスト設定を検証する

2. **予算がすぐに超過する**
   - コンテキストウィンドウを下げる
   - 要約のしきい値を上げる
   - より安価なエージェントを使う

3. **予算制約で結果が悪い**
   - 予算を少し増やす
   - プロンプトを最適化する
   - 段階的なアプローチを検討する

## 次のステップ

- 費用対効果の高い選択について [エージェントの選択](agents.ja.md) を見直す
- 効率のために [プロンプト](prompts.ja.md) を最適化する
- コスト最適化されたパターンについて [例](../examples/index.ja.md) を探る
