# シンプルなタスクの例: Todo リスト CLI

この例は、Ralph Orchestrator を使って単純なコマンドライン Todo リストアプリケーションを
構築する方法を示します。

## 概要

次のような Python CLI アプリケーションを作成します。
- Todo 項目を管理する（追加、一覧、完了、削除）
- データを JSON ファイルに永続化する
- 色付き出力を含む
- 包括的なエラー処理を持つ

## プロンプト

`todo-prompt.md` ファイルを作成します。

```markdown
# Build Todo List CLI Application

## Objective
Create a command-line todo list manager with file persistence.

## Requirements

### Core Features
1. Add new todo items with descriptions
2. List all todos with status
3. Mark todos as complete
4. Remove todos
5. Clear all todos
6. Save todos to JSON file

### Technical Specifications
- Language: Python 3.8+
- File storage: todos.json
- Use argparse for CLI
- Add colored output (use colorama or ANSI codes)
- Include proper error handling

### Commands
- `todo add <description>` - Add new todo
- `todo list` - Show all todos
- `todo done <id>` - Mark as complete
- `todo remove <id>` - Delete todo
- `todo clear` - Remove all todos

### File Structure
```
todo-app/
├── todo.py          # Main CLI application
├── todos.json       # Data storage
├── test_todo.py     # Unit tests
└── README.md        # Documentation
```

## Example Usage

```bash
$ python todo.py add "Buy groceries"
✅ Added: Buy groceries (ID: 1)

$ python todo.py add "Write documentation"
✅ Added: Write documentation (ID: 2)

$ python todo.py list
Todo List:
[ ] 1. Buy groceries
[ ] 2. Write documentation

$ python todo.py done 1
✅ Completed: Buy groceries

$ python todo.py list
Todo List:
[✓] 1. Buy groceries
[ ] 2. Write documentation

$ python todo.py remove 1
✅ Removed: Buy groceries
```

## Data Format

todos.json:
```json
{
  "todos": [
    {
      "id": 1,
      "description": "Buy groceries",
      "completed": false,
      "created_at": "2024-01-10T10:00:00",
      "completed_at": null
    }
  ],
  "next_id": 2
}
```

## Success Criteria
- [ ] All commands working as specified
- [ ] Data persists between runs
- [ ] Colored output for better UX
- [ ] Error handling for edge cases
- [ ] Tests cover main functionality
- [ ] README with usage instructions

The orchestrator will continue iterations until all criteria are met or limits reached.
```

## 例の実行

### 基本的な実行

```bash
python ralph_orchestrator.py --prompt todo-prompt.md
```

### 特定の設定で

```bash
# 予算を意識したアプローチ
python ralph_orchestrator.py \
  --agent q \
  --prompt todo-prompt.md \
  --max-cost 2.0 \
  --max-iterations 20

# 品質重視のアプローチ
python ralph_orchestrator.py \
  --agent claude \
  --prompt todo-prompt.md \
  --max-cost 10.0 \
  --checkpoint-interval 3
```

## 期待される結果

### イテレーション

典型的な完了: 5〜15 イテレーション

### コストの見積もり

- **Q Chat**: $0.50 〜 $1.50
- **Gemini**: $0.75 〜 $2.00
- **Claude**: $2.00 〜 $5.00

### 作成されるファイル

正常に完了した後:

```
todo-app/
├── todo.py          # ~200 行
├── todos.json       # 初期の空の構造
├── test_todo.py     # ~100 行
└── README.md        # ~50 行
```

## 出力サンプル

生成される `todo.py` は次のようになるかもしれません。

```python
#!/usr/bin/env python3
"""
Todo List CLI Application
A simple command-line todo manager with JSON persistence.
"""

import argparse
import json
import os
from datetime import datetime
from pathlib import Path

# ANSI color codes
GREEN = '\033[92m'
YELLOW = '\033[93m'
RED = '\033[91m'
RESET = '\033[0m'
BOLD = '\033[1m'

class TodoManager:
    def __init__(self, filename='todos.json'):
        self.filename = filename
        self.todos = self.load_todos()
    
    def load_todos(self):
        """Load todos from JSON file."""
        if not os.path.exists(self.filename):
            return {'todos': [], 'next_id': 1}
        
        try:
            with open(self.filename, 'r') as f:
                return json.load(f)
        except json.JSONDecodeError:
            return {'todos': [], 'next_id': 1}
    
    def save_todos(self):
        """Save todos to JSON file."""
        with open(self.filename, 'w') as f:
            json.dump(self.todos, f, indent=2, default=str)
    
    def add_todo(self, description):
        """Add a new todo item."""
        todo = {
            'id': self.todos['next_id'],
            'description': description,
            'completed': False,
            'created_at': datetime.now().isoformat(),
            'completed_at': None
        }
        
        self.todos['todos'].append(todo)
        self.todos['next_id'] += 1
        self.save_todos()
        
        print(f"{GREEN}✅ Added: {description} (ID: {todo['id']}){RESET}")
        return todo['id']
    
    def list_todos(self):
        """List all todos."""
        if not self.todos['todos']:
            print(f"{YELLOW}No todos found.{RESET}")
            return
        
        print(f"{BOLD}Todo List:{RESET}")
        for todo in self.todos['todos']:
            status = f"{GREEN}[✓]{RESET}" if todo['completed'] else "[ ]"
            print(f"{status} {todo['id']}. {todo['description']}")
    
    def complete_todo(self, todo_id):
        """Mark a todo as complete."""
        for todo in self.todos['todos']:
            if todo['id'] == todo_id:
                todo['completed'] = True
                todo['completed_at'] = datetime.now().isoformat()
                self.save_todos()
                print(f"{GREEN}✅ Completed: {todo['description']}{RESET}")
                return True
        
        print(f"{RED}❌ Todo with ID {todo_id} not found.{RESET}")
        return False
    
    def remove_todo(self, todo_id):
        """Remove a todo."""
        for i, todo in enumerate(self.todos['todos']):
            if todo['id'] == todo_id:
                removed = self.todos['todos'].pop(i)
                self.save_todos()
                print(f"{GREEN}✅ Removed: {removed['description']}{RESET}")
                return True
        
        print(f"{RED}❌ Todo with ID {todo_id} not found.{RESET}")
        return False
    
    def clear_todos(self):
        """Clear all todos."""
        count = len(self.todos['todos'])
        self.todos = {'todos': [], 'next_id': 1}
        self.save_todos()
        print(f"{GREEN}✅ Cleared {count} todos.{RESET}")

def main():
    parser = argparse.ArgumentParser(description='Todo List CLI')
    subparsers = parser.add_subparsers(dest='command', help='Commands')
    
    # Add command
    add_parser = subparsers.add_parser('add', help='Add a new todo')
    add_parser.add_argument('description', nargs='+', help='Todo description')
    
    # List command
    subparsers.add_parser('list', help='List all todos')
    
    # Done command
    done_parser = subparsers.add_parser('done', help='Mark todo as complete')
    done_parser.add_argument('id', type=int, help='Todo ID')
    
    # Remove command
    remove_parser = subparsers.add_parser('remove', help='Remove a todo')
    remove_parser.add_argument('id', type=int, help='Todo ID')
    
    # Clear command
    subparsers.add_parser('clear', help='Clear all todos')
    
    args = parser.parse_args()
    
    if not args.command:
        parser.print_help()
        return
    
    manager = TodoManager()
    
    if args.command == 'add':
        description = ' '.join(args.description)
        manager.add_todo(description)
    elif args.command == 'list':
        manager.list_todos()
    elif args.command == 'done':
        manager.complete_todo(args.id)
    elif args.command == 'remove':
        manager.remove_todo(args.id)
    elif args.command == 'clear':
        manager.clear_todos()

if __name__ == '__main__':
    main()
```

## バリエーション

### 1. 拡張版

これらの機能をプロンプトに追加します。

```markdown
## Additional Features
- Priority levels (high, medium, low)
- Due dates with reminders
- Categories/tags
- Search functionality
- Export to CSV/Markdown
```

### 2. Web インターフェース

Web アプリケーションに変えます。

```markdown
## Web Version
Instead of CLI, create a Flask web app with:
- HTML interface
- REST API endpoints
- SQLite database
- Basic authentication
```

### 3. 共同編集版

マルチユーザーのサポートを追加します。

```markdown
## Multi-User Features
- User accounts
- Shared todo lists
- Permissions (view/edit)
- Activity logging
```

## トラブルシューティング

### 問題: ファイルが作成されない

**解決策**: エージェントに書き込み権限があることを確認します。

```bash
# 権限を確認する
ls -la

# 明示的なパスで実行する
python ralph_orchestrator.py --prompt ./todo-prompt.md
```

### 問題: テストが失敗する

**解決策**: テストフレームワークを指定します。

```markdown
## Testing Requirements
Use pytest for testing:
- Install: pip install pytest
- Run: pytest test_todo.py
- Coverage: pytest --cov=todo
```

### 問題: 色が機能しない

**解決策**: Windows 向けのフォールバックを追加します。

```markdown
## Color Output
- Try colorama first (cross-platform)
- Fall back to ANSI codes
- Detect terminal support
- Add --no-color option
```

## 学びのポイント

### この例が教えること

1. **CLI 開発**: argparse を効果的に使う
2. **データの永続化**: JSON ファイルの扱い
3. **エラー処理**: グレースフルな失敗モード
4. **ユーザー体験**: 色付き出力と明確なフィードバック
5. **テスト**: CLI アプリのユニットテストを書く

### 主要なパターン

- CLI アクションのためのコマンドパターン
- データ保存のためのリポジトリパターン
- 関心事の明確な分離
- 包括的なエラーメッセージ

## 次のステップ

この例を完了した後:

1. **機能を拡張する**: 上記のバリエーションを追加する
2. **テストを改善する**: 統合テストを追加する
3. **パッケージ化する**: 配布用に setup.py を作成する
4. **CI/CD を追加する**: GitHub Actions のワークフロー

## 関連する例

- [Web API の例](web-api.ja.md) - REST API 版を構築する
- [CLI ツールの例](cli-tool.ja.md) - より高度な CLI パターン
- [データ分析の例](data-analysis.ja.md) - Todo の統計を処理する

---

📚 [Web API の例](web-api.ja.md) に進む →
