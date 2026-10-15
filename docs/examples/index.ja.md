# 例

Ralph の実際の動作を示す実践的な例です。

## このセクションの内容

| 例 | 説明 |
|---------|-------------|
| [シンプルなタスク](simple-task.ja.md) | 基本的な従来型モードの使い方 |
| [TDD ワークフロー](tdd-workflow.ja.md) | ハットを使ったテスト駆動開発 |
| [自動 PDD 設計](pdd-design.ja.md) | 模擬要件インタビューを伴う、例のみの設計ワークフロー |
| [仕様駆動開発](spec-driven.ja.md) | 例のみのワークフローパターン。出荷される組み込みではない |
| [マルチハットワークフロー](multi-hat.ja.md) | ハット間の複雑な連携 |
| [デバッグ](debugging.ja.md) | Ralph を使ったバグ調査 |

## 手早い例

### 従来型モード

完了までの単純なループ:

```bash
ralph init --backend claude

cat > PROMPT.md << 'EOF'
Write a function that calculates factorial.
Include tests.
EOF

ralph run
```

### ハットベースモード

組み込みのハットコレクションを使う:

```bash
ralph init --backend claude

cat > PROMPT.md << 'EOF'
Implement a URL validator function.
Must handle:
- HTTP and HTTPS protocols
- IPv4 addresses
- Domain names
- Port numbers
EOF

ralph run -c ralph.yml -H builtin:code-assist
```

### インラインプロンプト

プロンプトファイルを省く:

```bash
ralph run -p "Add input validation to the signup form"
```

### カスタム設定

既定を上書きする:

```bash
ralph run --max-iterations 50 -p "Refactor the authentication module"
```

## ワークフローの例

### 機能開発

```bash
# コア設定を初期化する
ralph init --backend claude

# 詳細なプロンプトを作成する
cat > PROMPT.md << 'EOF'
# Feature: User Dashboard

Add a user dashboard with:
- Profile summary widget
- Recent activity feed
- Quick action buttons

Use React components.
Follow existing UI patterns.
EOF

# 既定の実装ハットで Ralph を実行する
ralph run -c ralph.yml -H builtin:code-assist
```

### バグ調査

```bash
# debug ハットコレクションを使う
ralph run -c ralph.yml -H builtin:debug -p "Users report login fails on Safari. Error: 'Invalid token'. Investigate and fix."
```

### コードレビュー

```bash
# review ハットコレクションを使う
ralph run -c ralph.yml -H builtin:review -p "Review the changes in src/api/auth.rs for security issues"
```

## 完全な例

詳しい手順が利用できます。

- [シンプルなタスク](simple-task.ja.md) — 従来型モードのステップバイステップ
- [TDD ワークフロー](tdd-workflow.ja.md) — ハットによるレッド・グリーン・リファクタ
- [自動 PDD 設計](pdd-design.ja.md) — レビュー済みの設計パッケージで終わる模擬インタビュー
- [仕様駆動](spec-driven.ja.md) — 仕様優先のパターンの例
- [マルチハット](multi-hat.ja.md) — 複雑なハットの連携
- [デバッグ](debugging.ja.md) — バグ調査のワークフロー
