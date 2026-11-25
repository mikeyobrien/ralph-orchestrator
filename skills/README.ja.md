# Ralph Orchestrator エージェントスキル

このディレクトリは、Ralph を操作する外部エージェントハーネス向けの、正規の公開スキル
パッケージです。

3 つのスキルを同梱しています。

- `ralph-hats`: ハットコレクションの作成、検査、検証、改善を行う
- `ralph-loop`: Ralph ループの実行、監視、再開、マージ、デバッグを行う
- `ralph-docs`: 公開されている `llms.txt` ドキュメントマップを通じて Ralph 自体を
  内省・改善する。「Ralph は X をどうやって行うのか？」という質問に答え、コード変更の
  範囲を ralph-orchestrator リポジトリに絞り込む

これらは公開エージェントスキルです。Ralph 内部の `ralph tools skill` レジストリの一部
ではありません。

## Claude Code でのインストール

このリポジトリをマーケットプレイスのソースとして追加します。

```text
/plugin marketplace add mikeyobrien/ralph-orchestrator
```

その後、マーケットプレイスのブラウザから `ralph-orchestrator` プラグインをインストール
します。

## Vercel の `npx skills` でのインストール

このリポジトリ内のスキル一覧を表示します。

```bash
npx skills add mikeyobrien/ralph-orchestrator --list
```

Claude Code 向けにすべてのスキルをインストールします。

```bash
npx skills add mikeyobrien/ralph-orchestrator \
  --skill ralph-hats \
  --skill ralph-loop \
  --skill ralph-docs \
  -a claude-code \
  -y
```

Codex 系エージェント向けに 1 つのスキルをインストールします。

```bash
npx skills add mikeyobrien/ralph-orchestrator \
  --skill ralph-loop \
  -a codex \
  -y
```

ローカル開発中は、チェックアウト済みのリポジトリからもインストールできます。

```bash
npx skills add . --list
```
