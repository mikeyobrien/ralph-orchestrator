# Ralph で Web API を構築する

この例は、Ralph Orchestrator を使って、データベース統合を伴う完全な REST API を構築する
方法を示します。

## タスクの説明

次を備えた Todo リストアプリケーション用の Flask REST API を作成します。
- SQLite データベース
- CRUD 操作
- 入力検証
- エラー処理
- ユニットテスト

## PROMPT.md ファイル

````markdown
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
````

## Ralph の実行

```bash
# プロジェクトを初期化する
ralph init

# プロンプトを PROMPT.md にコピーする
cp web-api-prompt.md PROMPT.md

# Claude で実行する（複雑なタスクに推奨）
ralph run --agent claude --max-iterations 50
```

## 期待される出力

Ralph は次のファイルを作成します。

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

## 進捗の監視

```bash
# Ralph の進捗を見る
ralph status

# リアルタイムで監視する
watch -n 5 'ralph status'

# ログを確認する
tail -f .agent/logs/ralph.log
```

## イテレーションの例

### イテレーション 1: プロジェクトのセットアップ
- プロジェクト構造を作成する
- Flask アプリケーションを初期化する
- SQLAlchemy の設定をセットアップする

### イテレーション 2〜5: モデルの実装
- Todo モデルを作成する
- データベーススキーマを実装する
- マイグレーションをセットアップする

### イテレーション 6〜10: API エンドポイント
- CRUD 操作を実装する
- ルーティングを追加する
- HTTP メソッドを扱う

### イテレーション 11〜15: 検証
- 入力検証を追加する
- エラー処理を実装する
- 応答スキーマを作成する

### イテレーション 16〜20: テスト
- ユニットテストを書く
- カバレッジを確保する
- 問題を修正する

### 最終イテレーション
- README を作成する
- requirements.txt を追加する
- すべての要件を満たす

## 成功のコツ

1. **明確な要件**: API エンドポイントとデータモデルについて具体的にする
2. **例を含める**: 必要ならサンプルのリクエスト/レスポンスを提供する
3. **テスト要件**: テストフレームワークとカバレッジの期待を指定する
4. **エラー処理**: 適切なエラー処理を明示的に求める
5. **ドキュメント**: README での API ドキュメントを求める

## よくある問題と解決策

### 問題: データベース接続エラー
```markdown
# Add to prompt:
Ensure database is properly initialized before first request.
Use app.app_context() for database operations.
```

### 問題: 循環インポートの依存
```markdown
# Add to prompt:
Avoid circular imports by importing models after db initialization.
Use application factory pattern if needed.
```

### 問題: テストの失敗
```markdown
# Add to prompt:
Use in-memory SQLite database for tests.
Ensure proper test isolation with fixtures.
```

## 例の拡張

### 認証を追加する
```markdown
## Additional Requirements
- JWT authentication
- User registration and login
- Protected endpoints
- Role-based access control
```

### ページネーションを追加する
```markdown
## Additional Requirements
- Paginate GET /todos endpoint
- Support page and per_page parameters
- Return pagination metadata
```

### フィルタリングを追加する
```markdown
## Additional Requirements
- Filter todos by completed status
- Search todos by title
- Sort by created_at or updated_at
```

## コストの見積もり

- **イテレーション**: 完全な実装で約 20〜30
- **時間**: 約 10〜15 分
- **エージェント**: 複雑なロジックには Claude 推奨
- **API 呼び出し**: 約 $0.20〜0.30（Claude の価格）

## 検証

Ralph が完了した後:

```bash
# 依存をインストールする
pip install -r requirements.txt

# テストを実行する
pytest test_api.py -v

# サーバーを起動する
python app.py

# エンドポイントをテストする
curl http://localhost:5000/todos
curl -X POST http://localhost:5000/todos \
  -H "Content-Type: application/json" \
  -d '{"title": "Test Todo"}'
```
