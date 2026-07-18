<!-- 2026-01-28 -->
# Ralph Orchestrator

[![License](https://img.shields.io/badge/license-MIT-blue)](LICENSE)
[![Rust](https://img.shields.io/badge/rust-1.75+-orange)](https://www.rust-lang.org/)
[![Build](https://img.shields.io/github/actions/workflow/status/mikeyobrien/ralph-orchestrator/ci.yml?branch=main&label=CI)](https://github.com/mikeyobrien/ralph-orchestrator/actions)
[![Coverage](https://img.shields.io/endpoint?url=https://mikeyobrien.github.io/ralph-orchestrator/badges/coverage.json)](CONTRIBUTING.md#coverage)
[![Mentioned in Awesome Claude Code](https://awesome.re/mentioned-badge.svg)](https://github.com/hesreallyhim/awesome-claude-code)
[![Docs](https://img.shields.io/badge/docs-mkdocs-blue)](https://mikeyobrien.github.io/ralph-orchestrator/)
[![Discord](https://img.shields.io/discord/1482421188700667906?label=Discord&logo=discord&logoColor=white)](https://discord.gg/XWUyeUNffh)

タスクが完了するまで AI エージェントをループに保つ、ハットベースのオーケストレーション
フレームワーク。

> 「僕が英語で失敗する？そんなの不可能だい！」 - Ralph Wiggum

**[ドキュメント](https://mikeyobrien.github.io/ralph-orchestrator/)** | **[はじめに](https://mikeyobrien.github.io/ralph-orchestrator/getting-started/quick-start/)** | **[プリセット](https://mikeyobrien.github.io/ralph-orchestrator/guide/presets/)**

## インストール

### npm 経由（推奨）

```bash
npm install -g @ralph-orchestrator/ralph-cli
```

### GitHub Releases のインストーラ経由

```bash
curl --proto '=https' --tlsv1.2 -LsSf \
  https://github.com/mikeyobrien/ralph-orchestrator/releases/latest/download/ralph-cli-installer.sh | sh
```

### Cargo 経由

```bash
cargo install ralph-cli
```

> Homebrew は現在、このリポジトリの自動リリースフローからは公開されていません。npm、Cargo、
> または GitHub Releases のインストーラを優先してください。

## クイックスタート

```bash
# 1. お好みのバックエンドで Ralph を初期化する
ralph init --backend claude

# 2. 機能を計画する（対話的な PDD セッション）
ralph plan "Add user authentication with JWT"
# 生成: .ralph/specs/user-authentication/requirements.md, design.md, implementation-plan.md

# 3. 機能を実装する
ralph run -p "Implement the feature in .ralph/specs/user-authentication/"
```

Ralph は `LOOP_COMPLETE` を出力するか、イテレーション上限に達するまで反復します。

より単純なタスクでは、計画を省いて直接実行できます。

```bash
ralph run -p "Add input validation to the /users endpoint"
```

## Web ダッシュボード（アルファ版）

> **アルファ版:** Web ダッシュボードは活発に開発中です。粗い部分や破壊的変更が
> あり得ます。

<img width="1513" height="1128" alt="image" src="https://github.com/user-attachments/assets/ce5f072f-3d81-44d8-8f2f-88b42b33a3be" />

Ralph には、オーケストレーションループの監視と管理のための Web ダッシュボードが
含まれています。

```bash
ralph web                              # Rust RPC API + フロントエンドを起動しブラウザを開く
ralph web --no-open                    # ブラウザの自動起動をスキップ
ralph web --backend-port 4000          # RPC API のポートを指定
ralph web --frontend-port 8080         # フロントエンドのポートを指定
ralph web --legacy-node-api            # 非推奨の Node tRPC バックエンドを利用
```

### MCP サーバーのワークスペーススコープ

`ralph mcp serve` は、サーバーインスタンスごとに単一のワークスペースルートに限定されます。

```bash
ralph mcp serve --workspace-root /path/to/repo
```

優先順位は次のとおりです。

1. `--workspace-root`
2. `RALPH_API_WORKSPACE_ROOT`
3. 現在の作業ディレクトリ

複数リポジトリで使う場合は、リポジトリ/ワークスペースごとに MCP サーバーインスタンスを
1 つずつ実行します。Ralph の現在のコントロールプレーン API は、設定、タスク、ループ、
計画セッション、コレクションを単一のワークスペースルート配下に永続化するため、
ワークスペースごとにサーバーを 1 つ立てるのが確定的なモデルです。

**要件:**
- Rust ツールチェーン（`ralph-api` 用）
- Node.js >= 18 + npm（フロントエンド用）

初回実行時、`ralph web` は `node_modules` の欠落を自動検出し `npm install` を実行します。

Node.js をセットアップするには次のようにします。

```bash
# オプション 1: nvm（推奨）
nvm install    # .nvmrc を読み込む

# オプション 2: 直接インストール
# https://nodejs.org/
```

開発時:

```bash
npm install              # フロントエンド + レガシーバックエンドの依存をインストール
npm run dev:api          # Rust RPC API（ポート 3000）
npm run dev:web          # フロントエンド（ポート 5173）
npm run dev              # フロントエンドのみ（既定）
npm run dev:legacy-server  # 非推奨の Node バックエンド（任意）
npm run test             # フロントエンド/バックエンドの全ワークスペーステスト
```

## MCP サーバーモード

Ralph は、MCP 互換クライアント向けに stdio 上の MCP サーバーとして実行できます。

```bash
ralph mcp serve
```

このモードは、対話的なターミナルワークフローではなく、MCP クライアントの設定から使います。

## Ralph とは？

Ralph は [Ralph Wiggum テクニック](https://ghuntley.com/ralph/) を実装したものです。
継続的な反復による自律的なタスク完了を実現します。次をサポートします。

- **マルチバックエンド対応** — Claude Code、Kiro、Gemini CLI、Codex、Forge、Amp、
  Copilot CLI、OpenCode
- **ハットシステム** — イベントを通じて連携する専門ペルソナ
- **バックプレッシャー** — 不完全な作業を拒否するゲート（テスト、lint、型チェック）
- **メモリとタスク** — 永続的な学習とランタイムの作業追跡
- **5 つのサポート組み込み** — `code-assist`、`debug`、`research`、`review`、
  `pdd-to-code-assist`。さらに多くのパターンが例として文書化されています

## RObot（ヒューマンインザループ）

Ralph は、Telegram を通じてオーケストレーション中の人間との対話をサポートします。
エージェントは質問して回答があるまでブロックでき、人間はいつでも能動的なガイダンスを
送れます。

手早いオンボーディング（Telegram）:

```bash
ralph bot onboard --telegram   # ガイド付きセットアップ（トークン + chat id）
ralph bot status               # 設定を確認する
ralph bot test                 # テストメッセージを送信する
ralph run -c ralph.bot.yml -p  "Help the human"
```

```yaml
# ralph.yml
RObot:
  enabled: true
  telegram:
    bot_token: "your-token"  # または RALPH_TELEGRAM_BOT_TOKEN 環境変数
```

- **エージェントの質問** — エージェントは `human.interact` イベントを発行し、応答が届くか
  タイムアウトするまでループがブロックする
- **能動的なガイダンス** — いつでもメッセージを送ってループの途中でエージェントを誘導する
- **並列ループのルーティング** — メッセージは reply-to、`@loop-id` プレフィックス、または
  既定で主ループに振り分けられる
- **Telegram コマンド** — リアルタイムのループ可視化のための `/status`、`/tasks`、
  `/restart`

セットアップ手順は
[Telegram ガイド](https://mikeyobrien.github.io/ralph-orchestrator/guide/telegram/)
を参照してください。

## ドキュメント

完全なドキュメントは
**[mikeyobrien.github.io/ralph-orchestrator](https://mikeyobrien.github.io/ralph-orchestrator/)**
にあります。

- [インストール](https://mikeyobrien.github.io/ralph-orchestrator/getting-started/installation/)
- [クイックスタート](https://mikeyobrien.github.io/ralph-orchestrator/getting-started/quick-start/)
- [設定](https://mikeyobrien.github.io/ralph-orchestrator/guide/configuration/)
- [CLI リファレンス](https://mikeyobrien.github.io/ralph-orchestrator/guide/cli-reference/)
- [プリセット](https://mikeyobrien.github.io/ralph-orchestrator/guide/presets/)
- [概念: ハットとイベント](https://mikeyobrien.github.io/ralph-orchestrator/concepts/hats-and-events/)
- [アーキテクチャ](https://mikeyobrien.github.io/ralph-orchestrator/advanced/architecture/)


## FAQ

### 一般

**Ralph Orchestrator とは？**
Ralph は Ralph Wiggum テクニックを実装したハットベースのオーケストレーション
フレームワークです。継続的な反復による自律的なタスク完了を実現します。タスクが完了する
まで AI エージェントをループに保ち、Claude Code、Gemini CLI、Codex などの複数の
バックエンドをサポートします。

**Ralph は他の AI コーディングツールとどう違うのか？**
一回限りの AI アシスタントとは異なり、Ralph は専門ペルソナを持つ「ハットシステム」を
使って完了まで反復します。不完全な作業を拒否するバックプレッシャーゲート（テスト、lint、
型チェック）に加え、継続的な学習のための永続的なメモリとタスクを備えています。

### インストールとセットアップ

**システム要件は？**
- Rust 1.75 以降（`ralph-api` コンポーネント用）
- Node.js >= 18 + npm（Web ダッシュボードのフロントエンド用）
- AI コーディングアシスタントの CLI（Claude Code、Codex、Gemini CLI など）

**どのインストール方法を使うべきか？**
- **npm**（ほとんどのユーザーに推奨）: `npm install -g @ralph-orchestrator/ralph-cli`
- **Cargo**: `cargo install ralph-cli`（Rust 開発者に最適）
- **GitHub Releases インストーラ**: `curl ... | sh` によるワンリンクインストール

**Homebrew はサポートされているか？**
Homebrew は現在、このリポジトリの自動リリースフローからは公開されていません。npm、Cargo、
または GitHub Releases のインストーラを優先してください。

### 使い方

**Ralph で新しいプロジェクトを始めるには？**
```bash
ralph init --backend claude
ralph plan "Add user authentication with JWT"
ralph run -p "Implement the feature in .ralph/specs/user-authentication/"
```

**Ralph はどのバックエンドをサポートしているか？**
Claude Code、Kiro、Gemini CLI、Codex、Forge、Amp、Copilot CLI、OpenCode。

**「ハットシステム」とは？**
Ralph は、イベントを通じて連携する専門ペルソナ（ハット）を使います。各ハットは特定の
役割 — code-assist、debug、research、review、pdd-to-code-assist — を持ち、構造化された
多段階のタスク実行を可能にします。

### RObot（ヒューマンインザループ）

**RObot とは？**
RObot は、Telegram を通じてオーケストレーション中の人間との対話を可能にします。
エージェントは質問して回答があるまでブロックでき、人間はループの途中で能動的なガイダンスを
送れます。

**Telegram 連携をセットアップするには？**
```bash
ralph bot onboard --telegram   # ガイド付きセットアップ
ralph bot status               # 設定を確認する
ralph bot test                 # テストメッセージを送信する
```

### Web ダッシュボード

**Web ダッシュボードにアクセスするには？**
`ralph web` を実行して Rust RPC API + フロントエンドを起動し、ブラウザを開きます。
ダッシュボードは現在アルファ版です。粗い部分や破壊的変更があり得ます。

**ダッシュボードのポートをカスタマイズできるか？**
はい: `ralph web --backend-port 4000 --frontend-port 8080`

### MCP サーバー

**Ralph を MCP サーバーとして実行するには？**
```bash
ralph mcp serve --workspace-root /path/to/repo
```
各 MCP サーバーインスタンスは単一のワークスペースルートに限定されます。複数リポジトリで
使う場合は、ワークスペースごとにインスタンスを 1 つずつ実行します。

### トラブルシューティング

**「node_modules not found」で Ralph が起動しない**
プロジェクトディレクトリで `npm install` を実行するか、初回実行時に `ralph web` に
自動検出・インストールさせてください。

**Node.js が未インストールの場合のセットアップは？**
nvm（推奨）を使う: `nvm install`（`.nvmrc` を読み込む）、または https://nodejs.org/
から直接インストールします。

**どこで助けを得られるか？**
- [Discord サーバー](https://discord.gg/XWUyeUNffh) に参加する
- [Issue トラッカー](https://github.com/mikeyobrien/ralph-orchestrator/issues)
  でバグを報告する
- [mikeyobrien.github.io/ralph-orchestrator](https://mikeyobrien.github.io/ralph-orchestrator/)
  で完全なドキュメントを読む

## コントリビューション

コントリビューションを歓迎します！ガイドラインは [CONTRIBUTING.md](CONTRIBUTING.md) を、
コミュニティ基準は [CODE_OF_CONDUCT.md](CODE_OF_CONDUCT.md) を参照してください。

## ライセンス

MIT ライセンス — 詳細は [LICENSE](LICENSE) を参照してください。

## 💬 コミュニティとサポート

**ralph-orchestrator** コミュニティに参加して、AI エージェントのパターンを議論したり、
実装の助けを得たり、ロードマップに貢献したりしてください。

* **Discord**: [サーバーに参加](https://discord.gg/XWUyeUNffh) して、メンテナや他の
  ユーザーとリアルタイムで話しましょう。
* **GitHub Issues**: バグ報告や正式な機能要望には、
  [Issue トラッカー](https://github.com/mikeyobrien/ralph-orchestrator/issues)
  を使ってください。

## 謝辞

- **[Geoffrey Huntley](https://ghuntley.com/ralph/)** — Ralph Wiggum テクニックの考案者
- **[Strands Agents SOP](https://github.com/strands-agents/agent-sop)** — エージェント
  SOP フレームワーク
- **[ratatui](https://ratatui.rs/)** — ターミナル UI フレームワーク

---

*「僕、勉強してるんだい！」 - Ralph Wiggum*
