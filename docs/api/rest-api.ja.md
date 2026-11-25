# REST API リファレンス

レガシーの Node Web Server は、古い tRPC/REST サーフェスに依存している利用者向けに
`/api/v1/*` で REST API を公開します。この API は非推奨です。RPC v1（`/rpc/v1`）が正典の
コントロールプレーンです。

## ベース URL

```
http://localhost:3000/api/v1
```

## エンドポイント

### ヘルス

#### GET /api/v1/health

サーバーのヘルス状態を返します。

**レスポンス** `200 OK`
```json
{
  "status": "ok",
  "version": "1.0.0",
  "timestamp": "2026-01-29T12:00:00.000Z"
}
```

---

### タスク

#### GET /api/v1/tasks

すべてのタスクを一覧します。

**クエリパラメータ**

| パラメータ | 型 | 説明 |
|-----------|------|-------------|
| `status` | string | 状態でフィルタする（`open`、`running`、`closed`、`failed`、`pending`） |
| `includeArchived` | string | アーカイブ済みタスクも含めるには `"true"` を設定する |

**レスポンス** `200 OK`
```json
[
  {
    "id": "task-abc123",
    "title": "Implement feature",
    "status": "open",
    "priority": 2,
    "blockedBy": null,
    "preset": null,
    "currentIteration": null,
    "maxIterations": null,
    "loopId": null,
    "createdAt": "2026-01-29T10:00:00.000Z"
  }
]
```

#### POST /api/v1/tasks

新しいタスクを作成します。

**リクエストボディ**

| フィールド | 型 | 必須 | 説明 |
|-------|------|----------|-------------|
| `id` | string | Yes | 一意なタスク識別子 |
| `title` | string | Yes | タスクのタイトル |
| `status` | string | No | 初期状態（既定: `"open"`） |
| `priority` | number | No | 優先度 1-5、1が最高（既定: `2`） |
| `blockedBy` | string\|null | No | ブロックしているタスクの ID |
| `autoExecute` | boolean | No | ブロッカーがなければ実行キューに自動投入する |
| `preset` | string | No | 関連付けるプリセット名 |

**レスポンス** `201 Created`
```json
{
  "id": "task-abc123",
  "title": "Implement feature",
  "status": "open",
  "priority": 2
}
```

**エラー**
- `400` — `id` または `title` が欠落している、あるいは `priority` が範囲外（1-5）

#### GET /api/v1/tasks/:id

ID で単一のタスクを取得します。

**レスポンス** `200 OK` — タスクオブジェクト（一覧項目と同じ形）

**エラー**
- `404` — タスクが見つからない

#### PATCH /api/v1/tasks/:id

既存のタスクを更新します。

**リクエストボディ**（すべてのフィールドは任意）

| フィールド | 型 | 説明 |
|-------|------|-------------|
| `title` | string | 新しいタイトル（空にはできない） |
| `status` | string | 新しい状態 |
| `priority` | number | 新しい優先度（1-5） |
| `blockedBy` | string\|null | ブロッカーを設定または解除する |

**レスポンス** `200 OK` — 更新されたタスクオブジェクト

**エラー**
- `400` — 無効な優先度、または空のタイトル
- `404` — タスクが見つからない

#### DELETE /api/v1/tasks/:id

タスクを削除します。`failed` または `closed` 状態のタスクのみ削除できます。

**レスポンス** `204 No Content`

**エラー**
- `404` — タスクが見つからない
- `409` — タスクが削除不可能な状態（例: `running`、`open`、`pending`）

#### POST /api/v1/tasks/:id/run

タスクを実行キューに投入します。サーバー側で TaskBridge が構成されている必要があります。

**レスポンス** `200 OK`
```json
{
  "success": true,
  "queuedTaskId": "queued-xyz",
  "task": { ... }
}
```

**エラー**
- `400` — キュー投入に失敗
- `404` — タスクが見つからない
- `503` — タスク実行が未構成（TaskBridge なし）

---

### ハット

#### GET /api/v1/hats

すべてのハット定義を、アクティブ状態とともに一覧します。

**レスポンス** `200 OK`
```json
[
  {
    "key": "execution-lead",
    "name": "Execution Lead",
    "description": "Implements tasks and verifies results",
    "isActive": true
  }
]
```

#### GET /api/v1/hats/:key

キーで特定のハットを取得します。

**レスポンス** `200 OK` — `isActive` フラグ付きのハットオブジェクト

**エラー**
- `404` — ハットが見つからない

---

### プリセット

#### GET /api/v1/presets

すべてのソースから利用可能なプリセットを一覧します。

プリセットは優先順位順に返されます。
1. **builtin** — Ralph に同梱（`presets/` ディレクトリから）
2. **directory** — ユーザー作成（`.ralph/hats/` から）
3. **collection** — データベースのコレクション（Builder 経由で作成）

**レスポンス** `200 OK`
```json
[
  {
    "id": "tdd-red-green",
    "name": "tdd-red-green",
    "source": "builtin",
    "description": "TDD workflow with red-green-refactor cycle"
  },
  {
    "id": "my-custom",
    "name": "my-custom",
    "source": "directory",
    "path": ".ralph/hats/my-custom.yml"
  },
  {
    "id": "uuid-abc-123",
    "name": "My Collection",
    "source": "collection",
    "description": "Custom hat collection"
  }
]
```

---

## エラー形式

すべてのエラーレスポンスは次の構造に従います。

```json
{
  "error": "Not Found",
  "message": "Task with id 'task-xyz' not found"
}
```

## レガシーサーバーの実行

```bash
ralph web --legacy-node-api  # 非推奨の Node バックエンド + フロントエンドを起動する
npm run dev:legacy-server    # Node バックエンドのみ
```

## 認証

REST API は現在、認証を必要としません。ローカル開発用途を想定して設計されています。
