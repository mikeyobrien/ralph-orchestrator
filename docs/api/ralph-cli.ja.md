# ralph-cli

バイナリのエントリポイントと CLI パースです。

## 概要

`ralph-cli` は、次を行うメインバイナリです。

- コマンドライン引数をパースする
- コマンドハンドラへルーティングする
- ランタイムのロギング/出力の挙動を構成する

## トップレベルコマンド

`crates/ralph-cli/src/main.rs` の `Commands` 列挙型には、現在次が含まれます。

- `run`
- `preflight`
- `hooks`
- `doctor`
- `tutorial`
- `events`
- `init`
- `clean`
- `emit`
- `plan`
- `code-task`（隠しレガシーエイリアス `task` も含む）
- `tools`
- `loops`
- `hats`
- `tui`
- `web`
- `mcp`
- `bot`
- `completions`

ユーザー向けのフラグと例については、正典の CLI ガイド `docs/guide/cli-reference.md` を
参照してください。

## MCP サーバーモード（`ralph mcp`）

`ralph mcp serve` は、`stdio` 経由の Model Context Protocol サーバーとして Ralph を実行します。

備考:

- MCP クライアント構成向け（非対話的）
- プロトコルメッセージには stdout、ログには stderr を使う
- `stream_next` のようなストリームポーリングツールを含む、制御プレーンのツールを公開する

## ランタイムディレクトリ

Ralph のランタイム状態は `.ralph/` 配下に保管され、既定で無視されます。コミットされる計画
成果物は `.ralph/specs/` に、コミットされるコードタスクファイルは `.ralph/tasks/` に置かれます。
レガシーの `.agent/` パスは新しい成果物には使われません。

## コマンドディスパッチ

ディスパッチは、`run()` 内で `cli.command` に対する `match` を通じて処理され、各サブモジュール
（例: `web::execute(args).await`、`mcp::execute(args).await`、`bot::execute(...)`）に委譲されます。

## グローバルオプション

グローバル CLI オプションには次が含まれます。

- `--config <PATH>`
- `--verbose`
- `--color <auto|always|never>`

## シェル補完

`ralph completions <shell>` は補完スクリプトを出力します。

例:

```bash
ralph completions bash > ~/.local/share/bash-completion/completions/ralph
```

## 終了コード

コマンドハンドラは `anyhow::Result` を介してプロセスエラーを返し、バイナリのエントリポイントで
表面化されます。
