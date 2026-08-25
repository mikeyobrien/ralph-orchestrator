# Ralph Orchestrator

<div align="center" markdown>

**タスクが完了するまで AI エージェントをループに保つ、ハットベースのオーケストレーション
フレームワーク。**

[![License](https://img.shields.io/badge/license-MIT-blue)](https://github.com/mikeyobrien/ralph-orchestrator/blob/main/LICENSE)
[![Rust](https://img.shields.io/badge/rust-1.75+-orange)](https://www.rust-lang.org/)
[![Build](https://img.shields.io/github/actions/workflow/status/mikeyobrien/ralph-orchestrator/ci.yml?branch=main&label=CI)](https://github.com/mikeyobrien/ralph-orchestrator/actions)

> 「僕が英語で失敗する？そんなの不可能だい！」 - Ralph Wiggum

</div>

---

## Ralph とは？

Ralph は [Ralph Wiggum テクニック](https://ghuntley.com/ralph/) を実装したものです。
継続的な反復による自律的なタスク完了を実現します。Ralph にタスクを与えれば、完了するまで
働き続けます。

> 「オーケストレーターは薄い調整レイヤーであり、プラットフォームではない。Ralph は賢い。
> 仕事は Ralph にさせよう。」

### 2 つの動作モード

| モード | 説明 | 最適な用途 |
|------|-------------|----------|
| **従来型** | 単純なループ — Ralph は完了まで反復する | 手早いタスク、単純な自動化 |
| **ハットベース** | 専門ペルソナがイベントを通じて連携する | 複雑なワークフロー、多段階のプロセス |

## 主な機能

<div class="grid cards" markdown>

-   :material-robot: **マルチバックエンド対応**

    Claude Code、Kiro、Gemini CLI、Codex、Forge、Amp、Copilot CLI、OpenCode で動作

-   :material-hat-fedora: **ハットシステム**

    型付きイベントを通じて連携する、異なる挙動を持つ専門の Ralph ペルソナ

-   :material-shield-check: **バックプレッシャーの強制**

    不完全な作業を拒否するゲート — テスト、lint、型チェックが通らなければならない

-   :material-brain: **メモリとタスク**

    セッションをまたいだ永続的な学習と、ランタイムの作業追跡

-   :material-monitor: **インタラクティブな TUI**

    Ralph の活動を監視するリアルタイムのターミナル UI

-   :material-cog: **31 のプリセット**

    サポートされる少数の組み込みワークフローと、文書化された例のより大きなカタログ

</div>

## 手早い例

```bash
# 従来型モードで初期化する
ralph init --backend claude

# タスクを作成する
cat > PROMPT.md << 'EOF'
Build a REST API with these endpoints:
- POST /users - Create user
- GET /users/:id - Get user by ID
- PUT /users/:id - Update user

Use Express.js with TypeScript.
EOF

# Ralph を実行する
ralph run
```

Ralph は `LOOP_COMPLETE` を出力するか、イテレーション上限に達するまで反復します。

## Ralph の信条

1. **フレッシュコンテキストは信頼性** — 各イテレーションはコンテキストをクリアする。毎回
   スペック、計画、コードを読み直す。
2. **規定よりバックプレッシャー** — どうやるかを規定しない。悪い作業を拒否するゲートを作る。
3. **計画は使い捨て** — 再生成のコストは計画ループ 1 回分。安い。
4. **ディスクは状態、Git は記憶** — ファイルが引き継ぎの仕組みである。
5. **スクリプトではなく信号で舵を取る** — スクリプトではなく、標識を加える。
6. **Ralph に Ralph させる** — ループの*中*ではなく、ループの*上*に座る。

## はじめに

<div class="grid cards" markdown>

-   :material-download: **[インストール](getting-started/installation.ja.md)**

    npm、GitHub Releases インストーラ、または Cargo で Ralph をインストールする

-   :material-rocket-launch: **[クイックスタート](getting-started/quick-start.ja.md)**

    5 分で使い始める

-   :material-book-open: **[概念](concepts/index.ja.md)**

    ハット、イベント、メモリ、バックプレッシャーを理解する

-   :material-cog: **[設定](guide/configuration.ja.md)**

    自分のワークフロー向けに Ralph を設定する

</div>

## アーキテクチャ

Ralph は、7 つのクレートを持つ Cargo ワークスペースとして構成されています。

| クレート | 用途 |
|-------|---------|
| `ralph-proto` | プロトコル型: Event、Hat、Topic |
| `ralph-core` | ビジネスロジック: EventLoop、Config |
| `ralph-adapters` | CLI バックエンドの統合 |
| `ralph-tui` | ratatui によるターミナル UI |
| `ralph-cli` | バイナリのエントリポイント |
| `ralph-e2e` | エンドツーエンドのテスト |
| `ralph-bench` | ベンチマーク |

## コミュニティ

- [GitHub Issues](https://github.com/mikeyobrien/ralph-orchestrator/issues) — バグ報告と機能要望
- [GitHub Discussions](https://github.com/mikeyobrien/ralph-orchestrator/discussions) — 質問とアイデアの共有
- [コントリビューションガイド](contributing/index.ja.md) — Ralph の改善に協力する

## ライセンス

Ralph Orchestrator は、[MIT ライセンス](https://github.com/mikeyobrien/ralph-orchestrator/blob/main/LICENSE)
の下でライセンスされたオープンソースソフトウェアです。

---

<div align="center" markdown>

*「僕、勉強してるんだい！」 - Ralph Wiggum*

</div>
