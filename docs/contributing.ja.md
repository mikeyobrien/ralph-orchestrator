# Ralph Orchestrator へのコントリビューション

Ralph Orchestrator へのコントリビューションにご関心をお寄せいただきありがとうございます！
このガイドは、プロジェクトへの貢献を始めるのに役立ちます。

## 行動規範

このプロジェクトに参加することで、あなたは私たちの
[行動規範](https://github.com/mikeyobrien/ralph-orchestrator/blob/main/CODE_OF_CONDUCT.md)
を守ることに同意します。コントリビュートする前に読んでください。

## コントリビューションの方法

### 1. バグを報告する

バグを見つけましたか？修正に協力してください。

1. 重複を避けるため**既存の issue を確認する**
2. 次を含む**新しい issue を作成する**:
   - 明確なタイトルと説明
   - 再現手順
   - 期待される挙動と実際の挙動
   - システム情報
   - エラーメッセージ/ログ

**バグ報告のテンプレート:**
```markdown
## Description
Brief description of the bug

## Steps to Reproduce
1. Run command: `python ralph_orchestrator.py ...`
2. See error

## Expected Behavior
What should happen

## Actual Behavior
What actually happens

## Environment
- OS: [e.g., Ubuntu 22.04]
- Python: [e.g., 3.10.5]
- Ralph Version: [e.g., 1.0.0]
- AI Agent: [e.g., claude]

## Logs
```
Error messages here
```
```

### 2. 機能を提案する

アイデアがありますか？ぜひ聞かせてください。

1. **既存の機能要望を確認する**
2. 大きな変更については**ディスカッションを開く**
3. 次を含む**機能要望を作成する**:
   - ユースケースの説明
   - 提案する解決策
   - 代替アプローチ
   - 実装上の考慮事項

### 3. ドキュメントを改善する

ドキュメントの改善はいつでも歓迎です。

- 誤字や文法を直す
- 分かりにくいセクションを明確にする
- 欠けている情報を追加する
- 新しい例を作成する
- ドキュメントを翻訳する

### 4. コードをコントリビュートする

コーディングの準備はできましたか？次の手順に従ってください。

#### 開発環境のセットアップ

```bash
# リポジトリをフォークしてクローンする
git clone https://github.com/YOUR_USERNAME/ralph-orchestrator.git
cd ralph-orchestrator

# 仮想環境を作成する
python -m venv venv
source venv/bin/activate  # Windows では: venv\Scripts\activate

# 開発用の依存をインストールする
pip install -e .
pip install pytest pytest-cov black ruff

# pre-commit フックをインストールする（任意）
pip install pre-commit
pre-commit install
```

#### 開発ワークフロー

1. **ブランチを作成する**
   ```bash
   git checkout -b feature/your-feature-name
   # または
   git checkout -b fix/issue-number
   ```

2. **変更する**
   - 既存のコードスタイルに従う
   - テストを追加/更新する
   - ドキュメントを更新する

3. **変更をテストする**
   ```bash
   # すべてのテストを実行する
   pytest

   # 特定のテストを実行する
   pytest test_orchestrator.py::test_function

   # カバレッジを確認する
   pytest --cov=ralph_orchestrator --cov-report=html
   ```

4. **コードを整形する**
   ```bash
   # black で整形する
   black ralph_orchestrator.py

   # ruff で lint する
   ruff check ralph_orchestrator.py
   ```

5. **変更をコミットする**
   ```bash
   git add .
   git commit -m "feat: add new feature"
   # Conventional Commits を使う: feat, fix, docs, test, refactor, style, chore
   ```

6. **プッシュして PR を作成する**
   ```bash
   git push origin feature/your-feature-name
   ```

## 開発ガイドライン

### コードスタイル

私たちは、次の好みとともに PEP 8 に従います。

- **行の長さ**: 88 文字（Black の既定）
- **クォート**: 文字列にはダブルクォート
- **インポート**: `isort` でソート
- **型ヒント**: 有益な場所で使う
- **docstring**: Google スタイル

**例:**
```python
def calculate_cost(
    input_tokens: int,
    output_tokens: int,
    agent_type: str = "claude"
) -> float:
    """
    Calculate token usage cost.
    
    Args:
        input_tokens: Number of input tokens
        output_tokens: Number of output tokens
        agent_type: Type of AI agent
        
    Returns:
        Cost in USD
        
    Raises:
        ValueError: If agent_type is unknown
    """
    if agent_type not in TOKEN_COSTS:
        raise ValueError(f"Unknown agent: {agent_type}")
    
    rates = TOKEN_COSTS[agent_type]
    cost = (input_tokens * rates["input"] + 
            output_tokens * rates["output"]) / 1_000_000
    return round(cost, 4)
```

### テストのガイドライン

すべての新機能にはテストが必要です。

1. 個々の関数の**ユニットテスト**
2. ワークフローの**統合テスト**
3. **エッジケース**とエラー条件
4. テストの目的の**ドキュメント**

**テストの例:**
```python
def test_calculate_cost():
    """Test cost calculation for different agents."""
    # Test Claude pricing
    cost = calculate_cost(1000, 500, "claude")
    assert cost == 0.0105
    
    # Test invalid agent
    with pytest.raises(ValueError):
        calculate_cost(1000, 500, "invalid")
    
    # Test edge case: zero tokens
    cost = calculate_cost(0, 0, "claude")
    assert cost == 0.0
```

### コミットメッセージの慣習

私たちは [Conventional Commits](https://www.conventionalcommits.org/) を使います。

- `feat:` 新機能
- `fix:` バグ修正
- `docs:` ドキュメントの変更
- `test:` テストの追加/変更
- `refactor:` コードのリファクタリング
- `style:` コードスタイルの変更
- `chore:` 保守タスク
- `perf:` パフォーマンスの改善

**例:**
```bash
feat: add Gemini agent support
fix: resolve token overflow in long prompts
docs: update installation guide for Windows
test: add integration tests for checkpointing
refactor: extract prompt validation logic
```

### プルリクエストのプロセス

1. **タイトル**: Conventional Commits の形式を使う
2. **説明**: 何を、なぜかを説明する
3. **テスト**: 実施したテストを記述する
4. **スクリーンショット**: UI 変更があれば含める
5. **チェックリスト**: PR テンプレートを完成させる

**PR テンプレート:**
```markdown
## Description
Brief description of changes

## Type of Change
- [ ] Bug fix
- [ ] New feature
- [ ] Documentation update
- [ ] Performance improvement

## Testing
- [ ] All tests pass
- [ ] Added new tests
- [ ] Manual testing performed

## Checklist
- [ ] Code follows style guidelines
- [ ] Self-reviewed code
- [ ] Updated documentation
- [ ] No breaking changes
```

## プロジェクト構成

```
ralph-orchestrator/
├── ralph_orchestrator.py   # メインのオーケストレーター
├── ralph                   # CLI ラッパー
├── tests/                  # テストファイル
│   ├── test_orchestrator.py
│   ├── test_integration.py
│   └── test_production.py
├── docs/                   # ドキュメント
│   ├── index.md
│   ├── guide/
│   └── api/
├── examples/               # プロンプトの例
├── .agent/                 # ランタイムデータ
└── .github/               # GitHub 設定
```

## テスト

### テストの実行

```bash
# すべてのテスト
pytest

# カバレッジ付き
pytest --cov=ralph_orchestrator

# 特定のテストファイル
pytest test_orchestrator.py

# 詳細な出力
pytest -v

# 最初の失敗で停止する
pytest -x
```

### テストのカテゴリ

1. **ユニットテスト**: 個々の関数をテストする
2. **統合テスト**: コンポーネントの相互作用をテストする
3. **E2E テスト**: 完全なワークフローをテストする
4. **パフォーマンステスト**: リソース使用をテストする
5. **セキュリティテスト**: 入力検証をテストする

## ドキュメント

### ローカルでのドキュメントのビルド

```bash
# MkDocs をインストールする
pip install mkdocs mkdocs-material

# ローカルで配信する
mkdocs serve

# 静的サイトをビルドする
mkdocs build
```

### ドキュメントの基準

- 明確で簡潔な言葉
- すべての機能にコード例
- 単なる「どうやるか」ではなく「なぜか」を説明する
- 例を最新に保つ
- トラブルシューティングのヒントを含める

## リリースのプロセス

1. **バージョン更新**: コード内のバージョンを更新する
2. **変更履歴**: CHANGELOG.md を更新する
3. **テスト**: すべてのテストが通ることを確認する
4. **ドキュメント**: 必要なら更新する
5. **タグ**: バージョンタグを作成する
6. **リリース**: GitHub リリースを作成する

## 助けを得る

### コントリビューター向け

- 💬 [Discord サーバー](https://discord.gg/ralph-orchestrator)
- 📧 [メンテナにメール](mailto:maintainers@ralph-orchestrator.dev)
- 🗣️ [GitHub Discussions](https://github.com/mikeyobrien/ralph-orchestrator/discussions)

### リソース

- [開発環境セットアップ動画](https://youtube.com/...)
- [アーキテクチャの概要](advanced/architecture.ja.md)
- [API ドキュメント](api/orchestrator.ja.md)
- [テストガイド](contributing/testing.ja.md)

## 表彰

コントリビューターは、次で表彰されます。

- [CONTRIBUTORS.md](https://github.com/mikeyobrien/ralph-orchestrator/blob/main/CONTRIBUTORS.md)
- リリースノート
- ドキュメントのクレジット

## ライセンス

コントリビュートすることで、あなたの貢献が MIT ライセンスの下でライセンスされることに
同意したものとみなされます。

---

Ralph Orchestrator へのコントリビューション、ありがとうございます！ 🎉
