# WebSearch 連携ガイド

## 概要

Ralph Orchestrator は現在、Claude アダプタ向けに完全な WebSearch サポートを備えており、
Claude が最新の情報を検索したり、トピックを調べたり、知識のカットオフを超えたデータに
アクセスしたりできます。

## 機能

WebSearch により Claude は次のことができます。

- 時事や最近のニュースを検索する
- 技術文書やベストプラクティスを調べる
- ライブラリやフレームワークの最新情報を見つける
- 複数のソースからデータを収集する
- リアルタイムの情報（天気、株価など）にアクセスする

## 設定

### 既定の設定

WebSearch は、Claude アダプタを使うときに**既定で有効**です。追加の設定は不要です。

### 明示的な設定

WebSearch はいくつかの方法で明示的に制御できます。

#### 1. CLI 経由（自動）

Ralph を Claude で使うとき、WebSearch は自動的に有効になります。

```bash
ralph -a claude  # WebSearch は既定で有効
```

#### 2. アダプタ設定経由

```python
from src.ralph_orchestrator.adapters.claude import ClaudeAdapter

# WebSearch を有効にしてアダプタを作成する（既定）
adapter = ClaudeAdapter()
adapter.configure(enable_web_search=True)  # これが既定

# 必要なら WebSearch を無効にする
adapter.configure(enable_web_search=False)
```

#### 3. オーケストレーター経由

```python
from src.ralph_orchestrator.orchestrator import RalphOrchestrator

orchestrator = RalphOrchestrator(
    prompt_file="TASK.md",
    primary_tool="claude"
)

# Claude アダプタは自動的に WebSearch が有効
orchestrator.run()
```

## 使用例

### 例 1: 最新トピックの調査

```python
adapter = ClaudeAdapter()
adapter.configure(enable_all_tools=True)

response = adapter.execute("""
    Search the web for the latest developments in quantum computing
    and create a summary of the most significant breakthroughs in 2024.
""")
```

### 例 2: 技術文書の調査

```python
response = adapter.execute("""
    Use WebSearch to find the latest best practices for Python async programming.
    Compare different approaches and provide recommendations.
""", enable_web_search=True)
```

### 例 3: リアルタイム情報

```python
response = adapter.execute("""
    Search for current weather conditions in major tech hubs:
    - San Francisco
    - Seattle  
    - Austin
    - New York
    
    Also find the current stock prices for major tech companies.
""", enable_all_tools=True)
```

### 例 4: フレームワークの調査

```python
response = adapter.execute("""
    Research the latest features in React 19 and Next.js 15.
    Use WebSearch to find migration guides and breaking changes.
    Create a comparison table of new features.
""")
```

## 他のツールとの組み合わせ

WebSearch は、他の Claude ツールとシームレスに連携します。

```python
response = adapter.execute("""
    1. Use WebSearch to find the latest Python web framework benchmarks
    2. Create a comparison table in a file called benchmarks.md
    3. Search local codebase for current framework usage
    4. Provide recommendations based on findings
""", enable_all_tools=True)
```

## WebSearch のテスト

同梱のテストスクリプトを実行して WebSearch の機能を検証します。

```bash
python test_websearch.py
```

これは次をテストします。

- 基本的な WebSearch の機能
- 特定のツールリストを指定した WebSearch
- 非同期の WebSearch 操作

## ベストプラクティス

1. **具体的にする**: よりよい結果のために明確な検索クエリを与える
2. **ソースを組み合わせる**: 包括的な調査のために WebSearch とローカルファイル分析を使う
3. **情報を検証する**: 重要な情報は複数の検索で相互参照する
4. **時間に敏感なデータ**: 時事、価格、最近の動向には WebSearch を使う
5. **ドキュメント**: 公式ドキュメントと最近の更新を検索する

## セキュリティ上の考慮事項

WebSearch が有効なとき、Claude は次のことができます。

- 一般公開されているあらゆる Web コンテンツにアクセスする
- 外部サイトへ HTTP/HTTPS リクエストを行う
- Web ページのコンテンツを処理・分析する

本番環境で WebSearch を有効にするときは、セキュリティ要件を考慮してください。

## トラブルシューティング

### WebSearch が動作しない

1. Claude SDK がインストールされているか確認する:
   ```bash
   pip install claude-code-sdk
   ```

2. WebSearch が有効か確認する:
   ```python
   adapter = ClaudeAdapter(verbose=True)
   adapter.configure(enable_web_search=True, enable_all_tools=True)
   ```

3. 単純なクエリでテストする:
   ```python
   response = adapter.execute("What is the current date?", enable_web_search=True)
   ```

### レート制限

WebSearch はレート制限の対象となることがあります。問題が起きた場合:

- 検索の間に遅延を入れる
- 関連するクエリをまとめる
- 適切な場合はキャッシュを使う

## 高度な設定

### WebSearch を含むカスタムツールリスト

```python
# WebSearch を含む特定のツールのみを有効にする
adapter.configure(
    allowed_tools=['WebSearch', 'Read', 'Write', 'Edit'],
    enable_web_search=True
)
```

### 条件付きの WebSearch

```python
# 特定のタスクにのみ WebSearch を有効にする
if task_requires_research:
    adapter.configure(enable_web_search=True)
else:
    adapter.configure(enable_web_search=False)
```

## Ralph ワークフローとの統合

WebSearch は、さまざまなワークフローで Ralph の能力を高めます。

1. **ドキュメント生成**: 調査して最新のドキュメントを作成する
2. **依存関係の更新**: 最新バージョンと移行ガイドを見つける
3. **バグ調査**: 既知の問題と解決策を検索する
4. **ベストプラクティス**: 現在の業界標準を調べる
5. **API 統合**: API のドキュメントと例を見つける

## パフォーマンスのヒント

- 適切な場合は検索結果をキャッシュする
- 関連する検索をまとめる
- より速い結果のために具体的な検索クエリを使う
- 包括的な分析のためにローカルツールと組み合わせる

## 今後の強化

WebSearch 連携について計画されている改善:

- 検索結果のキャッシュ
- カスタム検索プロバイダ
- 高度なフィルタリングオプション
- 検索履歴の追跡
- オフラインのフォールバックオプション
