# Testing Guide

## Overview

This guide covers testing strategies, tools, and best practices for Hats development and deployment.

## Test Suite Structure

```
tests/
├── unit/                 # Unit tests
│   ├── test_orchestrator.py
│   ├── test_agents.py
│   ├── test_config.py
│   └── test_metrics.py
├── integration/          # Integration tests
│   ├── test_full_cycle.py
│   ├── test_git_operations.py
│   └── test_agent_execution.py
├── e2e/                  # End-to-end tests
│   ├── test_cli.py
│   └── test_scenarios.py
├── fixtures/             # Test fixtures
│   ├── prompts/
│   └── configs/
└── conftest.py          # Pytest configuration
```

## Running Tests

### All Tests
```bash
# Run all tests
pytest

# With coverage
pytest --cov=hats_orchestrator --cov-report=html

# Verbose output
pytest -v

# Specific test file
pytest tests/unit/test_orchestrator.py
```

### Test Categories
```bash
# Unit tests only
pytest tests/unit/

# Integration tests
pytest tests/integration/

# End-to-end tests
pytest tests/e2e/

# Fast tests (exclude slow)
pytest -m "not slow"
```

## Unit Tests

### Testing the Orchestrator

```python
import pytest
from unittest.mock import Mock, patch, MagicMock
from hats_orchestrator import HatsOrchestrator

class TestHatsOrchestrator:
    """Unit tests for HatsOrchestrator"""
    
    @pytest.fixture
    def orchestrator(self):
        """Create orchestrator instance"""
        config = {
            'agent': 'claude',
            'prompt_file': 'test.md',
            'max_iterations': 10,
            'dry_run': True
        }
        return HatsOrchestrator(config)
    
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
    
    @patch('hats_orchestrator.HatsOrchestrator.execute_agent')
    def test_iteration_limit(self, mock_execute, orchestrator):
        """Test max iterations limit"""
        mock_execute.return_value = (True, "Output")
        orchestrator.config['max_iterations'] = 2
        
        result = orchestrator.run()
        
        assert result['iterations'] == 2
        assert result['success'] is False
        assert 'max_iterations' in result['stop_reason']
```

### Testing Agents

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

## Integration Tests

### Full Cycle Test

```python
import pytest
import tempfile
import shutil
from pathlib import Path
from hats_orchestrator import HatsOrchestrator

class TestIntegration:
    """Integration tests for full Hats cycle"""
    
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
        orchestrator = HatsOrchestrator(config)
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
        
        orchestrator = HatsOrchestrator(config)
        orchestrator.create_checkpoint(1)
        
        # Check Git log
        result = subprocess.run(
            ['git', 'log', '--oneline'],
            cwd=test_dir,
            capture_output=True,
            text=True
        )
        
        assert 'Hats checkpoint' in result.stdout
```

## End-to-End Tests

### CLI Testing

```python
import pytest
from click.testing import CliRunner
from hats_cli import cli

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
        assert 'Hats' in result.output
    
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

## Test Fixtures

### Prompt Fixtures

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

### Mock Agent

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

## Performance Testing

```python
import pytest
import time
from hats_orchestrator import HatsOrchestrator

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
        
        orchestrator = HatsOrchestrator(config)
        
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
        orchestrator = HatsOrchestrator(config)
        orchestrator.run()
        
        final_memory = process.memory_info().rss / 1024 / 1024  # MB
        memory_increase = final_memory - initial_memory
        
        # Memory increase should be reasonable
        assert memory_increase < 100  # Less than 100MB
```

## Test Coverage

### Coverage Configuration

```ini
# .coveragerc
[run]
source = hats_orchestrator
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

### Coverage Reports

```bash
# Generate HTML coverage report
pytest --cov=hats_orchestrator --cov-report=html

# View report
open htmlcov/index.html

# Terminal report with missing lines
pytest --cov=hats_orchestrator --cov-report=term-missing
```

## Continuous Integration

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
        pytest --cov=hats_orchestrator --cov-report=xml
    
    - name: Upload coverage
      uses: codecov/codecov-action@v3
      with:
        file: ./coverage.xml
```

## Test Best Practices

### 1. Test Isolation
- Each test should be independent
- Use fixtures for setup/teardown
- Clean up resources after tests

### 2. Mock External Dependencies
- Mock subprocess calls
- Mock file system operations when possible
- Mock network requests

### 3. Test Edge Cases
- Empty inputs
- Invalid configurations
- Network failures
- Timeout scenarios

### 4. Use Descriptive Names
```python
# Good
def test_orchestrator_stops_at_max_iterations():
    pass

# Bad
def test_1():
    pass
```

### 5. Arrange-Act-Assert Pattern
```python
def test_example():
    # Arrange
    orchestrator = HatsOrchestrator(config)
    
    # Act
    result = orchestrator.run()
    
    # Assert
    assert result['success'] is True
```

## Debugging Tests

### Pytest Options
```bash
# Show print statements
pytest -s

# Stop on first failure
pytest -x

# Run specific test
pytest tests/test_file.py::TestClass::test_method

# Run tests matching pattern
pytest -k "test_orchestrator"

# Show local variables on failure
pytest -l
```

### Using pdb
```python
def test_debugging():
    import pdb; pdb.set_trace()
    # Debugger will stop here
    result = some_function()
    assert result == expected
```