# テストの例

## 概要

このガイドは、包括的なテストスイートの生成に Ralph Orchestrator を使う方法を示します。

## 例 1: ユニットテストの生成

### ソースコード
```python
class Calculator:
    def add(self, a, b):
        return a + b
    
    def divide(self, a, b):
        if b == 0:
            raise ValueError("Cannot divide by zero")
        return a / b
    
    def calculate_percentage(self, value, percentage):
        return (value * percentage) / 100
```

### テスト生成の依頼
```python
prompt = """
Generate comprehensive unit tests for this Calculator class using pytest.
Include edge cases, error handling, and parametrized tests.
"""

response = orchestrator.execute(prompt, agent="claude")
```

### 生成されたテスト
```python
import pytest
from calculator import Calculator

class TestCalculator:
    """Comprehensive test suite for Calculator class."""
    
    @pytest.fixture
    def calc(self):
        """Provide a Calculator instance for tests."""
        return Calculator()
    
    # Addition Tests
    @pytest.mark.parametrize("a,b,expected", [
        (2, 3, 5),
        (-1, 1, 0),
        (0, 0, 0),
        (1.5, 2.5, 4.0),
        (-5, -3, -8),
        (1e10, 1e10, 2e10),  # Large numbers
    ])
    def test_add(self, calc, a, b, expected):
        """Test addition with various inputs."""
        assert calc.add(a, b) == expected
    
    # Division Tests
    @pytest.mark.parametrize("a,b,expected", [
        (10, 2, 5),
        (7, 2, 3.5),
        (-10, 2, -5),
        (0, 5, 0),
        (1, 3, 0.3333333333333333),
    ])
    def test_divide(self, calc, a, b, expected):
        """Test division with valid inputs."""
        assert calc.divide(a, b) == pytest.approx(expected)
    
    def test_divide_by_zero(self, calc):
        """Test that dividing by zero raises ValueError."""
        with pytest.raises(ValueError, match="Cannot divide by zero"):
            calc.divide(10, 0)
    
    # Percentage Tests
    @pytest.mark.parametrize("value,percentage,expected", [
        (100, 10, 10),
        (50, 50, 25),
        (200, 25, 50),
        (0, 50, 0),
        (100, 0, 0),
        (100, 150, 150),  # Over 100%
        (100, -10, -10),   # Negative percentage
    ])
    def test_calculate_percentage(self, calc, value, percentage, expected):
        """Test percentage calculation with various inputs."""
        assert calc.calculate_percentage(value, percentage) == expected
    
    # Edge Cases
    def test_add_with_none(self, calc):
        """Test that adding None raises TypeError."""
        with pytest.raises(TypeError):
            calc.add(None, 5)
    
    def test_float_precision(self, calc):
        """Test floating point precision handling."""
        result = calc.add(0.1, 0.2)
        assert result == pytest.approx(0.3)
    
    # Property-based tests (using hypothesis)
    @pytest.mark.skipif(not pytest.importorskip("hypothesis"), 
                        reason="hypothesis not installed")
    def test_add_commutative(self, calc):
        """Test that addition is commutative."""
        from hypothesis import given, strategies as st
        
        @given(st.floats(allow_nan=False), st.floats(allow_nan=False))
        def check_commutative(a, b):
            assert calc.add(a, b) == calc.add(b, a)
        
        check_commutative()
```

## 例 2: 統合テストの生成

### API コード
```python
class UserService:
    def __init__(self, db, email_service):
        self.db = db
        self.email_service = email_service
    
    def create_user(self, email, name):
        if self.db.user_exists(email):
            raise ValueError("User already exists")
        
        user = self.db.create_user(email=email, name=name)
        self.email_service.send_welcome_email(email, name)
        return user
```

### テスト生成の依頼
```python
prompt = """
Generate integration tests for UserService that test the interaction 
between database and email service. Use mocking where appropriate.
"""

response = orchestrator.execute(prompt, agent="claude")
```

### 生成された統合テスト
```python
import pytest
from unittest.mock import Mock, MagicMock, call
from user_service import UserService

class TestUserServiceIntegration:
    """Integration tests for UserService."""
    
    @pytest.fixture
    def mock_db(self):
        """Create a mock database."""
        db = Mock()
        db.user_exists.return_value = False
        db.create_user.return_value = {
            "id": 1,
            "email": "test@example.com",
            "name": "Test User"
        }
        return db
    
    @pytest.fixture
    def mock_email_service(self):
        """Create a mock email service."""
        return Mock()
    
    @pytest.fixture
    def user_service(self, mock_db, mock_email_service):
        """Create UserService with mocked dependencies."""
        return UserService(mock_db, mock_email_service)
    
    def test_create_user_success(self, user_service, mock_db, mock_email_service):
        """Test successful user creation flow."""
        # Arrange
        email = "newuser@example.com"
        name = "New User"
        
        # Act
        user = user_service.create_user(email, name)
        
        # Assert
        mock_db.user_exists.assert_called_once_with(email)
        mock_db.create_user.assert_called_once_with(email=email, name=name)
        mock_email_service.send_welcome_email.assert_called_once_with(email, name)
        assert user["email"] == "test@example.com"
    
    def test_create_user_already_exists(self, user_service, mock_db):
        """Test user creation when user already exists."""
        # Arrange
        mock_db.user_exists.return_value = True
        
        # Act & Assert
        with pytest.raises(ValueError, match="User already exists"):
            user_service.create_user("existing@example.com", "Existing User")
        
        # Verify email was not sent
        mock_db.create_user.assert_not_called()
    
    def test_create_user_email_failure(self, user_service, mock_email_service):
        """Test handling of email service failure."""
        # Arrange
        mock_email_service.send_welcome_email.side_effect = Exception("Email failed")
        
        # Act & Assert
        with pytest.raises(Exception, match="Email failed"):
            user_service.create_user("user@example.com", "User")
    
    def test_create_multiple_users(self, user_service, mock_db, mock_email_service):
        """Test creating multiple users in sequence."""
        users = [
            ("user1@example.com", "User One"),
            ("user2@example.com", "User Two"),
            ("user3@example.com", "User Three"),
        ]
        
        for email, name in users:
            user_service.create_user(email, name)
        
        # Verify all calls were made
        assert mock_db.user_exists.call_count == 3
        assert mock_db.create_user.call_count == 3
        assert mock_email_service.send_welcome_email.call_count == 3
        
        # Verify call order
        email_calls = [call(email, name) for email, name in users]
        mock_email_service.send_welcome_email.assert_has_calls(email_calls)
```

## 例 3: エンドツーエンドテストの生成

### Web アプリケーションのルート
```python
@app.route('/api/login', methods=['POST'])
def login():
    data = request.json
    username = data.get('username')
    password = data.get('password')
    
    if not username or not password:
        return jsonify({"error": "Missing credentials"}), 400
    
    user = authenticate_user(username, password)
    if not user:
        return jsonify({"error": "Invalid credentials"}), 401
    
    token = generate_token(user)
    return jsonify({"token": token}), 200
```

### テスト生成の依頼
```python
prompt = """
Generate end-to-end tests for this login endpoint using pytest and requests.
Test successful login, missing fields, and invalid credentials.
"""

response = orchestrator.execute(prompt, agent="claude")
```

### 生成された E2E テスト
```python
import pytest
import requests
import json
from unittest.mock import patch

class TestLoginEndToEnd:
    """End-to-end tests for login endpoint."""
    
    @pytest.fixture
    def base_url(self):
        """Base URL for the API."""
        return "http://localhost:5000"
    
    @pytest.fixture
    def valid_credentials(self):
        """Valid user credentials."""
        return {
            "username": "testuser",
            "password": "testpass123"
        }
    
    def test_successful_login(self, base_url, valid_credentials):
        """Test successful login returns token."""
        response = requests.post(
            f"{base_url}/api/login",
            json=valid_credentials,
            headers={"Content-Type": "application/json"}
        )
        
        assert response.status_code == 200
        data = response.json()
        assert "token" in data
        assert len(data["token"]) > 0
    
    def test_missing_username(self, base_url):
        """Test login with missing username."""
        response = requests.post(
            f"{base_url}/api/login",
            json={"password": "testpass123"},
            headers={"Content-Type": "application/json"}
        )
        
        assert response.status_code == 400
        data = response.json()
        assert data["error"] == "Missing credentials"
    
    def test_missing_password(self, base_url):
        """Test login with missing password."""
        response = requests.post(
            f"{base_url}/api/login",
            json={"username": "testuser"},
            headers={"Content-Type": "application/json"}
        )
        
        assert response.status_code == 400
        data = response.json()
        assert data["error"] == "Missing credentials"
    
    def test_empty_request_body(self, base_url):
        """Test login with empty request body."""
        response = requests.post(
            f"{base_url}/api/login",
            json={},
            headers={"Content-Type": "application/json"}
        )
        
        assert response.status_code == 400
        data = response.json()
        assert data["error"] == "Missing credentials"
    
    def test_invalid_credentials(self, base_url):
        """Test login with invalid credentials."""
        response = requests.post(
            f"{base_url}/api/login",
            json={
                "username": "wronguser",
                "password": "wrongpass"
            },
            headers={"Content-Type": "application/json"}
        )
        
        assert response.status_code == 401
        data = response.json()
        assert data["error"] == "Invalid credentials"
    
    @pytest.mark.parametrize("content_type", [
        "text/plain",
        "application/xml",
        None
    ])
    def test_invalid_content_type(self, base_url, valid_credentials, content_type):
        """Test login with invalid content types."""
        headers = {}
        if content_type:
            headers["Content-Type"] = content_type
        
        response = requests.post(
            f"{base_url}/api/login",
            data=json.dumps(valid_credentials),
            headers=headers
        )
        
        assert response.status_code in [400, 415]
    
    def test_sql_injection_attempt(self, base_url):
        """Test that SQL injection attempts are handled safely."""
        response = requests.post(
            f"{base_url}/api/login",
            json={
                "username": "admin' OR '1'='1",
                "password": "' OR '1'='1"
            },
            headers={"Content-Type": "application/json"}
        )
        
        assert response.status_code == 401
        data = response.json()
        assert data["error"] == "Invalid credentials"
    
    def test_rate_limiting(self, base_url):
        """Test that rate limiting is enforced."""
        # Make 10 rapid requests
        for _ in range(10):
            requests.post(
                f"{base_url}/api/login",
                json={"username": "test", "password": "wrong"},
                headers={"Content-Type": "application/json"}
            )
        
        # 11th request should be rate limited
        response = requests.post(
            f"{base_url}/api/login",
            json={"username": "test", "password": "wrong"},
            headers={"Content-Type": "application/json"}
        )
        
        assert response.status_code == 429  # Too Many Requests
```

## 例 4: パフォーマンステストの生成

### テスト生成の依頼
```python
prompt = """
Generate performance tests for a function that processes large datasets.
Include tests for execution time, memory usage, and scalability.
"""

response = orchestrator.execute(prompt, agent="claude")
```

### 生成されたパフォーマンステスト
```python
import pytest
import time
import tracemalloc
from memory_profiler import profile
import numpy as np

class TestPerformance:
    """Performance test suite for data processing functions."""
    
    @pytest.fixture
    def large_dataset(self):
        """Generate a large dataset for testing."""
        return np.random.rand(1000000)
    
    def test_execution_time(self, large_dataset):
        """Test that processing completes within time limit."""
        start_time = time.perf_counter()
        
        # Process the data
        result = process_data(large_dataset)
        
        end_time = time.perf_counter()
        execution_time = end_time - start_time
        
        # Assert execution time is under 1 second
        assert execution_time < 1.0, f"Execution took {execution_time:.2f}s"
    
    def test_memory_usage(self, large_dataset):
        """Test that memory usage stays within limits."""
        tracemalloc.start()
        
        # Process the data
        result = process_data(large_dataset)
        
        current, peak = tracemalloc.get_traced_memory()
        tracemalloc.stop()
        
        # Convert to MB
        peak_mb = peak / 1024 / 1024
        
        # Assert memory usage is under 100MB
        assert peak_mb < 100, f"Peak memory usage: {peak_mb:.2f}MB"
    
    @pytest.mark.parametrize("size", [100, 1000, 10000, 100000])
    def test_scalability(self, size):
        """Test that performance scales linearly with data size."""
        data = np.random.rand(size)
        
        start_time = time.perf_counter()
        result = process_data(data)
        execution_time = time.perf_counter() - start_time
        
        # Calculate time per element
        time_per_element = execution_time / size
        
        # Assert time per element is roughly constant (with 20% tolerance)
        expected_time_per_element = 1e-6  # 1 microsecond
        assert time_per_element < expected_time_per_element * 1.2
    
    def test_concurrent_processing(self):
        """Test performance under concurrent load."""
        import concurrent.futures
        
        def process_batch():
            data = np.random.rand(10000)
            return process_data(data)
        
        start_time = time.perf_counter()
        
        with concurrent.futures.ThreadPoolExecutor(max_workers=4) as executor:
            futures = [executor.submit(process_batch) for _ in range(10)]
            results = [f.result() for f in futures]
        
        execution_time = time.perf_counter() - start_time
        
        # Should complete 10 batches in under 2 seconds with parallelism
        assert execution_time < 2.0
    
    @pytest.mark.benchmark
    def test_benchmark(self, benchmark):
        """Benchmark the function using pytest-benchmark."""
        data = np.random.rand(10000)
        result = benchmark(process_data, data)
        
        # Assertions on benchmark stats
        assert benchmark.stats["mean"] < 0.01  # Mean time under 10ms
        assert benchmark.stats["stddev"] < 0.002  # Low variance
```

## テスト生成のベストプラクティス

### 1. カバレッジの目標
- コードカバレッジ 80% 以上を目指す
- すべての公開メソッドをテストする
- エッジケースとエラー経路を含める

### 2. テストの構成
- 関連するテストをクラスにまとめる
- 説明的なテスト名を使う
- AAA パターンに従う（Arrange, Act, Assert）

### 3. フィクスチャとモック
- 共通のセットアップにフィクスチャを使う
- 外部依存をモックする
- テストを分離し独立に保つ

### 4. パラメータ化されたテスト
- 似たテストケースには parametrize を使う
- 境界値をテストする
- 否定的なテストケースを含める

### 5. パフォーマンステスト
- 現実的なパフォーマンス目標を設定する
- 代表的なデータサイズでテストする
- リソース使用を監視する

## 関連項目

- [テストのベストプラクティス](../contributing/testing.ja.md)
- [CI/CD 統合](../deployment/ci-cd.ja.md)
