# 監視と可観測性

## 概要

Ralph の実行を効果的に監視することは、問題の早期発見、パフォーマンスの最適化、そして
オーケストレーションが期待どおりに進んでいることの確認に役立ちます。このガイドでは、
利用可能な監視ツールとテクニックを解説します。

## 組み込みの監視

### 状態ファイル

Ralph は `.agent/metrics/` に状態情報を書き込みます。

```
📊 Metrics Collection Flow

╭───────────────╮
│  Ralph Loop   │
╰───────────────╯
  │
  │
  ∨
┌───────────────┐
│  Iteration N  │
└───────────────┘
  │
  │
  ∨
┌───────────────┐
│ Write Metrics │
└───────────────┘
  │
  │
  ∨
┌────────────────────────┐
│ .agent/metrics/         │
│  state_<timestamp>.json │
└────────────────────────┘
```

<details>
<summary>graph-easy のソース</summary>

```
graph { label: "📊 Metrics Collection Flow"; flow: south; }
[ Ralph Loop ] { shape: rounded; } -> [ Iteration N ]
[ Iteration N ] -> [ Write Metrics ]
[ Write Metrics ] -> [ .agent/metrics/\nstate_<timestamp>.json ]
```

</details>

### 状態ファイルの形式

```json
{
  "iteration": 15,
  "timestamp": "2024-01-21T10:30:00Z",
  "status": "running",
  "elapsed_time": 450.5,
  "total_cost": 2.45,
  "agent": "claude",
  "prompt_file": "PROMPT.md",
  "checkpoints_created": 5,
  "errors_count": 0,
  "task_complete": false
}
```

## リアルタイムの監視

### コンソール出力

Ralph の実行中は、ライブの進行状況が表示されます。

```bash
./ralph status
```

出力例:

```
Ralph Orchestrator Status
==========================
Iteration: 15/100
Elapsed: 7m 30s
Cost: $2.45 / $10.00
Agent: claude
Status: Running
Last checkpoint: iteration_15
```

### 詳細ロギング

詳細な出力を有効にします。

```bash
python ralph_orchestrator.py --verbose
```

これは次を表示します。
- エージェントへの完全なプロンプト
- 生のエージェント応答
- ファイル操作の詳細
- タイミング情報

## メトリクス収集

### 組み込みのメトリクス

`MetricsCollector` クラスが自動的に追跡します。

```python
class MetricsCollector:
    def __init__(self):
        self.metrics = {
            'iterations': [],
            'costs': [],
            'errors': [],
            'checkpoints': []
        }

    def record_iteration(self, data):
        self.metrics['iterations'].append({
            'number': data['iteration'],
            'duration': data['duration'],
            'success': data['success'],
            'timestamp': datetime.now()
        })

    def get_summary(self):
        return {
            'total_iterations': len(self.metrics['iterations']),
            'total_cost': sum(self.metrics['costs']),
            'error_rate': len(self.metrics['errors']) / max(1, len(self.metrics['iterations'])),
            'avg_iteration_time': mean([i['duration'] for i in self.metrics['iterations']])
        }
```

### カスタムメトリクス

独自のメトリクス収集を追加できます。

```python
# custom_metrics.py
import json
import time
from pathlib import Path

class CustomMetrics:
    def __init__(self, output_dir='.agent/custom_metrics'):
        self.output_dir = Path(output_dir)
        self.output_dir.mkdir(parents=True, exist_ok=True)

    def track_file_changes(self, files_before, files_after):
        """ファイル変更を追跡する"""
        added = set(files_after) - set(files_before)
        removed = set(files_before) - set(files_after)

        metrics = {
            'timestamp': time.time(),
            'files_added': list(added),
            'files_removed': list(removed),
            'total_files': len(files_after)
        }

        self.save_metrics('file_changes', metrics)

    def save_metrics(self, name, data):
        filepath = self.output_dir / f"{name}_{int(time.time())}.json"
        with open(filepath, 'w') as f:
            json.dump(data, f, indent=2)
```

## 監視ツール

### ターミナルダッシュボード

シンプルなターミナルダッシュボードを作成します。

```python
# monitor_dashboard.py
import curses
import json
import time
from pathlib import Path

def monitor_dashboard(stdscr):
    curses.curs_set(0)
    stdscr.nodelay(1)
    stdscr.timeout(1000)

    while True:
        stdscr.clear()

        # 最新の状態を読み取る
        state_files = sorted(Path('.agent/metrics').glob('state_*.json'))
        if state_files:
            with open(state_files[-1]) as f:
                state = json.load(f)

            # ダッシュボードを表示する
            stdscr.addstr(0, 0, "Ralph Orchestrator Monitor", curses.A_BOLD)
            stdscr.addstr(2, 0, f"Iteration: {state['iteration']}")
            stdscr.addstr(3, 0, f"Status: {state['status']}")
            stdscr.addstr(4, 0, f"Cost: ${state['total_cost']:.2f}")
            stdscr.addstr(5, 0, f"Errors: {state['errors_count']}")

            # プログレスバー
            progress = state['iteration'] / 100
            bar_width = 50
            filled = int(bar_width * progress)
            bar = '█' * filled + '░' * (bar_width - filled)
            stdscr.addstr(7, 0, f"[{bar}] {progress*100:.1f}%")

        stdscr.refresh()

        # 終了キーを確認する
        key = stdscr.getch()
        if key == ord('q'):
            break

        time.sleep(1)

if __name__ == "__main__":
    curses.wrapper(monitor_dashboard)
```

### Web ダッシュボード

シンプルな Flask ベースの監視ダッシュボード。

```python
# web_monitor.py
from flask import Flask, jsonify, render_template
import json
from pathlib import Path

app = Flask(__name__)

@app.route('/')
def dashboard():
    return render_template('dashboard.html')

@app.route('/api/status')
def get_status():
    state_files = sorted(Path('.agent/metrics').glob('state_*.json'))
    if state_files:
        with open(state_files[-1]) as f:
            return jsonify(json.load(f))
    return jsonify({'status': 'not_running'})

@app.route('/api/history')
def get_history():
    state_files = sorted(Path('.agent/metrics').glob('state_*.json'))
    history = []
    for f in state_files[-20:]:  # 直近20エントリ
        with open(f) as file:
            history.append(json.load(file))
    return jsonify(history)

if __name__ == '__main__':
    app.run(debug=True, port=5000)
```

## アラートと通知

### コスト上限のアラート

```python
def check_cost_alerts(current_cost, max_cost, thresholds=[0.5, 0.8, 0.95]):
    """コストがしきい値を超えたらアラートする"""
    percentage = current_cost / max_cost

    for threshold in thresholds:
        if percentage >= threshold:
            send_alert(f"Cost alert: {percentage*100:.0f}% of budget used (${current_cost:.2f}/${max_cost:.2f})")
```

### Slack 統合

```python
import requests

def send_slack_alert(webhook_url, message):
    """Slack にアラートを送る"""
    payload = {
        'text': f"🤖 Ralph Alert: {message}",
        'username': 'Ralph Orchestrator'
    }
    requests.post(webhook_url, json=payload)
```

### メール通知

```python
import smtplib
from email.mime.text import MIMEText

def send_email_alert(smtp_config, message):
    """メールアラートを送る"""
    msg = MIMEText(message)
    msg['Subject'] = 'Ralph Orchestrator Alert'
    msg['From'] = smtp_config['from']
    msg['To'] = smtp_config['to']

    with smtplib.SMTP(smtp_config['host'], smtp_config['port']) as server:
        server.starttls()
        server.login(smtp_config['user'], smtp_config['password'])
        server.send_message(msg)
```

## パフォーマンス分析

### イテレーション時間の分析

```python
def analyze_iteration_times(metrics_dir='.agent/metrics'):
    """イテレーションの時間パターンを分析する"""
    state_files = sorted(Path(metrics_dir).glob('state_*.json'))
    times = []

    for f in state_files:
        with open(f) as file:
            data = json.load(file)
            times.append(data.get('elapsed_time', 0))

    if times:
        return {
            'min': min(times),
            'max': max(times),
            'avg': sum(times) / len(times),
            'total': sum(times)
        }
    return {}
```

### コスト分析

```python
def analyze_cost_efficiency(metrics_dir='.agent/metrics'):
    """コスト効率のメトリクスを分析する"""
    state_files = sorted(Path(metrics_dir).glob('state_*.json'))

    data = []
    for f in state_files:
        with open(f) as file:
            state = json.load(file)
            data.append({
                'iteration': state['iteration'],
                'cost': state['total_cost'],
                'cost_per_iteration': state['total_cost'] / max(1, state['iteration'])
            })

    return data
```

## ログ管理

### ログのローテーション

```python
import logging
from logging.handlers import RotatingFileHandler

def setup_logging():
    handler = RotatingFileHandler(
        '.agent/logs/ralph.log',
        maxBytes=10*1024*1024,  # 10MB
        backupCount=5
    )

    formatter = logging.Formatter(
        '%(asctime)s - %(name)s - %(levelname)s - %(message)s'
    )
    handler.setFormatter(formatter)

    logger = logging.getLogger('ralph')
    logger.addHandler(handler)
    logger.setLevel(logging.INFO)

    return logger
```

### ログの集約

複数のログソースを集約します。

```python
def aggregate_logs(log_dir='.agent/logs'):
    """ログエントリを集約して要約する"""
    log_files = Path(log_dir).glob('*.log')
    summary = {
        'errors': [],
        'warnings': [],
        'checkpoints': []
    }

    for log_file in log_files:
        with open(log_file) as f:
            for line in f:
                if 'ERROR' in line:
                    summary['errors'].append(line.strip())
                elif 'WARNING' in line:
                    summary['warnings'].append(line.strip())
                elif 'checkpoint' in line.lower():
                    summary['checkpoints'].append(line.strip())

    return summary
```

## ヘルスチェック

### システムヘルスチェック

```python
def health_check():
    """Ralph の全体的な健全性を確認する"""
    checks = {
        'agent_available': check_agent_availability(),
        'disk_space': check_disk_space(),
        'git_status': check_git_status(),
        'lock_file': check_lock_file(),
        'recent_activity': check_recent_activity()
    }

    all_healthy = all(checks.values())

    return {
        'healthy': all_healthy,
        'checks': checks,
        'timestamp': time.time()
    }

def check_disk_space(min_free_gb=1):
    """十分なディスク容量があるか確認する"""
    import shutil
    stat = shutil.disk_usage('.')
    free_gb = stat.free / (1024**3)
    return free_gb >= min_free_gb

def check_recent_activity(max_idle_minutes=10):
    """エージェントが最近活動しているか確認する"""
    state_files = sorted(Path('.agent/metrics').glob('state_*.json'))
    if not state_files:
        return False

    latest = state_files[-1]
    mtime = latest.stat().st_mtime
    idle_minutes = (time.time() - mtime) / 60

    return idle_minutes <= max_idle_minutes
```

## トラブルシューティング

### よくある監視上の問題

| 問題 | 考えられる原因 | 解決策 |
|-------|-----------------|----------|
| メトリクスが見つからない | `.agent/metrics/` ディレクトリが存在しない | ディレクトリを作成、権限を確認する |
| 古い状態データ | プロセスがクラッシュした | プロセスの状態を確認、必要なら再起動する |
| 欠落したチェックポイント | ディスク容量不足 | ディスク容量を確認、クリーンアップする |
| ログ肥大 | ローテーションが未設定 | ログローテーションを実装する |
| ダッシュボードが更新されない | ファイル監視の問題 | ポーリング間隔、ファイルパスを確認する |

### デバッグモード

包括的な監視のためにデバッグモードを有効にします。

```bash
export RALPH_DEBUG=1
export RALPH_METRICS_VERBOSE=1
python ralph_orchestrator.py --debug
```

これにより次が有効になります。
- 詳細なメトリクスの捕捉
- 追加のログエントリ
- パフォーマンスのプロファイリング
- メモリ使用量の追跡

## ベストプラクティス

1. **早期・頻繁に監視する** - 実行開始時から監視を設定する
2. **上限にアラートを設定する** - コスト・時間・イテレーションのアラートを設定する
3. **トレンドを追跡する** - 時間経過でのパフォーマンスパターンを監視する
4. **定期的にレビューする** - 監視データを定期的に確認する
5. **収集を自動化する** - スクリプトを使って手動監視作業を減らす
6. **アラートを文書化する** - アラート発生時に何をすべきか明確な手順を用意する
7. **アクセスを保護する** - 監視ダッシュボードへのアクセスを制限する

## 関連ドキュメント

- [設定](../guide/configuration.ja.md) - 監視パラメータを構成する
- [トラブルシューティング](../reference/troubleshooting.ja.md) - 一般的な問題の解決
- [パフォーマンス最適化](performance.ja.md) - パフォーマンスの向上
