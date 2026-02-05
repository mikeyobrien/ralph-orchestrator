# Building a Web API with Hats

This example demonstrates how to use Hats to build a complete REST API with database integration.

## Task Description

Create a Flask REST API for a todo list application with:
- SQLite database
- CRUD operations
- Input validation
- Error handling
- Unit tests

## PROMPT.md File

```markdown
# Task: Build Todo List REST API

Create a Flask REST API with the following requirements:

## API Endpoints

1. GET /todos - List all todos
2. GET /todos/<id> - Get single todo
3. POST /todos - Create new todo
4. PUT /todos/<id> - Update todo
5. DELETE /todos/<id> - Delete todo

## Data Model

Todo:
- id (integer, primary key)
- title (string, required, max 200 chars)
- description (text, optional)
- completed (boolean, default false)
- created_at (datetime)
- updated_at (datetime)

## Requirements

- Use Flask and SQLAlchemy
- SQLite database
- Input validation
- Proper HTTP status codes
- Error handling with JSON responses
- Unit tests with pytest
- README with API documentation

## File Structure

```
todo-api/
├── app.py           # Main application
├── models.py        # Database models
├── schemas.py       # Validation schemas
├── requirements.txt # Dependencies
├── test_api.py      # Unit tests
└── README.md        # Documentation
```

<!-- The orchestrator will continue iterations until all requirements are met -->
```

## Running Hats

```bash
# Initialize project
hats init

# Copy prompt to PROMPT.md
cp web-api-prompt.md PROMPT.md

# Run with Claude (recommended for complex tasks)
hats run --agent claude --max-iterations 50
```

## Expected Output

Hats will create the following files:

### app.py

```python
from flask import Flask, request, jsonify
from flask_sqlalchemy import SQLAlchemy
from datetime import datetime
from marshmallow import ValidationError
from schemas import TodoSchema
import os

app = Flask(__name__)
app.config['SQLALCHEMY_DATABASE_URI'] = 'sqlite:///todos.db'
app.config['SQLALCHEMY_TRACK_MODIFICATIONS'] = False
db = SQLAlchemy(app)

# Import models after db initialization
from models import Todo

# Create tables
with app.app_context():
    db.create_all()

# Schema instances
todo_schema = TodoSchema()
todos_schema = TodoSchema(many=True)

@app.route('/todos', methods=['GET'])
def get_todos():
    """Get all todos"""
    todos = Todo.query.all()
    return jsonify(todos_schema.dump(todos)), 200

@app.route('/todos/<int:id>', methods=['GET'])
def get_todo(id):
    """Get single todo"""
    todo = Todo.query.get_or_404(id)
    return jsonify(todo_schema.dump(todo)), 200

@app.route('/todos', methods=['POST'])
def create_todo():
    """Create new todo"""
    try:
        data = todo_schema.load(request.json)
    except ValidationError as err:
        return jsonify({'errors': err.messages}), 400
    
    todo = Todo(**data)
    db.session.add(todo)
    db.session.commit()
    
    return jsonify(todo_schema.dump(todo)), 201

@app.route('/todos/<int:id>', methods=['PUT'])
def update_todo(id):
    """Update todo"""
    todo = Todo.query.get_or_404(id)
    
    try:
        data = todo_schema.load(request.json, partial=True)
    except ValidationError as err:
        return jsonify({'errors': err.messages}), 400
    
    for key, value in data.items():
        setattr(todo, key, value)
    
    todo.updated_at = datetime.utcnow()
    db.session.commit()
    
    return jsonify(todo_schema.dump(todo)), 200

@app.route('/todos/<int:id>', methods=['DELETE'])
def delete_todo(id):
    """Delete todo"""
    todo = Todo.query.get_or_404(id)
    db.session.delete(todo)
    db.session.commit()
    
    return '', 204

@app.errorhandler(404)
def not_found(error):
    return jsonify({'error': 'Resource not found'}), 404

@app.errorhandler(500)
def internal_error(error):
    db.session.rollback()
    return jsonify({'error': 'Internal server error'}), 500

if __name__ == '__main__':
    app.run(debug=True)
```

### models.py

```python
from datetime import datetime
from app import db

class Todo(db.Model):
    __tablename__ = 'todos'
    
    id = db.Column(db.Integer, primary_key=True)
    title = db.Column(db.String(200), nullable=False)
    description = db.Column(db.Text)
    completed = db.Column(db.Boolean, default=False)
    created_at = db.Column(db.DateTime, default=datetime.utcnow)
    updated_at = db.Column(db.DateTime, default=datetime.utcnow)
    
    def __repr__(self):
        return f'<Todo {self.id}: {self.title}>'
```

### test_api.py

```python
import pytest
import json
from app import app, db, Todo

@pytest.fixture
def client():
    app.config['TESTING'] = True
    app.config['SQLALCHEMY_DATABASE_URI'] = 'sqlite:///:memory:'
    
    with app.test_client() as client:
        with app.app_context():
            db.create_all()
        yield client

def test_create_todo(client):
    response = client.post('/todos',
        json={'title': 'Test Todo', 'description': 'Test description'})
    assert response.status_code == 201
    data = json.loads(response.data)
    assert data['title'] == 'Test Todo'

def test_get_todos(client):
    # Create test todo
    client.post('/todos', json={'title': 'Test'})
    
    response = client.get('/todos')
    assert response.status_code == 200
    data = json.loads(response.data)
    assert len(data) == 1

def test_update_todo(client):
    # Create todo
    create_response = client.post('/todos', json={'title': 'Original'})
    todo_id = json.loads(create_response.data)['id']
    
    # Update todo
    response = client.put(f'/todos/{todo_id}',
        json={'title': 'Updated'})
    assert response.status_code == 200
    data = json.loads(response.data)
    assert data['title'] == 'Updated'

def test_delete_todo(client):
    # Create todo
    create_response = client.post('/todos', json={'title': 'Delete Me'})
    todo_id = json.loads(create_response.data)['id']
    
    # Delete todo
    response = client.delete(f'/todos/{todo_id}')
    assert response.status_code == 204
    
    # Verify deletion
    get_response = client.get(f'/todos/{todo_id}')
    assert get_response.status_code == 404
```

## Monitoring Progress

```bash
# Watch Hats's progress
hats status

# Monitor in real-time
watch -n 5 'hats status'

# Check logs
tail -f .agent/logs/hats.log
```

## Iteration Examples

### Iteration 1: Project Setup
- Creates project structure
- Initializes Flask application
- Sets up SQLAlchemy configuration

### Iteration 2-5: Model Implementation
- Creates Todo model
- Implements database schema
- Sets up migrations

### Iteration 6-10: API Endpoints
- Implements CRUD operations
- Adds routing
- Handles HTTP methods

### Iteration 11-15: Validation
- Adds input validation
- Implements error handling
- Creates response schemas

### Iteration 16-20: Testing
- Writes unit tests
- Ensures coverage
- Fixes any issues

### Final Iteration
- Creates README
- Adds requirements.txt
- Meets all requirements

## Tips for Success

1. **Clear Requirements**: Be specific about API endpoints and data models
2. **Include Examples**: Provide sample requests/responses if needed
3. **Test Requirements**: Specify testing framework and coverage expectations
4. **Error Handling**: Explicitly request proper error handling
5. **Documentation**: Ask for API documentation in README

## Common Issues and Solutions

### Issue: Database Connection Errors
```markdown
# Add to prompt:
Ensure database is properly initialized before first request.
Use app.app_context() for database operations.
```

### Issue: Import Circular Dependencies
```markdown
# Add to prompt:
Avoid circular imports by importing models after db initialization.
Use application factory pattern if needed.
```

### Issue: Test Failures
```markdown
# Add to prompt:
Use in-memory SQLite database for tests.
Ensure proper test isolation with fixtures.
```

## Extending the Example

### Add Authentication
```markdown
## Additional Requirements
- JWT authentication
- User registration and login
- Protected endpoints
- Role-based access control
```

### Add Pagination
```markdown
## Additional Requirements
- Paginate GET /todos endpoint
- Support page and per_page parameters
- Return pagination metadata
```

### Add Filtering
```markdown
## Additional Requirements
- Filter todos by completed status
- Search todos by title
- Sort by created_at or updated_at
```

## Cost Estimation

- **Iterations**: ~20-30 for complete implementation
- **Time**: ~10-15 minutes
- **Agent**: Claude recommended for complex logic
- **API Calls**: ~$0.20-0.30 (Claude pricing)

## Verification

After Hats completes:

```bash
# Install dependencies
pip install -r requirements.txt

# Run tests
pytest test_api.py -v

# Start server
python app.py

# Test endpoints
curl http://localhost:5000/todos
curl -X POST http://localhost:5000/todos \
  -H "Content-Type: application/json" \
  -d '{"title": "Test Todo"}'
```