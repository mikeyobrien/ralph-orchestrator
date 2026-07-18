# バックエンド

Ralph は複数の AI CLI バックエンドをサポートします。このガイドはセットアップと選択を
扱います。

## サポートされるバックエンド

| バックエンド | CLI ツール | メモ |
|---------|----------|-------|
| Claude Code | `claude` | 推奨、主要サポート |
| Kiro | `kiro-cli` | Amazon/AWS |
| Gemini CLI | `gemini` | Google |
| Codex | `codex` | OpenAI |
| Forge | `forge` | マルチプロバイダのターミナルエージェント |
| Amp | `amp` | Sourcegraph |
| Copilot CLI | `copilot` | GitHub |
| OpenCode | `opencode` | コミュニティ |
| Pi | `pi` | マルチプロバイダ |

## 自動検出

Ralph はインストール済みのバックエンドを自動検出します。

```bash
ralph init
# 利用可能なバックエンドを自動検出する
```

検出順（最初に利用可能なものが勝ち）:
1. Claude
2. Kiro
3. Gemini
4. Codex
5. Forge
6. Amp
7. Copilot
8. OpenCode
9. Pi

## 明示的な選択

自動検出を上書きします。

```bash
# CLI 経由
ralph init --backend kiro
ralph run --backend gemini

# 設定経由
# ralph.yml
cli:
  backend: "claude"
```

## バックエンドのセットアップ

以下の各バックエンドには次が含まれます。
- **インストール**手順
- **認証と環境変数**（API キーまたはログイン）
- **ハット YAML** の設定
- **`ralph doctor`** の検証メモ

バックエンド名（YAML と CLI フラグで使う）: `claude`, `kiro`, `gemini`, `codex`,
`forge`, `amp`, `copilot`, `opencode`, `pi`。

### Claude Code（`claude`）

完全な機能サポートを備えた推奨バックエンド。

```bash
# インストール
npm install -g @anthropic-ai/claude-code

# 認証
claude login

# 確認
claude --version
```

**認証と環境変数:**
- `claude login`（推奨）
- `ANTHROPIC_API_KEY`（`ralph doctor` の認証ヒントで使われる）

**ハット YAML:**
```yaml
hats:
  planner:
    backend: "claude"
```

**Doctor のチェック:**
- `claude --version` が成功しなければならない
- `ANTHROPIC_API_KEY` が欠けていると警告する

**機能:**
- 完全なストリーミング対応
- すべてのハット機能
- メモリ統合

### Kiro（`kiro`）

Amazon/AWS の AI アシスタント。

```bash
# インストール
# https://kiro.dev/ を参照

# 確認
kiro-cli --version
```

**認証と環境変数:**
- Kiro のドキュメントに従って Kiro CLI の認証（AWS/SSO）を完了する
- `KIRO_API_KEY`（任意。`ralph doctor` の認証ヒントで使われる）

**ハット YAML:**
```yaml
hats:
  coder:
    backend: "kiro"
```

**Kiro のエージェント選択（任意）:**
```yaml
hats:
  reviewer:
    backend:
      type: "kiro"
      agent: "codex"
```

**Doctor のチェック:**
- `kiro-cli --version` が成功しなければならない
- `KIRO_API_KEY` が欠けていると警告する（CLI で認証済みなら問題なし）

### Gemini CLI（`gemini`）

Google の AI CLI。

```bash
# インストール
npm install -g @google/gemini-cli

# API キーを設定する
export GEMINI_API_KEY=your-key

# 確認
gemini --version
```

**認証と環境変数:**
- `GEMINI_API_KEY`（`ralph doctor` の認証ヒントで使われる）

**ハット YAML:**
```yaml
hats:
  analyst:
    backend: "gemini"
```

**Doctor のチェック:**
- `gemini --version` が成功しなければならない
- `GEMINI_API_KEY` が欠けていると警告する

### Codex（`codex`）

OpenAI のコード重視モデル。

```bash
# インストール
# https://github.com/openai/codex を参照

# 設定
export OPENAI_API_KEY=your-key

# 確認
codex --version
```

**認証と環境変数:**
- `OPENAI_API_KEY` または `CODEX_API_KEY`（どちらかで `ralph doctor` の認証ヒントを満たす）

**ハット YAML:**
```yaml
hats:
  coder:
    backend: "codex"
```

**Doctor のチェック:**
- `codex --version` が成功しなければならない
- `OPENAI_API_KEY` も `CODEX_API_KEY` も設定されていないと警告する

### Forge（`forge`）

マルチプロバイダのターミナル AI エージェント。

```bash
# インストール
curl -fsSL https://forgecode.dev/cli | sh

# 認証
forge provider login

# 確認
forge --version
```

**認証と環境変数:**
- `forge provider login` または Forge のプロバイダ設定を通じてプロバイダを設定する
- Forge については `ralph doctor` が認証環境変数をチェックしない

**ハット YAML:**
```yaml
hats:
  coder:
    backend: "forge"
```

**Forge のエージェント選択（任意）:**
```yaml
hats:
  reviewer:
    backend:
      type: "forge"
      args: ["--agent", "sage"]
```

**Doctor のチェック:**
- `forge --version` が成功しなければならない

**実行モード:**
- ヘッドレスの Ralph 実行は `forge -p "<prompt>"` を呼ぶ
- 対話的な Forge は `forge` として起動される。Forge は引数なしの対話モードでの初期プロンプト
  注入をサポートしない

### Amp（`amp`）

Sourcegraph の AI アシスタント。

```bash
# インストール
# https://github.com/sourcegraph/amp を参照

# 確認
amp --version
```

**認証と環境変数:**
- Sourcegraph のドキュメントに従って `amp` CLI で認証する
- Amp については `ralph doctor` が認証環境変数をチェックしない

**ハット YAML:**
```yaml
hats:
  helper:
    backend: "amp"
```

**Doctor のチェック:**
- `amp --version` が成功しなければならない

### Copilot CLI（`copilot`）

GitHub の AI アシスタント。

```bash
# インストール
npm install -g @github/copilot

# 認証
copilot auth login

# 確認
copilot --version
```

**認証と環境変数:**
- Copilot CLI（`copilot auth login` または `gh auth login`）で認証する
- Copilot については `ralph doctor` が認証環境変数をチェックしない

**ハット YAML:**
```yaml
hats:
  reviewer:
    backend: "copilot"
```

**Doctor のチェック:**
- `copilot --version` が成功しなければならない

### OpenCode（`opencode`）

コミュニティの AI CLI。

```bash
# インストール
curl -fsSL https://opencode.ai/install | bash

# 確認
opencode --version
```

**認証と環境変数:**
- 次のいずれかを設定する: `OPENCODE_API_KEY`, `ANTHROPIC_API_KEY`, `OPENAI_API_KEY`
- OpenCode は複数のプロバイダをプロキシできる。使うプロバイダに合った環境変数を使う

**ハット YAML:**
```yaml
hats:
  strategist:
    backend: "opencode"
```

**Doctor のチェック:**
- `opencode --version` が成功しなければならない
- `OPENCODE_API_KEY`, `ANTHROPIC_API_KEY`, `OPENAI_API_KEY` のいずれも設定されていないと
  警告する

### Pi（`pi`）

マルチプロバイダの AI コーディングアシスタント。

```bash
# インストール
npm install -g @earendil-works/pi-coding-agent

# 確認
pi --version
```

**認証と環境変数:**
- 次のいずれかを設定する: `ANTHROPIC_API_KEY`, `OPENAI_API_KEY`, `GEMINI_API_KEY`、または
  サポートされる任意のプロバイダキー
- Pi は `--provider` で指定されたプロバイダにルーティングする（既定: google）
- `--api-key` で API キーを明示的に渡すか、プロバイダ固有の環境変数に頼る

**ハット YAML:**
```yaml
hats:
  coder:
    backend: "pi"
```

**Pi のプロバイダ選択（任意）:**
```yaml
hats:
  coder:
    backend:
      type: "pi"
      args: ["--provider", "anthropic", "--model", "claude-sonnet-4"]
```

**Doctor のチェック:**
- `pi --version` が成功しなければならない
- プロバイダの API キーが設定されていないと警告する

## ハットごとのバックエンド上書き

異なるハットは異なるバックエンドを使えます。

```yaml
hats:
  planner:
    backend: "claude"  # 計画に Claude を使う
    triggers: ["task.start"]
    instructions: "Create a plan..."

  coder:
    backend: "kiro"    # コーディングに Kiro を使う
    triggers: ["plan.ready"]
    instructions: "Implement..."
```

## カスタムバックエンド

サポートされていない CLI には、カスタムバックエンドを使います。

```yaml
cli:
  backend: "custom"
  custom_command: "my-ai-cli"
  prompt_mode: "arg"  # または "stdin"
```

**プロンプトモード:**

| モード | プロンプトの渡し方 |
|------|---------------------|
| `arg` | `my-ai-cli -p "prompt"` |
| `stdin` | `echo "prompt" \| my-ai-cli` |

## バックエンドの比較

| 機能 | Claude | Kiro | Gemini | Codex | Pi |
|---------|--------|------|--------|-------|----|
| ストリーミング | あり | あり | あり | あり | あり |
| ツール使用 | 完全 | 完全 | 部分的 | 部分的 | 完全 |
| コンテキストサイズ | 大 | 大 | 大 | 中 | 大 |
| 速度 | 速い | 速い | 速い | 中 | 速い |
| コスト | $$ | $ | $ | $$ | $ |

## トラブルシューティング

### バックエンドが見つからない

```
ERROR: No AI agents detected
```

**解決策:**
1. サポートされるバックエンドをインストールする
2. PATH に含まれていることを確認する
3. 直接テストする: `claude -p "test"` または `pi -p "test"`

### 認証に失敗した

```
ERROR: Authentication required
```

**解決策:**
```bash
# Claude
claude login

# Copilot
copilot auth login

# Gemini - API キーを設定する
export GEMINI_API_KEY=your-key

# Pi - プロバイダの API キーを設定する
export ANTHROPIC_API_KEY=your-key
```

CLI が既に認証済みなのに `ralph doctor` がまだ警告する場合は、上記の期待される環境変数が
設定されていることを確認してください（doctor のチェックはヒントであり、ハードな失敗では
ありません）。

### 誤ったバックエンドが使われる

```bash
# 特定のバックエンドを強制する
ralph run --backend claude

# または設定で指定する
cli:
  backend: "claude"
```

### バックエンドがハングする

一部のバックエンドは初回実行時に対話的な認証が必要です。

```bash
# まずバックエンドを直接実行する
claude -p "test"

# その後 Ralph で使う
ralph run
```

## ベストプラクティス

1. **主要なバックエンドを 1 つ選ぶ** — 一貫性が役立つ
2. **バックエンドを直接テストする** — Ralph で使う前に
3. **ハットごとの上書きは控えめに使う** — デバッグを複雑にし得る
4. **バックエンドを最新に保つ** — 新機能、バグ修正

## 次のステップ

- 自分のワークフロー向けに [プリセット](presets.ja.md) を設定する
- [コスト管理](cost-management.ja.md) について学ぶ
- [プロンプトの書き方](prompts.ja.md) を探る
