# Ralph へのコントリビューション

Ralph Orchestrator へのコントリビューションを歓迎します！

## このセクションの内容

| ガイド | 説明 |
|-------|-------------|
| [開発環境のセットアップ](setup.ja.md) | 開発環境をセットアップする |
| [コードスタイル](style.ja.md) | コーディングの基準と慣習 |
| [テスト](testing.ja.md) | テストの作成と実行 |
| [PR の提出](pull-requests.ja.md) | プルリクエストのプロセス |

## クイックスタート

```bash
# リポジトリをクローンする
git clone https://github.com/mikeyobrien/ralph-orchestrator.git
cd ralph-orchestrator

# ビルドする
cargo build

# テストを実行する
cargo test

# git フックをインストールする
./scripts/setup-hooks.sh
```

## コントリビューションの方法

### バグを報告する

バグを見つけましたか？次を含めて
[issue を開いて](https://github.com/mikeyobrien/ralph-orchestrator/issues/new)
ください。

- 問題の説明
- 再現手順
- 期待される挙動と実際の挙動
- 使用した Ralph のバージョンとバックエンド

### 機能を提案する

アイデアがありますか？まず
[ディスカッションを始めて](https://github.com/mikeyobrien/ralph-orchestrator/discussions/new)
次を行ってください。

- ユースケースを説明する
- 潜在的なアプローチを議論する
- 実装の前にフィードバックを得る

### コードを提出する

1. リポジトリをフォークする
2. 機能ブランチを作成する
3. テスト付きでコードを書く
4. すべてのテストが通ることを確認する
5. プルリクエストを提出する

### ドキュメントを改善する

ドキュメントの改善はいつでも歓迎です。

- 誤字や不明瞭な説明を直す
- 例を追加する
- 古い情報を更新する
- 他の言語に翻訳する

## 開発の哲学

Ralph は [6 つの信条](../concepts/tenets.ja.md) に従います。

1. **フレッシュコンテキストは信頼性**
2. **規定よりバックプレッシャー**
3. **計画は使い捨て**
4. **ディスクは状態、Git は記憶**
5. **スクリプトではなく信号で舵を取る**
6. **Ralph に Ralph させる**

コントリビューションは、これらの原則に沿うべきです。

## 避けるべきアンチパターン

Ralph の哲学より:

- エージェントが処理できる機能をオーケストレーターに組み込む
- 複雑なリトライロジック（フレッシュコンテキストが復旧を担う）
- 詳細な逐次手順の指示（代わりにバックプレッシャーを使う）
- タスク選択時に作業のスコープを決める（計画作成時にスコープを決める）
- コードで確認せずに機能が欠けていると仮定する

## 行動規範

敬意を持ち、建設的に。私たちは皆、Ralph をよりよくするためにここにいます。

## 助けを得る

- [GitHub Discussions](https://github.com/mikeyobrien/ralph-orchestrator/discussions)
- [Issue トラッカー](https://github.com/mikeyobrien/ralph-orchestrator/issues)
