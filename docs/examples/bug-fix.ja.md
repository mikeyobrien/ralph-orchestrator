# バグ修正の例

## 概要

このガイドは、コードの問題のデバッグと修正に Ralph Orchestrator を使う方法を示します。

## 例 1: ヌルポインタ例外

### 問題のあるコード
```python
def calculate_average(numbers):
    total = sum(numbers)
    return total / len(numbers)

# This crashes with: TypeError: unsupported operand type(s)
result = calculate_average(None)
```

### 修正の依頼
```python
prompt = """
Fix this function that crashes when passed None:

def calculate_average(numbers):
    total = sum(numbers)
    return total / len(numbers)

The function should handle None, empty lists, and non-numeric values gracefully.
"""

response = orchestrator.execute(prompt, agent="claude")
```

### 修正されたコード
```python
def calculate_average(numbers):
    """
    Calculate the average of a list of numbers.
    
    Args:
        numbers: List of numeric values or None
    
    Returns:
        float: Average value, or None if input is invalid
    """
    # Handle None input
    if numbers is None:
        return None
    
    # Handle empty list
    if not numbers:
        return 0
    
    # Filter out non-numeric values
    numeric_values = []
    for num in numbers:
        try:
            numeric_values.append(float(num))
        except (TypeError, ValueError):
            continue
    
    # Return None if no valid numbers found
    if not numeric_values:
        return None
    
    return sum(numeric_values) / len(numeric_values)
```

## 例 2: メモリリークの修正

### 問題のあるコード
```javascript
class EventManager {
    constructor() {
        this.events = {};
    }
    
    on(event, callback) {
        if (!this.events[event]) {
            this.events[event] = [];
        }
        this.events[event].push(callback);
    }
    
    emit(event, data) {
        if (this.events[event]) {
            this.events[event].forEach(cb => cb(data));
        }
    }
}
```

### 修正の依頼
```python
prompt = """
Fix the memory leak in this JavaScript EventManager class. 
The class never removes event listeners, causing memory leaks.
Add an 'off' method and ensure proper cleanup.
"""

response = orchestrator.execute(prompt, agent="claude")
```

### 修正されたコード
```javascript
class EventManager {
    constructor() {
        this.events = {};
        this.eventIds = new WeakMap();
        this.nextId = 0;
    }
    
    on(event, callback) {
        if (!this.events[event]) {
            this.events[event] = [];
        }
        
        // Store callback with unique ID for removal
        const id = this.nextId++;
        const callbackWrapper = { id, callback };
        this.events[event].push(callbackWrapper);
        this.eventIds.set(callback, id);
        
        // Return unsubscribe function
        return () => this.off(event, callback);
    }
    
    off(event, callback) {
        if (!this.events[event]) return;
        
        const id = this.eventIds.get(callback);
        if (id !== undefined) {
            this.events[event] = this.events[event].filter(
                wrapper => wrapper.id !== id
            );
            this.eventIds.delete(callback);
            
            // Clean up empty event arrays
            if (this.events[event].length === 0) {
                delete this.events[event];
            }
        }
    }
    
    emit(event, data) {
        if (this.events[event]) {
            this.events[event].forEach(wrapper => 
                wrapper.callback(data)
            );
        }
    }
    
    // Clear all listeners for an event
    removeAllListeners(event) {
        if (event) {
            delete this.events[event];
        } else {
            this.events = {};
        }
    }
    
    // Cleanup method for destroying the instance
    destroy() {
        this.events = {};
        this.eventIds = new WeakMap();
    }
}
```

## 例 3: 競合状態の修正

### 問題のあるコード
```python
import threading

class Counter:
    def __init__(self):
        self.count = 0
    
    def increment(self):
        temp = self.count
        # Simulating some processing
        temp = temp + 1
        self.count = temp
```

### 修正の依頼
```python
prompt = """
Fix the race condition in this Counter class that occurs when 
multiple threads call increment() simultaneously.
"""

response = orchestrator.execute(prompt, agent="claude")
```

### 修正されたコード
```python
import threading
from contextlib import contextmanager

class Counter:
    def __init__(self):
        self.count = 0
        self._lock = threading.Lock()
    
    def increment(self):
        """Thread-safe increment operation."""
        with self._lock:
            temp = self.count
            # Simulating some processing
            temp = temp + 1
            self.count = temp
    
    def decrement(self):
        """Thread-safe decrement operation."""
        with self._lock:
            self.count -= 1
    
    def get_value(self):
        """Thread-safe read operation."""
        with self._lock:
            return self.count
    
    @contextmanager
    def batch_operation(self):
        """Context manager for batch operations."""
        self._lock.acquire()
        try:
            yield self
        finally:
            self._lock.release()

# Usage example:
counter = Counter()

# Batch operations
with counter.batch_operation():
    for _ in range(100):
        counter.count += 1  # Direct access within context
```

## 例 4: SQL インジェクションの脆弱性

### 問題のあるコード
```python
def get_user(user_id):
    query = f"SELECT * FROM users WHERE id = {user_id}"
    return db.execute(query)
```

### 修正の依頼
```python
prompt = """
Fix the SQL injection vulnerability in this function.
Use parameterized queries and add input validation.
"""

response = orchestrator.execute(prompt, agent="claude")
```

### 修正されたコード
```python
def get_user(user_id):
    """
    Safely retrieve user by ID with SQL injection protection.
    
    Args:
        user_id: User identifier (int or string)
    
    Returns:
        dict: User data or None if not found
    
    Raises:
        ValueError: If user_id is invalid
    """
    # Input validation
    if user_id is None:
        raise ValueError("User ID cannot be None")
    
    try:
        # Convert to integer to ensure type safety
        user_id = int(user_id)
    except (ValueError, TypeError):
        raise ValueError(f"Invalid user ID: {user_id}")
    
    # Additional validation
    if user_id <= 0:
        raise ValueError("User ID must be positive")
    
    # Use parameterized query to prevent SQL injection
    query = "SELECT * FROM users WHERE id = %s"
    
    try:
        result = db.execute(query, (user_id,))
        return result.fetchone()
    except DatabaseError as e:
        logger.error(f"Database error fetching user {user_id}: {e}")
        return None

# Additional secure version using ORM
def get_user_orm(user_id):
    """ORM-based user retrieval (recommended)."""
    try:
        user_id = int(user_id)
        return User.query.filter_by(id=user_id).first()
    except (ValueError, TypeError):
        return None
```

## 例 5: 無限ループの修正

### 問題のあるコード
```python
def find_target(arr, target):
    left = 0
    right = len(arr) - 1
    
    while left < right:
        mid = (left + right) // 2
        if arr[mid] == target:
            return mid
        elif arr[mid] < target:
            left = mid  # Bug: should be mid + 1
        else:
            right = mid
    
    return -1
```

### 修正の依頼
```python
prompt = """
Fix the infinite loop bug in this binary search implementation.
The function gets stuck when the target is not in the array.
"""

response = orchestrator.execute(prompt, agent="claude")
```

### 修正されたコード
```python
def find_target(arr, target):
    """
    Binary search implementation with infinite loop fix.
    
    Args:
        arr: Sorted array to search
        target: Value to find
    
    Returns:
        int: Index of target or -1 if not found
    """
    if not arr:
        return -1
    
    left = 0
    right = len(arr) - 1
    
    while left <= right:  # Changed condition to include equality
        mid = (left + right) // 2
        
        if arr[mid] == target:
            return mid
        elif arr[mid] < target:
            left = mid + 1  # Fixed: increment to avoid infinite loop
        else:
            right = mid - 1  # Fixed: decrement for consistency
    
    return -1

# Enhanced version with additional features
def find_target_enhanced(arr, target, return_insertion_point=False):
    """
    Enhanced binary search with insertion point option.
    
    Args:
        arr: Sorted array to search
        target: Value to find
        return_insertion_point: If True, return where target should be inserted
    
    Returns:
        int: Index of target, or insertion point if not found and requested
    """
    if not arr:
        return 0 if return_insertion_point else -1
    
    left = 0
    right = len(arr) - 1
    
    while left <= right:
        mid = (left + right) // 2
        
        if arr[mid] == target:
            return mid
        elif arr[mid] < target:
            left = mid + 1
        else:
            right = mid - 1
    
    return left if return_insertion_point else -1
```

## よくあるバグのパターンと修正

### 1. オフバイワンエラー
- 配列の境界を確認する
- ループ条件を検証する
- エッジケース（空、単一要素）をテストする

### 2. null/undefined の処理
- 関数の入口で null チェックを追加する
- JavaScript ではオプショナルチェーンを使う
- 妥当な既定値を提供する

### 3. リソースリーク
- 適切なクリーンアップを実装する（ファイル、接続を閉じる）
- Python ではコンテキストマネージャを使う
- クリーンアップのために finally ブロックを追加する

### 4. 並行性の問題
- 共有リソースにロックを使う
- アトミックな操作を実装する
- スレッドセーフなデータ構造の使用を検討する

### 5. 型エラー
- 型チェック/検証を追加する
- TypeScript/型ヒントを使う
- 型変換を明示的に扱う

## デバッグのヒント

1. **まず再現する**: 修正する前に必ずバグを再現する
2. **ロギングを追加する**: フローを理解するために戦略的なロギングを挿入する
3. **ユニットテスト**: バグを露出させるテストを書く
4. **エッジケース**: 空、null、境界値でテストする
5. **コードレビュー**: 修正を他の人にレビューしてもらう

## 関連項目

- [テストの例](./testing.ja.md)
- [エージェントガイド](../guide/agents.ja.md)
- [エラー処理のベストプラクティス](../03-best-practices/best-practices.ja.md)
