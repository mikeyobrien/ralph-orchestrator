# テストガイド

## 概要

このガイドは、Ralph Orchestrator の開発とデプロイのためのテスト戦略、ツール、ベスト
プラクティスを扱います。

## テストスイートの構成

```
tests/
├── unit/                 # ユニットテスト
│   ├── test_orchestrator.py
│   ├── test_agents.py
│   ├── test_config.py
│   └── test_metrics.py
├── integration/          # 統合テスト
│   ├── test_full_cycle.py
│   ├── test_git_operations.py
│   └── test_agent_execution.py
├── e2e/                  # エンドツーエンドのテスト
│   ├── test_cli.py
│   └── test_scenarios.py
├── fixtures/             # テストフィクスチャ
│   ├── prompts/
│   └── configs/
└── conftest.py          # Pytest の設定
```

## テストの実行

### すべてのテスト
```bash
# すべてのテストを実行する
pytest

# カバレッジ付き
pytest --cov=ralph_orchestrator --cov-report=html

# 詳細な出力
pytest -v

# 特定のテストファイル
pytest tests/unit/test_orchestrator.py
```

### テストのカテゴリ
```bash
# ユニットテストのみ
pytest tests/unit/

# 統合テスト
pytest tests/integration/

# エンドツーエンドのテスト
pytest tests/e2e/

# 速いテスト（遅いものを除外）
pytest -m "not slow"
```

## ユニットテスト

### オーケストレーターのテスト

```python
import pytest
from unittest.mock import Mock, patch, MagicMock
from ralph_orchestrator import RalphOrchestrator

class TestRalphOrchestrator:
    """Unit tests for RalphOrchestrator"""
    
    @pytest.fixture
    def orchestrator(self):
        """Create orchestrator instance"""
        config = {
            'agent': 'claude',
            'prompt_file': 'test.md',
            'max_iterations': 10,
            'dry_run': True
        }
        return RalphOrchestrator(config)
    
    def test_initialization(self, orchestrator):
        """Test orchestrator initialization"""
        assert orchestrator.config['agent'] == 'claude'
        assert orchestrator.config['max_iterations'] == 10
        assert orchestrator.iteration_count == 0
    
    @patch('subprocess.run')
    def test_execute_agent(self, mock_run, orchestrator):
        """Test agent execution"""
        mock_run.return_value = MagicMock(
            returncode=0,
            stdout='Task completed',
            stderr=''
        )
        
        success, output = orchestrator.execute_agent('claude', 'test.md')
        
        assert success is True
        assert output == 'Task completed'
        mock_run.assert_called_once()
    
    def test_check_task_complete(self, orchestrator, tmp_path):
        """Test task completion check"""
        # Create prompt file without marker
        prompt_file = tmp_path / "prompt.md"
        prompt_file.write_text("Do something")
        assert orchestrator.check_task_complete(str(prompt_file)) is False
        
        # Add completion marker
        prompt_file.write_text("Do something\n<!-- Legacy marker removed -->")
        assert orchestrator.check_task_complete(str(prompt_file)) is True
    
    @patch('ralph_orchestrator.RalphOrchestrator.execute_agent')
    def test_iteration_limit(self, mock_execute, orchestrator):
        """Test max iterations limit"""
        mock_execute.return_value = (True, "Output")
        orchestrator.config['max_iterations'] = 2
        
        result = orchestrator.run()
        
        assert result['iterations'] == 2
        assert result['success'] is False
        assert 'max_iterations' in result['stop_reason']
```

### エージェントのテスト

```python
import pytest
from unittest.mock import patch, MagicMock
from agents import ClaudeAgent, GeminiAgent, AgentManager

class TestAgents:
    """Unit tests for AI agents"""
    
    @patch('shutil.which')
    def test_claude_availability(self, mock_which):
        """Test Claude agent availability check"""
        mock_which.return_value = '/usr/bin/claude'
        agent = ClaudeAgent()
        assert agent.available is True
        
        mock_which.return_value = None
        agent = ClaudeAgent()
        assert agent.available is False
    
    @patch('subprocess.run')
    def test_agent_execution_timeout(self, mock_run):
        """Test agent timeout handling"""
        mock_run.side_effect = subprocess.TimeoutExpired('cmd', 300)
        
        agent = ClaudeAgent()
        success, output = agent.execute('prompt.md')
        
        assert success is False
        assert 'timeout' in output.lower()
    
    def test_agent_manager_auto_select(self):
        """Test automatic agent selection"""
        manager = AgentManager()
        
        with patch.object(manager.agents['claude'], 'available', True):
            with patch.object(manager.agents['gemini'], 'available', False):
                agent = manager.get_agent('auto')
                assert agent.name == 'claude'
```

## 統合テスト

### フルサイクルのテスト

```python
import pytest
import tempfile
import shutil
from pathlib import Path
from ralph_orchestrator import RalphOrchestrator

class TestIntegration:
    """Integration tests for full Ralph cycle"""
    
    @pytest.fixture
    def test_dir(self):
        """Create temporary test directory"""
        temp_dir = tempfile.mkdtemp()
        yield Path(temp_dir)
        shutil.rmtree(temp_dir)
    
    def test_full_execution_cycle(self, test_dir):
        """Test complete execution cycle"""
        # Setup
        prompt_file = test_dir / "PROMPT.md"
        prompt_file.write_text("""
        Create a Python function that returns 'Hello'
        <!-- Legacy marker removed -->
        """)
        
        config = {
            'agent': 'auto',
            'prompt_file': str(prompt_file),
            'max_iterations': 5,
            'working_directory': str(test_dir),
            'dry_run': False
        }
        
        # Execute
        orchestrator = RalphOrchestrator(config)
        result = orchestrator.run()
        
        # Verify
        assert result['success'] is True
        assert result['iterations'] > 0
        assert (test_dir / '.agent').exists()
    
    @pytest.mark.slow
    def test_checkpoint_creation(self, test_dir):
        """Test Git checkpoint creation"""
        # Initialize Git repo
        subprocess.run(['git', 'init'], cwd=test_dir)
        
        prompt_file = test_dir / "PROMPT.md"
        prompt_file.write_text("Test task")
        
        config = {
            'prompt_file': str(prompt_file),
            'checkpoint_interval': 1,
            'working_directory': str(test_dir)
        }
        
        orchestrator = RalphOrchestrator(config)
        orchestrator.create_checkpoint(1)
        
        # Check Git log
        result = subprocess.run(
            ['git', 'log', '--oneline'],
            cwd=test_dir,
            capture_output=True,
            text=True
        )
        
        assert 'Ralph checkpoint' in result.stdout
```

## エンドツーエンドのテスト

### CLI のテスト

```python
import pytest
from click.testing import CliRunner
from ralph_cli import cli

class TestCLI:
    """End-to-end CLI tests"""
    
    @pytest.fixture
    def runner(self):
        """Create CLI test runner"""
        return CliRunner()
    
    def test_cli_help(self, runner):
        """Test help command"""
        result = runner.invoke(cli, ['--help'])
        assert result.exit_code == 0
        assert 'Ralph Orchestrator' in result.output
    
    def test_cli_init(self, runner):
        """Test init command"""
        with runner.isolated_filesystem():
            result = runner.invoke(cli, ['init'])
            assert result.exit_code == 0
            assert Path('PROMPT.md').exists()
            assert Path('.agent').exists()
    
    def test_cli_status(self, runner):
        """Test status command"""
        with runner.isolated_filesystem():
            # Create prompt file
            Path('PROMPT.md').write_text('Test')
            
            result = runner.invoke(cli, ['status'])
            assert result.exit_code == 0
            assert 'PROMPT.md exists' in result.output
    
    def test_cli_run_dry(self, runner):
        """Test dry run"""
        with runner.isolated_filesystem():
            Path('PROMPT.md').write_text('Test task')
            
            result = runner.invoke(cli, ['run', '--dry-run'])
            assert result.exit_code == 0
```

## テストフィクスチャ

### プロンプトのフィクスチャ

```python
# tests/fixtures/prompts.py

SIMPLE_TASK = """
Create a Python function that adds two numbers.
"""

COMPLEX_TASK = """
Build a REST API with:
- User authentication
- CRUD operations
- Database integration
- Unit tests
"""

COMPLETED_TASK = """
Create a hello world function.
<!-- Legacy marker removed -->
"""
```

### モックエージェント

```python
# tests/fixtures/mock_agent.py

class MockAgent:
    """Mock agent for testing"""
    
    def __init__(self, responses=None):
        self.responses = responses or ['Default response']
        self.call_count = 0
    
    def execute(self, prompt_file):
        """Return predetermined responses"""
        if self.call_count < len(self.responses):
            response = self.responses[self.call_count]
        else:
            response = self.responses[-1]
        
        self.call_count += 1
        return True, response
```

## パフォーマンステスト

```python
import pytest
import time
from ralph_orchestrator import RalphOrchestrator

@pytest.mark.performance
class TestPerformance:
    """Performance and load tests"""
    
    def test_iteration_performance(self):
        """Test iteration execution time"""
        config = {
            'agent': 'auto',
            'max_iterations': 10,
            'dry_run': True
        }
        
        orchestrator = RalphOrchestrator(config)
        
        start_time = time.time()
        orchestrator.run()
        execution_time = time.time() - start_time
        
        # Should complete dry run quickly
        assert execution_time < 5.0
    
    def test_memory_usage(self):
        """Test memory consumption"""
        import psutil
        import os
        
        process = psutil.Process(os.getpid())
        initial_memory = process.memory_info().rss / 1024 / 1024  # MB
        
        # Run multiple iterations
        config = {'max_iterations': 100, 'dry_run': True}
        orchestrator = RalphOrchestrator(config)
        orchestrator.run()
        
        final_memory = process.memory_info().rss / 1024 / 1024  # MB
        memory_increase = final_memory - initial_memory
        
        # Memory increase should be reasonable
        assert memory_increase < 100  # Less than 100MB
```

## テストカバレッジ

### カバレッジの設定

```ini
# .coveragerc
[run]
source = ralph_orchestrator
omit = 
    */tests/*
    */test_*.py
    */__pycache__/*

[report]
exclude_lines =
    pragma: no cover
    def __repr__
    raise AssertionError
    raise NotImplementedError
    if __name__ == .__main__.:
    if TYPE_CHECKING:
```

### カバレッジレポート

```bash
# HTML カバレッジレポートを生成する
pytest --cov=ralph_orchestrator --cov-report=html

# レポートを表示する
open htmlcov/index.html

# 欠けている行付きのターミナルレポート
pytest --cov=ralph_orchestrator --cov-report=term-missing
```

## 継続的インテグレーション

### GitHub Actions

```yaml
# .github/workflows/test.yml
name: Tests

on: [push, pull_request]

jobs:
  test:
    runs-on: ubuntu-latest
    strategy:
      matrix:
        python-version: [3.8, 3.9, 3.10, 3.11]
    
    steps:
    - uses: actions/checkout@v3
    
    - name: Set up Python
      uses: actions/setup-python@v4
      with:
        python-version: ${{ matrix.python-version }}
    
    - name: Install dependencies
      run: |
        pip install -e .
        pip install pytest pytest-cov
    
    - name: Run tests
      run: |
        pytest --cov=ralph_orchestrator --cov-report=xml
    
    - name: Upload coverage
      uses: codecov/codecov-action@v3
      with:
        file: ./coverage.xml
```

## テストのベストプラクティス

### 1. テストの分離
- 各テストは独立しているべき
- セットアップ/後始末にフィクスチャを使う
- テスト後にリソースをクリーンアップする

### 2. 外部依存のモック
- サブプロセス呼び出しをモックする
- 可能ならファイルシステム操作をモックする
- ネットワークリクエストをモックする

### 3. エッジケースのテスト
- 空の入力
- 無効な設定
- ネットワーク障害
- タイムアウトのシナリオ

### 4. 説明的な名前を使う
```python
# Good
def test_orchestrator_stops_at_max_iterations():
    pass

# Bad
def test_1():
    pass
```

### 5. Arrange-Act-Assert パターン
```python
def test_example():
    # Arrange
    orchestrator = RalphOrchestrator(config)
    
    # Act
    result = orchestrator.run()
    
    # Assert
    assert result['success'] is True
```

## テストのデバッグ

### Pytest のオプション
```bash
# print 文を表示する
pytest -s

# 最初の失敗で停止する
pytest -x

# 特定のテストを実行する
pytest tests/test_file.py::TestClass::test_method

# パターンに一致するテストを実行する
pytest -k "test_orchestrator"

# 失敗時にローカル変数を表示する
pytest -l
```

### pdb を使う
```python
def test_debugging():
    import pdb; pdb.set_trace()
    # Debugger will stop here
    result = some_function()
    assert result == expected
```
