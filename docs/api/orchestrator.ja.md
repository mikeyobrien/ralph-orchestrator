# Orchestrator API リファレンス

Ralph Orchestrator コアモジュールの完全な API ドキュメントです。

## モジュール: `ralph_orchestrator`

AI エージェントの実行を調整するメインのオーケストレーションモジュールです。

## クラス

### `RalphOrchestrator`

実行ループを管理するメインのオーケストレータクラスです。

```python
class RalphOrchestrator:
    def __init__(
        self,
        prompt_file_or_config = None,
        primary_tool: str = "claude",
        max_iterations: int = 100,
        max_runtime: int = 14400,
        track_costs: bool = False,
        max_cost: float = 10.0,
        checkpoint_interval: int = 5,
        archive_dir: str = "./prompts/archive",
        verbose: bool = False
    ):
        """設定または個別のパラメータでオーケストレータを初期化する。"""
```

#### メソッド

##### `run()`

```python
def run(self) -> None:
    """完了または上限に達するまでオーケストレーションループを実行する。"""
```

##### `arun()`

```python
async def arun(self) -> None:
    """オーケストレーションループを非同期に実行する。"""
```

### `RalphConfig`

オーケストレータ用の設定データクラスです。

```python
@dataclass
class RalphConfig:
    agent: AgentType = AgentType.AUTO
    prompt_file: str = "PROMPT.md"
    max_iterations: int = 100
    max_runtime: int = 14400
    checkpoint_interval: int = 5
    retry_delay: int = 2
    archive_prompts: bool = True
    git_checkpoint: bool = True
    verbose: bool = False
    dry_run: bool = False
    max_tokens: int = 1000000
    max_cost: float = 50.0
    context_window: int = 200000
    context_threshold: float = 0.8
    metrics_interval: int = 10
    enable_metrics: bool = True
    max_prompt_size: int = 10485760
    allow_unsafe_paths: bool = False
    agent_args: List[str] = field(default_factory=list)
    adapters: Dict[str, AdapterConfig] = field(default_factory=dict)
```

### `AgentType`

```python
class AgentType(Enum):
    CLAUDE = "claude"
    Q = "q"
    GEMINI = "gemini"
    AUTO = "auto"
```

## 関数

### `main()`

CLI 実行のエントリポイントです。

```python
def main() -> int:
    """CLI 実行のメインエントリポイント。"""
```

## 使用例

```python
from ralph_orchestrator import RalphOrchestrator, RalphConfig

# 設定オブジェクトを使う
config = RalphConfig(agent=AgentType.CLAUDE)
orchestrator = RalphOrchestrator(config)
orchestrator.run()

# 個別のパラメータを使う
orchestrator = RalphOrchestrator(
    prompt_file_or_config="PROMPT.md",
    primary_tool="claude",
    max_iterations=50
)
orchestrator.run()
```

Ralph Wiggum テクニックを実装するメインのオーケストレーションモジュールです。

### クラス

#### `RalphOrchestrator`

イテレーションループを管理するメインのオーケストレータクラスです。

```python
class RalphOrchestrator:
    """
    自律的なタスク完了のために AI エージェントのイテレーションをオーケストレートする。

    Attributes:
        config (RalphConfig): 設定オブジェクト
        agent (Agent): アクティブな AI エージェントのインスタンス
        metrics (MetricsCollector): メトリクスの追跡
        state (OrchestratorState): 現在の状態
    """
```

##### コンストラクタ

```python
def __init__(self, config: RalphConfig) -> None:
    """
    設定でオーケストレータを初期化する。

    Args:
        config: 設定を持つ RalphConfig オブジェクト

    Raises:
        ValueError: 設定が無効な場合
        RuntimeError: 利用可能なエージェントがない場合
    """
```

##### メソッド

###### `run()`

```python
def run(self) -> int:
    """
    メインのオーケストレーションループを実行する。

    Returns:
        int: 終了コード（成功なら0、失敗なら非ゼロ）

    Raises:
        SecurityError: セキュリティ検証が失敗した場合
        RuntimeError: 回復不能なエラーが発生した場合
    """
```

###### `iterate()`

```python
def iterate(self) -> bool:
    """
    1回のイテレーションを実行する。

    Returns:
        bool: タスクが完了していれば True、それ以外は False

    Raises:
        AgentError: エージェントの実行が失敗した場合
        TokenLimitError: トークン上限を超えた場合
        CostLimitError: コスト上限を超えた場合
    """
```

###### `checkpoint()`

```python
def checkpoint(self) -> None:
    """
    現在の状態の Git チェックポイントを作成する。

    Raises:
        GitError: Git 操作が失敗した場合
    """
```

###### `save_state()`

```python
def save_state(self) -> None:
    """
    現在の状態をディスクに永続化する。

    状態には次が含まれる:
    - 現在のイテレーション番号
    - トークン使用量
    - 累積コスト
    - タイムスタンプ
    - エージェント情報
    """
```

###### `load_state()`

```python
def load_state(self) -> Optional[OrchestratorState]:
    """
    ディスクから以前の状態を読み込む。

    Returns:
        OrchestratorState、状態が存在しなければ None
    """
```

#### `RalphConfig`

オーケストレータ用の設定データクラスです。

```python
@dataclass
class RalphConfig:
    """
    Ralph オーケストレータの設定。

    すべてのパラメータは次のいずれかで設定できる:
    - コマンドライン引数
    - 環境変数（RALPH_*）
    - 設定ファイル（.ralph.conf）
    - 既定値
    """

    # エージェント設定
    agent: AgentType = AgentType.AUTO
    agent_args: List[str] = field(default_factory=list)

    # ファイルパス
    prompt_file: str = "PROMPT.md"

    # イテレーション上限
    max_iterations: int = 100
    max_runtime: int = 14400  # 4時間

    # トークンとコストの上限
    max_tokens: int = 1000000  # 100万トークン
    max_cost: float = 50.0  # 50 USD

    # コンテキスト管理
    context_window: int = 200000  # 20万トークン
    context_threshold: float = 0.8  # 80% でトリガー

    # チェックポイント
    checkpoint_interval: int = 5
    git_checkpoint: bool = True
    archive_prompts: bool = True

    # リトライ設定
    retry_delay: int = 2
    max_retries: int = 3

    # 監視
    metrics_interval: int = 10
    enable_metrics: bool = True

    # セキュリティ
    max_prompt_size: int = 10485760  # 10MB
    allow_unsafe_paths: bool = False

    # 出力
    verbose: bool = False
    dry_run: bool = False
```

#### `OrchestratorState`

オーケストレータの状態追跡です。

```python
@dataclass
class OrchestratorState:
    """
    永続化と復旧のためのオーケストレータ状態。
    """

    # イテレーション追跡
    current_iteration: int = 0
    total_iterations: int = 0

    # 時間追跡
    start_time: datetime = field(default_factory=datetime.now)
    last_iteration_time: Optional[datetime] = None
    total_runtime: float = 0.0

    # トークン追跡
    total_input_tokens: int = 0
    total_output_tokens: int = 0

    # コスト追跡
    total_cost: float = 0.0

    # エージェント情報
    agent_type: str = ""
    agent_version: Optional[str] = None

    # 完了状態
    is_complete: bool = False
    completion_reason: Optional[str] = None
```

### 関数

#### `detect_agents()`

```python
def detect_agents() -> List[AgentType]:
    """
    システム上で利用可能な AI エージェントを検出する。

    Returns:
        利用可能な AgentType 列挙値のリスト

    Example:
        >>> detect_agents()
        [AgentType.CLAUDE, AgentType.GEMINI]
    """
```

#### `validate_prompt_file()`

```python
def validate_prompt_file(
    file_path: str, 
    max_size: int = DEFAULT_MAX_PROMPT_SIZE
) -> None:
    """
    プロンプトファイルの安全性とサイズを検証する。

    Args:
        file_path: プロンプトファイルへのパス
        max_size: 許可される最大ファイルサイズ（バイト）

    Raises:
        FileNotFoundError: ファイルが存在しない場合
        SecurityError: ファイルに危険なパターンが含まれる場合
        ValueError: ファイルがサイズ上限を超える場合
    """
```

#### `sanitize_input()`

```python
def sanitize_input(text: str) -> str:
    """
    安全性のため入力テキストをサニタイズする。

    Args:
        text: サニタイズ対象の入力テキスト

    Returns:
        処理に対して安全なサニタイズ済みテキスト

    Example:
        >>> sanitize_input("rm -rf /; echo 'done'")
        "rm -rf _; echo 'done'"
    """
```

#### `calculate_cost()`

```python
def calculate_cost(
    input_tokens: int,
    output_tokens: int,
    agent_type: AgentType
) -> float:
    """
    トークン使用量に基づいてコストを計算する。

    Args:
        input_tokens: 入力トークン数
        output_tokens: 出力トークン数
        agent_type: 使用するエージェントの種類

    Returns:
        USD でのコスト

    Example:
        >>> calculate_cost(1000, 500, AgentType.CLAUDE)
        0.0105  # $0.0105
    """
```

### 例外

#### `OrchestratorError`

オーケストレータのエラーの基底例外です。

```python
class OrchestratorError(Exception):
    """オーケストレータのエラーの基底例外。"""
    pass
```

#### `SecurityError`

```python
class SecurityError(OrchestratorError):
    """セキュリティ検証が失敗したときに送出される。"""
    pass
```

#### `TokenLimitError`

```python
class TokenLimitError(OrchestratorError):
    """トークン上限を超えたときに送出される。"""
    pass
```

#### `CostLimitError`

```python
class CostLimitError(OrchestratorError):
    """コスト上限を超えたときに送出される。"""
    pass
```

#### `AgentError`

```python
class AgentError(OrchestratorError):
    """エージェントの実行が失敗したときに送出される。"""
    pass
```

### 定数

```python
# バージョン
VERSION = "1.0.0"

# 既定値
DEFAULT_MAX_ITERATIONS = 100
DEFAULT_MAX_RUNTIME = 14400  # 4時間
DEFAULT_PROMPT_FILE = "PROMPT.md"
DEFAULT_CHECKPOINT_INTERVAL = 5
DEFAULT_RETRY_DELAY = 2
DEFAULT_MAX_TOKENS = 1000000  # 100万トークン
DEFAULT_MAX_COST = 50.0  # 50 USD
DEFAULT_CONTEXT_WINDOW = 200000  # 20万トークン
DEFAULT_CONTEXT_THRESHOLD = 0.8  # 80%
DEFAULT_METRICS_INTERVAL = 10
DEFAULT_MAX_PROMPT_SIZE = 10485760  # 10MB

# 100万トークンあたりのトークンコスト
TOKEN_COSTS = {
    "claude": {"input": 3.0, "output": 15.0},
    "q": {"input": 0.5, "output": 1.5},
    "gemini": {"input": 0.5, "output": 1.5}
}

# レガシーの完了マーカー（非推奨 — オーケストレータは現在イテレーション/コスト/時間の上限を使う）
# COMPLETION_MARKERS = ["TASK_COMPLETE", "TASK_DONE", "COMPLETE"]

# セキュリティパターン
DANGEROUS_PATTERNS = [
    r"rm\s+-rf\s+/",
    r":(){ :|:& };:",
    r"dd\s+if=/dev/zero",
    r"mkfs\.",
    r"format\s+[cC]:",
]
```

## 使用例

### 基本的な使い方

```python
from ralph_orchestrator import RalphOrchestrator, RalphConfig

# 設定を作成する
config = RalphConfig(
    agent=AgentType.CLAUDE,
    prompt_file="task.md",
    max_iterations=50,
    max_cost=25.0
)

# オーケストレータを初期化する
orchestrator = RalphOrchestrator(config)

# オーケストレーションを実行する
exit_code = orchestrator.run()
```

### カスタム設定

```python
# 環境から読み込み、上書きを加える
config = RalphConfig()
config.max_iterations = 100
config.checkpoint_interval = 10
config.verbose = True

# カスタム設定で初期化する
orchestrator = RalphOrchestrator(config)
```

### 状態管理

```python
# 手動で状態を保存する
orchestrator.save_state()

# 以前の状態を読み込む
state = orchestrator.load_state()
if state:
    print(f"Resuming from iteration {state.current_iteration}")
```

### エラー処理

```python
try:
    orchestrator = RalphOrchestrator(config)
    exit_code = orchestrator.run()
except SecurityError as e:
    print(f"Security violation: {e}")
except TokenLimitError as e:
    print(f"Token limit exceeded: {e}")
except CostLimitError as e:
    print(f"Cost limit exceeded: {e}")
except Exception as e:
    print(f"Unexpected error: {e}")
```

## スレッドセーフティ

このオーケストレータは**スレッドセーフではありません**。並行実行が必要な場合は次を行います。

1. 別々のオーケストレータインスタンスを作成する
2. 異なる作業ディレクトリを使う
3. 外部の同期機構を実装する

## パフォーマンスの考慮事項

- **メモリ使用量**: ベース約50MB + エージェントのオーバーヘッド
- **ディスク I/O**: チェックポイントは Git コミットを作成する
- **ネットワーク**: エージェントの API 呼び出しにはレイテンシが生じ得る
- **CPU**: イテレーション間のオーバーヘッドは最小限（1%未満）

## 関連ドキュメント

- [設定 API](config.ja.md)
- [エージェント API](agents.ja.md)
- [メトリクス API](metrics.ja.md)
- [CLI リファレンス](cli.ja.md)

---

📚 続きは[設定 API](config.ja.md)へ →
