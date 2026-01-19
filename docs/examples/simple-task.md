# Simple Task Example: Todo List CLI

This example demonstrates building a simple command-line todo list application using Ralph Orchestrator.

## Overview

We'll create a Python CLI application that:
- Manages todo items (add, list, complete, remove)
- Persists data to JSON file
- Includes colored output
- Has comprehensive error handling

## The Prompt

Create a file `todo-prompt.md`:

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

## Running the Example

### Basic Execution

```bash
ralph run --prompt todo-prompt.md
```

### With Specific Settings

```bash
# Budget-conscious approach
ralph run \
  --agent q \
  --prompt todo-prompt.md \
  --max-cost 2.0 \
  --max-iterations 20

# Quality-focused approach
ralph run \
  --agent claude \
  --prompt todo-prompt.md \
  --max-cost 10.0 \
  --checkpoint-interval 3
```

## Expected Results

### Iterations

Typical completion: 5-15 iterations

### Cost Estimates

- **Q Chat**: $0.50 - $1.50
- **Gemini**: $0.75 - $2.00
- **Claude**: $2.00 - $5.00

### Files Created

After successful completion:

```
todo-app/
├── todo.py          # ~200 lines
├── todos.json       # Initial empty structure
├── test_todo.py     # ~100 lines
└── README.md        # ~50 lines
```

## Sample Output

Here's what the generated `todo.py` might look like:

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

## Variations

### 1. Enhanced Version

Add these features to the prompt:

```markdown
## Additional Features
- Priority levels (high, medium, low)
- Due dates with reminders
- Categories/tags
- Search functionality
- Export to CSV/Markdown
```

### 2. Web Interface

Transform to a web application:

```markdown
## Web Version
Instead of CLI, create a Flask web app with:
- HTML interface
- REST API endpoints
- SQLite database
- Basic authentication
```

### 3. Collaborative Version

Add multi-user support:

```markdown
## Multi-User Features
- User accounts
- Shared todo lists
- Permissions (view/edit)
- Activity logging
```

## Troubleshooting

### Issue: File Not Created

**Solution**: Ensure the agent has write permissions:

```bash
# Check permissions
ls -la

# Run with explicit path
ralph run --prompt ./todo-prompt.md
```

### Issue: Tests Failing

**Solution**: Specify test framework:

```markdown
## Testing Requirements
Use pytest for testing:
- Install: pip install pytest
- Run: pytest test_todo.py
- Coverage: pytest --cov=todo
```

### Issue: Colors Not Working

**Solution**: Add fallback for Windows:

```markdown
## Color Output
- Try colorama first (cross-platform)
- Fall back to ANSI codes
- Detect terminal support
- Add --no-color option
```

## Learning Points

### What This Example Teaches

1. **CLI Development**: Using argparse effectively
2. **Data Persistence**: JSON file handling
3. **Error Handling**: Graceful failure modes
4. **User Experience**: Colored output and clear feedback
5. **Testing**: Writing unit tests for CLI apps

### Key Patterns

- Command pattern for CLI actions
- Repository pattern for data storage
- Clear separation of concerns
- Comprehensive error messages

## Next Steps

After completing this example:

1. **Extend Features**: Add the variations mentioned above
2. **Improve Testing**: Add integration tests
3. **Package It**: Create setup.py for distribution
4. **Add CI/CD**: GitHub Actions workflow

## Related Examples

- [Web API Example](web-api.md) - Build a REST API version
- [CLI Tool Example](cli-tool.md) - More advanced CLI patterns
- [Data Analysis Example](data-analysis.md) - Process todo statistics

---

📚 Continue to [Web API Example](web-api.md) →