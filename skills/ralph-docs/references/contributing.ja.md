# Ralph へのコード変更を提案する

これは、ユーザーが（自分のハットコレクションではなく）`ralph-orchestrator` リポジトリ
自体で何かを改善・修正したいときに使います。

## コードを書く前に

1. **すでに機能として存在しないか確認する。** llms.txt から関連するドキュメントページを
   取得する。Ralph は意図的に最小限で、答えが「既存のつまみを使う」であることもある。
2. **信条に違反しないか確認する。** `concepts/tenets/index.md` を読む。6 つの信条は
   根幹をなす。フレッシュコンテキスト、バックプレッシャー、使い捨ての計画、ディスクは
   状態、信号で舵を取る、Ralph に Ralph させる。これらに逆らう変更は、たいてい別の場所に
   属する。
3. **正しいクレートに対応づける。** 下記のクレートマップを参照。

## クレートマップ

```
ralph-cli      → CLI エントリポイント。サブコマンド（run, plan, task, loops, web, hats）
ralph-core     → オーケストレーションロジック: イベントループ、ハット、メモリ、タスク、preset_source
ralph-adapters → バックエンド統合（Claude, Kiro, Gemini, Codex, Roo など）
ralph-telegram → ヒューマンインザループ用の Telegram ボット
ralph-tui      → ターミナル UI（ratatui）
ralph-e2e      → エンドツーエンドのテストフレームワーク
ralph-proto    → プロトコル定義
ralph-bench    → ベンチマーク
ralph-api      → HTTP API サーバー
backend/       → Web ダッシュボードサーバー（Fastify + tRPC + SQLite）
frontend/      → Web ダッシュボード UI（React + Vite）
```

より正確なファイルマップ（リポジトリの AGENTS.md より）:

| サブシステム | ファイル |
|---|---|
| イベントループ | `crates/ralph-core/src/event_loop/mod.rs` |
| ハットシステム | `crates/ralph-core/src/hatless_ralph.rs` |
| メモリ | `crates/ralph-core/src/memory.rs`, `memory_store.rs` |
| タスク | `crates/ralph-core/src/task.rs`, `task_store.rs` |
| プリセットソース（YAML + TOML） | `crates/ralph-core/src/preset_source.rs` |
| ロック調整 | `crates/ralph-core/src/worktree.rs` |
| ループレジストリ | `crates/ralph-core/src/loop_registry.rs` |
| マージキュー | `crates/ralph-core/src/merge_queue.rs` |
| CLI コマンド | `crates/ralph-cli/src/loops.rs`, `hats.rs`, `task_cli.rs`, `main.rs` |
| バックエンド選択 | `crates/ralph-adapters/src/cli_backend.rs`, `auto_detect.rs` |
| ACP エグゼキュータ | `crates/ralph-adapters/src/acp_executor.rs` |
| 事前確認 | `crates/ralph-cli/src/preflight.rs` |
| Doctor | `crates/ralph-cli/src/doctor.rs` |

## 必須のビルド/テスト手順

PR を開く前に、次を順番に実行します。

```bash
cargo fmt                                            # 警告は許容しない
cargo clippy --all-targets --all-features -- -D warnings
cargo test -p ralph-core                             # ローダー + コアのテスト
cargo test -p ralph-cli --bin ralph <your_module>::  # モジュール単位
cargo test -p ralph-cli --test <integration_file>    # 統合テスト
./scripts/ci-rust-gate.sh                            # CI が実際に実行する内容
```

変更がユーザーに見える場合は、次も更新します。

- `docs/guide/<relevant>.md`（ユーザーに見える挙動の変更）
- `docs/reference/changelog/index.md`（Unreleased の下に 1 行追加）
- 変更が影響する場合は `presets/` 配下のプリセット YAML/TOML

## PR の慣習

- **ブランチ命名**: `feat/<topic>`、`fix/<topic>`、`docs/<topic>`。
- **コミットスタイル**: Conventional Commits — `feat(cli):`、`fix(core):`、
  `docs(presets):` など。サブジェクトは 72 文字以内に収める。
- **本文**: 単なる*何を*ではなく*なぜ*を説明する。あれば issue / ドキュメントに
  リンクする。
- **マージ方式**: squash。リポジトリは既定で
  `gh pr merge <N> --squash --delete-branch` により squash マージする。
- **main に直接プッシュしない**。些細な変更でも PR を開く。

## 実際の Ralph ループに対して変更をテストする

（見た目の変更ではなく）挙動の変更については、マージ前に、最小のプリセットで実際の
バックエンドに対して少なくとも 1 回のエンドツーエンドのイテレーションを実行します。

```bash
cd /tmp/ralph-smoketest && mkdir -p x && cd x
echo "trivial task" > PROMPT.md
ralph run -H builtin:hatless-baseline -b claude -P PROMPT.md --max-iterations 2 -a -q
cat .ralph/events-*.jsonl
```

イベントが発火し、終了理由が期待どおりであることを確認します。

## いつ見送るか

- 純粋な表面的な仕上げ（誤字、古いドキュメントリンク、書式）→ ドキュメントの PR を開く。
- 複数のクレートに触れる機能要望 → まず `specs/` の下にスペックを書き、コーディング前に
  合意を得る。
- ハットの契約、イベントループのセマンティクス、ディスクレイアウト（`.ralph/` 構造）を
  変える変更 → 6 つの信条ドキュメントをすべて読み、スペックを起草し、議論を想定する。

## マージ後

ローカルの main を同期し、再インストールします。

```bash
git checkout main && git pull --rebase
cargo build --release --bin ralph
install -m 755 target/release/ralph ~/.cargo/bin/ralph
ralph --version
```

`cp` ではなく `install` を使ってください。macOS の Gatekeeper は元の署名をキャッシュ
しており、内容が変わった cp 済みバイナリを黙って SIGKILL することがあります。
