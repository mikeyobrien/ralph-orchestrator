# 変更履歴

Ralph Orchestrator への注目すべき変更は、すべてこのファイルに記録されます。

形式は [Keep a Changelog](https://keepachangelog.com/en/1.0.0/) に基づいており、この
プロジェクトは [セマンティックバージョニング](https://semver.org/spec/v2.0.0.html) に
従います。

## [Unreleased]

## [2.1.0] - 2026-01-20

### 追加

- **TUI イテレーションアーキテクチャ**: TUI を、スナップショットテストを伴うイテレーション
  ベースのモデルにリファクタリング
  - 各イテレーションが、明確な分離のために独自のバッファを持つ
  - 以前のイテレーションを見直すためのイテレーション切替（←/→ 矢印）
  - TUI コンポーネントのスナップショットベースのテスト

### 修正

- **TUI のコンテンツ表示**: 単語の途中でコンテンツを切っていた省略記号の切り詰めを削除
  - 長い行は、"..." で切り詰められる代わりにビューポート境界でソフトラップされるように
- **TUI の自動スクロール**: 最新の出力が見えるようにコンテンツが自動スクロールするように
- **TUI のアーティファクト**: イテレーション切替時の視覚的アーティファクトを防ぐため、
  ビューポートバッファのクリアを修正
- **Markdown の境界**: markdown コンテンツの描画時に行の境界を保持
- **バックエンドのサポート**: `ralph init` の有効なバックエンドに `opencode` と `copilot`
  を追加（#75, #77）
- **割り込み処理**: 割り込み後もプロセスが実行を続ける Ctrl+C の競合状態を修正（#76）

## [2.0.0] - 2026-01-14

### 追加

- **Hatless Ralph アーキテクチャ**: Ralph は今では、任意のハットを伴う恒常的な
  コーディネーターである
  - ソロモード: Ralph がすべてのイベントを処理する（ハット未設定）
  - マルチハットモード: Ralph が複数の専門ハットをオーケストレーションする
  - ハットごとのバックエンド設定（各ハットが異なる AI エージェントを使える）
  - 既定の公開: ハットがフォールバックイベントを指定できる
- **JSONL イベント形式**: イベントは出力の XML ではなく
  `.ralph/events-YYYYMMDD-HHMMSS.jsonl` に書き込まれる
  - 各実行が、分離のために一意のタイムスタンプ付きイベントファイルを作成する
  - 構造化されたイベント形式: `{"topic":"...", "payload":"...", "ts":"..."}`
  - 安全なイベント発行のための `ralph emit` コマンド
  - 最後の読み込み以降の新しいイベントを読む EventReader
  - 既存のイベントトピックと後方互換
- **インタラクティブ TUI モード**: エージェントとの対話のためのフルスクリーンターミナル UI
  - PTY 統合を伴う埋め込みターミナルウィジェット
  - プレフィックスコマンド（Ctrl+a）: quit, help, pause, skip, abort
  - ナビゲーション付きのスクロールモード（j/k/矢印/Page Up/Down/g/G）
  - スクロールモードでの検索（前方/後方に / と ?、次/前に n/N）
  - イテレーション境界の処理（イテレーション間で画面がクリアされる）
  - ralph.yml で設定可能なプレフィックスキー
- **モックバックエンドのテスト**: スクリプト化された応答による決定的な E2E テスト
  - 実際の AI エージェントなしでテストするための MockBackend
  - テストケースのためのシナリオ YAML 形式
  - ソロモード、マルチハット、孤立イベント、default_publishes、混合バックエンドを
    カバーする 5 つのシナリオテスト

### 変更

- **破壊的**: 既定のハットなし - 空の設定 = ソロ Ralph モード
- **破壊的**: すべてのプリセットから Planner ハットを削除
- **破壊的**: イベントは `.ralph/` ディレクトリに書き込まねばならない（XML 形式は非推奨）
- **破壊的**: HatRegistry は既定の planner/builder ハットを作成しなくなった
- CLI フラグ `--tui` が視覚的観察のための TUI モードを起動する
- TUI モードは、実行制御（pause, skip, abort）を削除しつつ、スクロールと検索のナビ
  ゲーションを提供する
- HatConfig は `backend` と `default_publishes` フィールドを含むようになった
- InstructionBuilder は新しいプロンプト形式のために `build_hatless_ralph()` を追加
- EventLoop は EventParser の代わりに EventReader を使う

### 削除

- **破壊的**: XML イベント形式はサポートされなくなった
- **破壊的**: 自動の planner/builder ハット作成

### 移行

v1.x からのアップグレードは [移行ガイド](../migration/v2-hatless-ralph.ja.md) を参照
してください。

## [1.2.3] - 2026-01-12

### 変更

- ドキュメントとバージョンメタデータの更新

## [1.2.2] - 2026-01-08

### 追加

- **Kiro CLI 統合**: Q Chat CLI サポートの後継
  - `kiro-cli chat` コマンドの完全サポート
  - Kiro が見つからない場合のレガシー `q` コマンドへの自動フォールバック
  - `kiro` アダプタ設定で設定可能
  - 新しいブランドで Q Chat のすべての機能を保持
- **完了マーカーの検出**: タスクは、プロンプトファイルの `- [x] TASK_COMPLETE`
  チェックボックスマーカーで完了を知らせられるように
  - オーケストレーターが各イテレーションの前にマーカーを確認する
  - マーカーが見つかると直ちにループを終了する
  - `- [x] TASK_COMPLETE` と `[x] TASK_COMPLETE` の両形式をサポート
- **ループ検出**: rapidfuzz を使った反復的なエージェント出力の自動検出
  - 現在の出力を最後の 5 つの出力と比較する
  - ループを検出するために 90% の類似度しきい値を使う
  - 暴走エージェントによる無限ループを防ぐ
- 新しい依存: 高速なファジー文字列マッチのための `rapidfuzz>=3.0.0,<4.0.0`
- MkDocs による静的ドキュメントサイト
- 包括的な API リファレンスドキュメント
- 追加のシナリオ例
- パフォーマンス監視ツール

### 変更

- エージェント実行のエラー処理を改善
- チェックポイント作成ロジックを強化
- `SafetyGuard.reset()` がループ検出履歴もクリアするように

### 修正

- 状態ファイル更新の競合状態
- 長時間実行セッションでのメモリリーク

## [1.2.0] - 2025-12

### 追加

- **ACP（Agent Client Protocol）サポート**: ACP 準拠エージェントとの完全な統合
  - JSON-RPC 2.0 メッセージプロトコルの実装
  - 4 モードの権限処理: `auto_approve`, `deny_all`, `allowlist`, `interactive`
  - セキュリティ検証を伴うファイル操作（`fs/read_text_file`, `fs/write_text_file`）
  - ターミナル操作（`terminal/create`, `terminal/output`, `terminal/wait_for_exit`,
    `terminal/kill`, `terminal/release`）
  - セッション管理とストリーミング更新
  - イテレーションをまたいだコンテキスト永続化のためのエージェントスクラッチパッドの仕組み
- 新しい CLI オプション: `--acp-agent`, `--acp-permission-mode`
- `ralph.yml` の `adapters.acp` 配下での ACP 設定サポート
- 環境変数の上書き: `RALPH_ACP_AGENT`, `RALPH_ACP_PERMISSION_MODE`, `RALPH_ACP_TIMEOUT`
- 305 個以上の新しい ACP 固有のテスト

### 変更

- テストスイートを 920 個以上に拡大
- ACP サポートのためにドキュメントを更新

## [1.1.0] - 2025-12

### 追加

- ノンブロッキング操作のための非同期優先アーキテクチャ
- ローテーションとセキュリティマスキングを伴うスレッドセーフな非同期ロギング
- シンタックスハイライト付きのリッチなターミナル出力
- インラインプロンプトのサポート（`-p "your task"`）
- MCP サーバーサポートを伴う Claude Agent SDK の統合
- 非同期の git チェックポイント（ノンブロッキング）
- パストラバーサル保護を伴うセキュリティ検証システム
- ログでの機微データのマスキング（API キー、トークン、パスワード）
- RLock を伴うスレッドセーフな設定
- セッションメトリクスと再入保護を伴う VerboseLogger
- メモリ効率の良い保存を伴うイテレーション統計の追跡

### 変更

- テストスイートを 620 個以上に拡大
- ClaudeErrorFormatter によるエラー処理の改善
- サブプロセス優先のクリーンアップを伴うシグナル処理の強化

### 修正

- カウントダウンのプログレスバーでのゼロ除算
- QChatAdapter でのプロセス参照リーク
- 非同期関数でのブロッキングファイル I/O
- エラーハンドラでの例外連鎖

## [1.0.3] - 2025-09-07

### 追加

- 本番デプロイガイド
- Dockerfile と docker-compose.yml による Docker サポート
- Kubernetes デプロイマニフェスト
- 監視用のヘルスチェックエンドポイント

### 変更

- リソース上限の処理を改善
- 構造化された JSON 出力によるロギングの強化
- 依存を最新バージョンに更新

### 修正

- Windows での Git チェックポイント作成
- エッジケースでのエージェントタイムアウト処理

## [1.0.2] - 2025-09-07

### 追加

- Q Chat 統合の改善
- リアルタイムのメトリクス収集
- 対話的な CLI モード
- Bash と ZSH の補完スクリプト

### 変更

- 拡張性向上のためにエージェントマネージャをリファクタリング
- コンテキストウィンドウ管理の改善
- 進捗報告の強化

### 修正

- プロンプトファイルでの Unicode 処理
- 中断をまたいだ状態の永続化

## [1.0.1] - 2025-09-07

### 追加

- Gemini CLI 統合
- 高度なコンテキスト管理戦略
- コスト追跡と見積もり
- HTML レポート生成

### 変更

- イテレーションパフォーマンスを最適化
- エラー回復の仕組みを改善
- Git 操作の強化

### 修正

- macOS でのエージェント検出
- 特殊文字を伴うプロンプトのアーカイブ
- チェックポイント間隔の計算

## [1.0.0] - 2025-09-07

### 追加

- 中核機能を伴う初回リリース
- Claude CLI 統合
- Q Chat 統合
- Git ベースのチェックポイント
- プロンプトのアーカイブ
- 状態の永続化
- 包括的なテストスイート
- CLI ラッパースクリプト
- 設定管理
- メトリクス収集

### 機能

- 利用可能な AI エージェントの自動検出
- 設定可能なイテレーションと実行時間の上限
- 指数バックオフを伴うエラー回復
- 詳細モードとドライランモード
- JSON 設定ファイルのサポート
- 環境変数による設定

### ドキュメント

- 例を伴う完全な README
- インストール手順
- 使い方ガイド
- API ドキュメント
- コントリビューションのガイドライン

## [0.9.0] - 2025-09-06（ベータ）

### 追加

- テスト用のベータリリース
- 基本的なオーケストレーションループ
- Claude 統合
- 単純なチェックポイント

### 既知の問題

- 限られたエラー処理
- メトリクス収集なし
- 単一エージェントのサポートのみ

## [0.5.0] - 2025-09-05（アルファ）

### 追加

- 初回のアルファリリース
- 概念実証の実装
- 基本的な Ralph ループ
- 手動テストのみ

---

## バージョン履歴の要約

### メジャーバージョン

- **1.0.0** - 完全な機能セットを備えた最初の安定版リリース
- **0.9.0** - コミュニティテスト用のベータリリース
- **0.5.0** - アルファの概念実証

### バージョニングのポリシー

私たちはセマンティックバージョニング（SemVer）を使います。

- **MAJOR** バージョン: 互換性のない API 変更
- **MINOR** バージョン: 後方互換の機能追加
- **PATCH** バージョン: 後方互換のバグ修正

### 非推奨のポリシー

非推奨とマークされた機能は、次のようになります。

1. 変更履歴に記録される
2. 2 つのマイナーバージョンにわたって非推奨の警告を表示する
3. 次のメジャーバージョンで削除される

### サポートのポリシー

- **現在のバージョン**: バグ修正と機能を伴う完全サポート
- **1 つ前のマイナーバージョン**: バグ修正のみ
- **より古いバージョン**: コミュニティサポートのみ

## アップグレードガイド

### 0.x から 1.0 へ

1. **設定の変更**
   - 旧: `max_iter` → 新: `max_iterations`
   - 旧: `agent_name` → 新: `agent`

2. **API の変更**
   - `RalphOrchestrator.execute()` → `RalphOrchestrator.run()`
   - 戻り値の形式がタプルから辞書に変更

3. **ファイル構造**
   - 状態ファイルが `.ralph/` から `.agent/metrics/` に移動
   - チェックポイント形式を更新

### 移行スクリプト

```bash
#!/bin/bash
# Migrate from 0.x to 1.0

# Backup old data
cp -r .ralph .ralph.backup

# Create new structure
mkdir -p .agent/metrics .agent/prompts .agent/checkpoints

# Migrate state files
mv .ralph/*.json .agent/metrics/ 2>/dev/null

# Update configuration
if [ -f "ralph.conf" ]; then
    python -c "
import json
with open('ralph.conf') as f:
    old_config = json.load(f)
# Update keys
old_config['max_iterations'] = old_config.pop('max_iter', 100)
old_config['agent'] = old_config.pop('agent_name', 'auto')
# Save new config
with open('ralph.json', 'w') as f:
    json.dump(old_config, f, indent=2)
"
fi

echo "Migration complete!"
```

## リリースのプロセス

### 1. リリース前のチェックリスト

- [ ] すべてのテストが通る
- [ ] ドキュメントが更新されている
- [ ] `CHANGELOG.md` が更新されている
- [ ] `Cargo.toml` でワークスペースのバージョンが更新されている
- [ ] `package.json` で npm パッケージのバージョンが更新されている
- [ ] README/インストール例がテストされている
- [ ] リリースワークフローのシークレットと npm の信頼された公開がまだ設定されている

### 2. リリースの手順

Ralph は `.github/workflows/release.yml` のタグ駆動の GitHub Actions ワークフローを
使います。リリースは `cargo-dist` によって自動的に作成され、その後 crates.io と npm の
公開ジョブが実行されます。

```bash
# 1. リリースメタデータを更新する
$EDITOR Cargo.toml
$EDITOR package.json
$EDITOR CHANGELOG.md

# 2. ローカルで検証する
cargo test
cargo package -p ralph-cli --allow-dirty --list > /dev/null
npm test

# 3. リリース準備をコミットする
git add Cargo.toml Cargo.lock package.json CHANGELOG.md README.md docs/
git commit -m "release: prepare vX.Y.Z"

# 4. リリースにタグを付ける
git tag -a vX.Y.Z -m "vX.Y.Z"

# 5. ブランチとタグをプッシュする
git push origin main
git push origin vX.Y.Z

# 6. リリースワークフローを監視する
gh run watch --workflow Release

# 7. 公開された成果物を検証する
gh release view vX.Y.Z
cargo install ralph-cli --version X.Y.Z
npm view @ralph-orchestrator/ralph-cli version
```

### 3. 自動ワークフローが公開するもの

タグのプッシュが成功すると、リリースワークフローは次を行います。

- `cargo-dist` で GitHub Release のアーカイブ/インストーラをビルドする
- GitHub Release を自動的に作成する
- ワークスペースのクレートを依存順に crates.io に公開する
- 信頼された公開を通じて npm パッケージを公開する

### 4. リリース後

- [ ] npm インストールをスモークテストする
- [ ] Cargo インストールをスモークテストする
- [ ] GitHub Releases のシェルインストーラをスモークテストする
- [ ] リリースを告知する
- [ ] 外部のパッケージ tap やミラーを更新する
- [ ] 次のリリースを計画する

## コントリビューター

Ralph Orchestrator の改善に協力してくださったすべてのコントリビューターに感謝します。

- Geoffrey Huntley (@ghuntley) - 元の Ralph Wiggum テクニック
- GitHub を通じたコミュニティのコントリビューター

## コントリビューションの方法

次の詳細は [CONTRIBUTING.md](../contributing.ja.md) を参照してください。

- バグの報告
- 機能の提案
- プルリクエストの提出
- 開発環境のセットアップ

## リンク

- [GitHub リポジトリ](https://github.com/mikeyobrien/ralph-orchestrator)
- [Issue トラッカー](https://github.com/mikeyobrien/ralph-orchestrator/issues)
- [ディスカッション](https://github.com/mikeyobrien/ralph-orchestrator/discussions)
- [ドキュメント](https://mikeyobrien.github.io/ralph-orchestrator/)
