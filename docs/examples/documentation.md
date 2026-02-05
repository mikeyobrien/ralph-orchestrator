# Documentation Writing Examples

## Overview

This guide provides examples of using Hats for documentation tasks.

## Example 1: API Documentation Generation

### Prompt
```python
prompt = """
Generate comprehensive API documentation for the following Python class:

class DataProcessor:
    def process_batch(self, data: List[dict], batch_size: int = 100) -> List[dict]:
        '''Process data in batches'''
        pass
    
    def validate_data(self, data: dict) -> bool:
        '''Validate data structure'''
        pass
"""

response = orchestrator.execute(prompt, agent="claude")
```

### Generated Documentation
```markdown
## DataProcessor API Reference

### Class: DataProcessor

A utility class for processing and validating data in batch operations.

#### Methods

##### `process_batch(data: List[dict], batch_size: int = 100) -> List[dict]`

Process a list of dictionaries in configurable batch sizes.

**Parameters:**
- `data` (List[dict]): The input data to process
- `batch_size` (int, optional): Number of items to process per batch. Defaults to 100.

**Returns:**
- List[dict]: Processed data maintaining the same structure

**Example:**
```python
processor = DataProcessor()
result = processor.process_batch([{"id": 1}, {"id": 2}], batch_size=50)
```

##### `validate_data(data: dict) -> bool`

Validate the structure and content of a data dictionary.

**Parameters:**
- `data` (dict): The data dictionary to validate

**Returns:**
- bool: True if data is valid, False otherwise
```

## Example 2: README Generation

### Prompt
```python
prompt = """
Create a README.md for a Python package called 'quicksort-plus' that implements 
an optimized quicksort algorithm with the following features:
- Hybrid approach with insertion sort for small arrays
- Three-way partitioning for duplicate elements
- Parallel processing support
"""

response = orchestrator.execute(prompt, agent="claude")
```

### Generated README
```markdown
# QuickSort Plus

An optimized implementation of the quicksort algorithm with modern enhancements.

## Features

- **Hybrid Sorting**: Automatically switches to insertion sort for small arrays (< 10 elements)
- **Three-Way Partitioning**: Efficiently handles arrays with many duplicate elements
- **Parallel Processing**: Leverages multiple cores for large datasets
- **Type Support**: Works with any comparable Python objects

## Installation

```bash
pip install quicksort-plus
```

## Quick Start

```python
from quicksort_plus import quicksort

# Basic usage
data = [3, 1, 4, 1, 5, 9, 2, 6]
sorted_data = quicksort(data)

# With parallel processing
large_data = list(range(1000000, 0, -1))
sorted_data = quicksort(large_data, parallel=True)
```

## Performance

| Array Size | Standard QuickSort | QuickSort Plus | Improvement |
|------------|-------------------|----------------|-------------|
| 100        | 0.012ms          | 0.008ms        | 33%         |
| 10,000     | 1.8ms            | 1.2ms          | 33%         |
| 1,000,000  | 248ms            | 142ms          | 43%         |

## License

MIT License - See LICENSE file for details
```

## Example 3: Code Comments Generation

### Prompt
```python
prompt = """
Add comprehensive docstrings and inline comments to this function:

def merge_sorted_arrays(arr1, arr2):
    result = []
    i = j = 0
    
    while i < len(arr1) and j < len(arr2):
        if arr1[i] <= arr2[j]:
            result.append(arr1[i])
            i += 1
        else:
            result.append(arr2[j])
            j += 1
    
    result.extend(arr1[i:])
    result.extend(arr2[j:])
    return result
"""

response = orchestrator.execute(prompt, agent="claude")
```

### Result with Documentation
```python
def merge_sorted_arrays(arr1: list, arr2: list) -> list:
    """
    Merge two sorted arrays into a single sorted array.
    
    This function implements the merge step of merge sort, combining
    two pre-sorted arrays into one sorted array in O(n+m) time.
    
    Args:
        arr1 (list): First sorted array
        arr2 (list): Second sorted array
    
    Returns:
        list: A new sorted array containing all elements from both inputs
    
    Examples:
        >>> merge_sorted_arrays([1, 3, 5], [2, 4, 6])
        [1, 2, 3, 4, 5, 6]
        
        >>> merge_sorted_arrays([1, 2], [3, 4, 5])
        [1, 2, 3, 4, 5]
    
    Time Complexity: O(n + m) where n and m are the lengths of the arrays
    Space Complexity: O(n + m) for the result array
    """
    result = []
    i = j = 0  # Initialize pointers for both arrays
    
    # Compare elements from both arrays and add the smaller one
    while i < len(arr1) and j < len(arr2):
        if arr1[i] <= arr2[j]:
            result.append(arr1[i])
            i += 1  # Move pointer in first array
        else:
            result.append(arr2[j])
            j += 1  # Move pointer in second array
    
    # Add remaining elements from arr1 (if any)
    result.extend(arr1[i:])
    
    # Add remaining elements from arr2 (if any)
    result.extend(arr2[j:])
    
    return result
```

## Example 4: Technical Specification

### Prompt
```python
prompt = """
Write a technical specification document for a rate limiting system that:
- Supports multiple algorithms (token bucket, sliding window)
- Has Redis backend
- Provides middleware for Flask/FastAPI
"""

response = orchestrator.execute(prompt, agent="claude")
```

### Generated Specification
```markdown
# Rate Limiting System - Technical Specification

## 1. Overview

A flexible rate limiting system supporting multiple algorithms with Redis backend
and framework middleware integration.

## 2. System Architecture

### 2.1 Core Components

- **Rate Limiter Engine**: Algorithm implementation layer
- **Storage Backend**: Redis-based persistence
- **Middleware Layer**: Framework integrations
- **Configuration Manager**: Dynamic rule management

### 2.2 Supported Algorithms

#### Token Bucket
- Configurable capacity and refill rate
- Burst traffic support
- Per-user/IP buckets

#### Sliding Window Log
- Precise rate limiting
- Configurable window size
- Memory-efficient implementation

## 3. API Specification

### 3.1 Core Interface

```python
class RateLimiter:
    def __init__(self, backend: RedisBackend, algorithm: Algorithm):
        pass
    
    def is_allowed(self, key: str, cost: int = 1) -> bool:
        pass
    
    def get_remaining(self, key: str) -> int:
        pass
```

### 3.2 Middleware Integration

```python
# Flask
@app.before_request
@rate_limit(requests=100, window=3600)
def check_rate_limit():
    pass

# FastAPI
@app.middleware("http")
async def rate_limit_middleware(request: Request, call_next):
    pass
```

## 4. Redis Schema

```
rate_limit:{algorithm}:{key} -> {
    "tokens": 100,
    "last_refill": 1234567890,
    "requests": []
}
```

## 5. Performance Requirements

- Latency: < 5ms per check
- Throughput: 10,000 checks/second
- Memory: O(n) where n = unique keys

## 6. Configuration

```yaml
rate_limiting:
  default_algorithm: token_bucket
  redis:
    host: localhost
    port: 6379
  rules:
    - path: /api/*
      limit: 100
      window: 3600
```
```

## Best Practices for Documentation Generation

### 1. Provide Context
Include relevant code structure, project type, and target audience in your prompts.

### 2. Specify Format
Request specific documentation formats (Markdown, reStructuredText, etc.).

### 3. Include Examples
Ask for usage examples and code snippets in documentation.

### 4. Request Sections
Specify required sections like installation, configuration, API reference, etc.

## See Also

- [Prompt Engineering Guide](../guide/prompts.md)
- [Web API Examples](./web-api.md)
- [Testing Examples](./testing.md)