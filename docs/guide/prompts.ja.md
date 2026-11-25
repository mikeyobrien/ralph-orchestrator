# プロンプトエンジニアリングガイド

効果的なプロンプトエンジニアリングは、Ralph Orchestrator のタスクを成功させる鍵です。この
ガイドは、結果を出すプロンプトを書くためのベストプラクティス、パターン、テクニックを
扱います。

## プロンプトファイルの基本

### ファイル形式

Ralph Orchestrator は、プロンプトに Markdown ファイルを使います。

```markdown
# Task Title

## Objective
Clear description of what needs to be accomplished.

## Requirements
- Specific requirement 1
- Specific requirement 2

## Success Criteria
The task is complete when:
- Criterion 1 is met
- Criterion 2 is met

The orchestrator will run until iteration/time/cost limits are reached.
```

### ファイルの場所

既定のプロンプトファイル: `PROMPT.md`

カスタムの場所:
```bash
python ralph_orchestrator.py --prompt path/to/task.md
```

## プロンプトの構造

### 必須の構成要素

すべてのプロンプトに次を含めるべきです。

1. **明確な目的**
2. **具体的な要件**
3. **成功基準**
4. **完了マーカー**

### テンプレート

```markdown
# [Task Name]

## Objective
[One or two sentences describing the goal]

## Context
[Background information the agent needs]

## Requirements
1. [Specific requirement]
2. [Specific requirement]
3. [Specific requirement]

## Constraints
- [Limitation or boundary]
- [Technical constraint]
- [Resource constraint]

## Success Criteria
The task is complete when:
- [ ] [Measurable outcome]
- [ ] [Verifiable result]
- [ ] [Specific deliverable]

## Notes
[Additional guidance or hints]

---
The orchestrator will continue iterations until limits are reached.
```

## プロンプトのパターン

### 1. ソフトウェア開発のパターン

```markdown
# Build Web API

## Objective
Create a RESTful API for user management with authentication.

## Requirements
1. Implement user CRUD operations
2. Add JWT authentication
3. Include input validation
4. Write comprehensive tests
5. Create API documentation

## Technical Specifications
- Framework: FastAPI
- Database: PostgreSQL
- Authentication: JWT tokens
- Testing: pytest

## Endpoints
- POST /auth/register
- POST /auth/login
- GET /users
- GET /users/{id}
- PUT /users/{id}
- DELETE /users/{id}

## Success Criteria
- [ ] All endpoints functional
- [ ] Tests passing with >80% coverage
- [ ] API documentation generated
- [ ] Authentication working

The orchestrator will run until completion criteria are met or limits reached.
```

### 2. ドキュメントのパターン

````markdown
# Create User Documentation

## Objective
Write comprehensive user documentation for the application.

## Requirements
1. Installation guide
2. Configuration reference
3. Usage examples
4. Troubleshooting section
5. FAQ

## Structure
```
docs/
├── getting-started.md
├── installation.md
├── configuration.md
├── usage/
│   ├── basic.md
│   └── advanced.md
├── troubleshooting.md
└── faq.md
```

## Style Guide
- Use clear, concise language
- Include code examples
- Add screenshots where helpful
- Follow Markdown best practices

## Success Criteria
- [ ] All sections complete
- [ ] Examples tested and working
- [ ] Reviewed for clarity
- [ ] No broken links

The orchestrator will continue iterations until limits are reached.
````

### 3. データ分析のパターン

```markdown
# Analyze Sales Data

## Objective
Analyze Q4 sales data and generate insights report.

## Data Sources
- sales_data.csv
- customer_demographics.json
- product_catalog.xlsx

## Analysis Requirements
1. Revenue trends by month
2. Top performing products
3. Customer segmentation
4. Regional performance
5. Year-over-year comparison

## Deliverables
1. Python analysis script
2. Jupyter notebook with visualizations
3. Executive summary (PDF)
4. Raw data exports

## Success Criteria
- [ ] All analyses complete
- [ ] Visualizations created
- [ ] Insights documented
- [ ] Code reproducible

The orchestrator will run until limits are reached.
```

### 4. デバッグのパターン

```markdown
# Debug Application Issue

## Problem Description
Users report application crashes when uploading large files.

## Symptoms
- Crash occurs with files >100MB
- Error: "Memory allocation failed"
- Affects 30% of users

## Investigation Steps
1. Reproduce the issue
2. Analyze memory usage
3. Review upload handling code
4. Check server resources
5. Examine error logs

## Required Fixes
- Identify root cause
- Implement solution
- Add error handling
- Write regression tests
- Update documentation

## Success Criteria
- [ ] Issue reproduced
- [ ] Root cause identified
- [ ] Fix implemented
- [ ] Tests passing
- [ ] No regressions

The orchestrator will continue verification iterations until limits are reached.
```

## ベストプラクティス

### 1. 具体的にする

❌ **悪い:**
```markdown
Build a website
```

✅ **良い:**
```markdown
Build a responsive e-commerce website using React and Node.js with:
- Product catalog with search
- Shopping cart functionality
- Stripe payment integration
- User authentication
- Order tracking
```

### 2. コンテキストを与える

❌ **悪い:**
```markdown
Fix the bug
```

✅ **良い:**
```markdown
Fix the memory leak in the image processing module that occurs when:
- Processing images larger than 10MB
- Multiple images are processed simultaneously
- The cleanup function in ImageProcessor.process() may not be releasing buffers
```

### 3. 成功を明確に定義する

❌ **悪い:**
```markdown
Make it work better
```

✅ **良い:**
```markdown
## Success Criteria
- Response time < 200ms for 95% of requests
- Memory usage stays below 512MB
- All unit tests pass
- No errors in 24-hour stress test
```

### 4. 例を含める

```markdown
## Example Input/Output

Input:
```json
{
  "user_id": 123,
  "action": "purchase",
  "items": ["SKU-001", "SKU-002"]
}
```

Expected Output:
```json
{
  "order_id": "ORD-789",
  "status": "confirmed",
  "total": 99.99,
  "estimated_delivery": "2024-01-15"
}
```
```

### 5. 制約を明示する

```markdown
## Constraints
- Must be Python 3.8+ compatible
- Cannot use external APIs
- Must complete in under 5 seconds
- Memory usage < 1GB
- Must follow PEP 8 style guide
```

## 反復的なプロンプト

Ralph Orchestrator は実行中にプロンプトファイルを変更します。反復をサポートするプロンプトを
設計してください。

### 自己文書化する進捗

```markdown
## Progress Log
<!-- Agent will update this section -->
- [ ] Step 1: Setup environment
- [ ] Step 2: Implement core logic
- [ ] Step 3: Add tests
- [ ] Step 4: Documentation

## Current Status
<!-- Agent updates this -->
Working on: [current task]
Completed: [list of completed items]
Next: [planned next step]
```

### チェックポイントマーカー

```markdown
## Checkpoints
- [ ] CHECKPOINT_1: Basic structure complete
- [ ] CHECKPOINT_2: Core functionality working
- [ ] CHECKPOINT_3: Tests passing
- [ ] CHECKPOINT_4: Documentation complete
- [ ] All criteria verified
```

## 高度なテクニック

### 1. 多段階のプロンプト

```markdown
# Phase 1: Research
Research existing solutions and document findings.

<!-- After Phase 1 complete, update prompt for Phase 2 -->

# Phase 2: Implementation
Based on research, implement the solution.

# Phase 3: Testing
Comprehensive testing and validation.
```

### 2. 条件付きの指示

```markdown
## Implementation

If using Python:
- Use type hints
- Follow PEP 8
- Use pytest for testing

If using JavaScript:
- Use TypeScript
- Follow Airbnb style guide
- Use Jest for testing
```

### 3. 学習するプロンプト

```markdown
## Approach
1. First, try the simple solution
2. If that doesn't work, research alternatives
3. Document what was learned
4. Implement the best solution

## Document Learnings
<!-- Agent fills this during execution -->
- Attempted: [approach]
- Result: [outcome]
- Learning: [insight]
```

### 4. エラーからの回復

```markdown
## Error Handling
If you encounter errors:
1. Document the error in this file
2. Research the solution
3. Try alternative approaches
4. Update this prompt with findings

## Error Log
<!-- Agent updates this -->
```

## プロンプトのセキュリティ

### サニタイズ

Ralph Orchestrator は、次についてプロンプトを自動的にサニタイズします。

- コマンドインジェクションの試み
- パストラバーサル攻撃
- 悪意あるパターン

### 安全なパターン

```markdown
## File Operations
Work only in the ./workspace directory
Do not modify system files
Create backups before changes
```

### サイズ制限

既定の最大プロンプトサイズ: 10MB

必要に応じて調整します。
```bash
python ralph_orchestrator.py --max-prompt-size 20971520  # 20MB
```

## プロンプトのテスト

### ドライラン

実行せずにプロンプトをテストします。

```bash
python ralph_orchestrator.py --dry-run --prompt test.md
```

### 限られたイテレーション

少ないイテレーションでテストします。

```bash
python ralph_orchestrator.py --max-iterations 3 --prompt test.md
```

### 詳細モード

プロンプト処理をデバッグします。

```bash
python ralph_orchestrator.py --verbose --prompt test.md
```

## よくある落とし穴

### 1. 曖昧な指示

❌ **避ける:**
- 「Make it good」
- 「Optimize everything」
- 「Fix all issues」

✅ **代わりに:**
- 「Achieve 95% test coverage」
- 「Reduce response time to <100ms」
- 「Fix the memory leak in process_image()」

### 2. 完了基準の欠落

❌ **避ける:**
タスクがいつ完了するかを指定し忘れる

✅ **代わりに:**
オーケストレーターが目指せる明確な完了基準を必ず含める

### 3. 過度に複雑なプロンプト

❌ **避ける:**
50 以上の要件を持つ単一のプロンプト

✅ **代わりに:**
フェーズまたは別々のタスクに分割する

### 4. 例がない

❌ **避ける:**
例なしで望む挙動を説明する

✅ **代わりに:**
入出力の例とエッジケースを含める

## プロンプトライブラリ

### スターターテンプレート

1. [Web API 開発](../examples/web-api.ja.md)
2. [CLI ツールの作成](../examples/cli-tool.ja.md)
3. [データ分析](../examples/data-analysis.ja.md)
4. [ドキュメントの執筆](../examples/documentation.ja.md)
5. [バグ修正](../examples/bug-fix.ja.md)
6. [テストスイート](../examples/testing.ja.md)

## 次のステップ

- 効率的なプロンプトのために [コスト管理](cost-management.ja.md) を探る
- 最適な結果のために [エージェントの選択](agents.ja.md) を見直す
- 実世界のプロンプトについて [例](../examples/index.ja.md) を見る
