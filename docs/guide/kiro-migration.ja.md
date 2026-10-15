# Q Chat から Kiro CLI への移行

Amazon Q Developer CLI は **Kiro CLI**（v1.20 以降）にリブランドされました。Ralph
Orchestrator v1.2.3 以降は、新しい `KiroAdapter` によってこの移行を完全にサポートします。

このガイドは、既存の Q Chat の設定とワークフローを Kiro CLI に移行するのに役立ちます。

## 簡単な要約

- **新しいコマンド:** `kiro-cli`（`q` を置き換える）
- **アダプタフラグ:** `-a kiro`（`-a q` または `-a qchat` を置き換える）
- **設定セクション:** `adapters.kiro`（`adapters.qchat` を置き換える）
- **環境変数:** `RALPH_KIRO_*`（`RALPH_QCHAT_*` を置き換える）

## コマンドラインの変更

Ralph を Kiro CLI で実行するには:

```bash
# 新しい方法
ralph run -a kiro

# レガシーな方法（引き続き動作するが非推奨）
ralph run -a q
ralph run -a qchat
```

`kiro-cli` が見つからない場合、Ralph は利用可能であれば自動的に `q` コマンドにフォール
バックし、後方互換性を保ちます。

## 設定の変更

### ralph.yml

新しい `kiro` セクションを使うよう `ralph.yml` の設定を更新します。`q` と `qchat`
セクションは非推奨ですが、引き続きサポートされます。

```yaml
# 新しい設定
adapters:
  kiro:
    enabled: true
    timeout: 600
    args: []
    env: {}

# 非推奨の設定
# adapters:
#   q:
#     enabled: true
#     timeout: 600
```

### 環境変数

環境変数を新しい名前空間に更新します。

| レガシー変数 | 新しい変数 | 既定 |
|----------------|--------------|---------|
| `RALPH_QCHAT_COMMAND` | `RALPH_KIRO_COMMAND` | `kiro-cli` |
| `RALPH_QCHAT_TIMEOUT` | `RALPH_KIRO_TIMEOUT` | `600` |
| `RALPH_QCHAT_PROMPT_FILE` | `RALPH_KIRO_PROMPT_FILE` | `PROMPT.md` |
| `RALPH_QCHAT_TRUST_TOOLS` | `RALPH_KIRO_TRUST_TOOLS` | `true` |
| `RALPH_QCHAT_NO_INTERACTIVE` | `RALPH_KIRO_NO_INTERACTIVE` | `true` |

## システムパス

Kiro CLI は、設定とデータに新しいディレクトリパスを使います。Ralph のアダプタはこれらの
変更を認識していますが、手動のセットアップやスクリプトは更新すべきです。

| コンポーネント | レガシーパス（Q Chat） | 新しいパス（Kiro） |
|-----------|----------------------|-----------------|
| **MCP サーバー** | `~/.aws/amazonq/mcp.json` | `~/.kiro/settings/mcp.json` |
| **プロンプト** | `~/.aws/amazonq/prompts` | `~/.kiro/prompts` |
| **プロジェクト設定** | `.amazonq/` | `.kiro/` |
| **グローバル設定** | `~/.aws/amazonq/` | `~/.kiro/` |
| **ログ** | `$TMPDIR/qchat-log` | `$TMPDIR/kiro-log` |

## 移行の手順

1.  **Kiro CLI をインストールする**: 新しい Kiro CLI（バージョン 1.20 以降）を
    インストール済みであることを確認します。
2.  **設定を更新する**: `q` アダプタの設定を `kiro` に置き換えるよう `ralph.yml` を
    更新します。
3.  **スクリプトを更新する**: CI/CD や起動スクリプトを `ralph run -a kiro` を使うよう
    変更します。
4.  **MCP 設定を移動する**: カスタム MCP サーバーを使っている場合は、`mcp.json` を新しい
    場所に移動します。
    ```bash
    mkdir -p ~/.kiro/settings
    cp ~/.aws/amazonq/mcp.json ~/.kiro/settings/mcp.json
    ```

## 後方互換性

Ralph は完全な後方互換性を維持します。
- `-a q` の実行は引き続き動作する（内部でレガシー設定とともに `KiroAdapter` を使う）。
- `kiro-cli` が欠けている場合は `q` にフォールバックする。
- 古い環境変数（`RALPH_QCHAT_*`）は、設定を厳密に分離するために `KiroAdapter` によって
  自動的には読まれませんが、（それらを読む）レガシーの `QChatAdapter` は、可能な場合は
  `KiroAdapter` のロジックにリダイレクトするか、フォールバックとして動作します。

> **メモ:** `QChatAdapter` クラスは現在非推奨であり、初期化時に警告を発します。
