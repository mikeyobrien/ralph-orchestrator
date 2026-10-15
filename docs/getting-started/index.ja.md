# はじめに

Ralph Orchestrator へようこそ！このセクションは、素早く使い始めるのに役立ちます。

## 学べること

1. **[インストール](installation.ja.md)** — Ralph とその前提条件をインストールする
2. **[クイックスタート](quick-start.ja.md)** — 最初の Ralph オーケストレーションを実行する
3. **[最初のタスク](first-task.ja.md)** — 実際のタスクを作成・設定する

## 前提条件

始める前に、次を用意していることを確認してください。

- **Rust 1.75 以降**（ソースからビルドする場合）
- **少なくとも 1 つの AI CLI ツール**をインストールしていること:
    - [Claude Code](https://github.com/anthropics/claude-code)（推奨）
    - [Kiro](https://kiro.dev/)
    - [Gemini CLI](https://github.com/google-gemini/gemini-cli)
    - [Codex](https://github.com/openai/codex)
    - [Forge](https://github.com/tailcallhq/forgecode)
    - [Amp](https://github.com/sourcegraph/amp)
    - [Copilot CLI](https://docs.github.com/copilot)
    - [OpenCode](https://opencode.ai/)

## 手早いインストール

=== "npm（推奨）"

    ```bash
    npm install -g @ralph-orchestrator/ralph-cli
    ```

=== "GitHub Releases インストーラ"

    ```bash
    curl --proto '=https' --tlsv1.2 -LsSf \
      https://github.com/mikeyobrien/ralph-orchestrator/releases/latest/download/ralph-cli-installer.sh | sh
    ```

=== "Cargo"

    ```bash
    cargo install ralph-cli
    ```

## インストールの確認

```bash
ralph --version
ralph --help
```

## 次のステップ

インストールが済んだら、[クイックスタート](quick-start.ja.md) ガイドに進んで最初の
オーケストレーションを実行してください。
