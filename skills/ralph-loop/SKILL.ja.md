---
name: ralph-loop
description: Run, monitor, resume, merge, and debug Ralph loops. Use this skill whenever the user asks to operate `ralph run` or `ralph loops`, inspect loop state, recover suspended loops, analyze diagnostics, or unblock merge queue issues.
---

# Ralph ループ

このスキルは、Ralph のループを外部から操作するために使用します。

## このスキルを使う場面

- 適切な `-c` と `-H` の入力を指定して、Ralph の実行を開始または継続する
- ループの状態、ワークツリー、ログ、履歴、差分を確認する
- フックで一時停止したループを再開する
- 完了したワークツリーループをマージ、または破棄する
- 現在の診断ファイルを使って、想定外のループ挙動をデバッグする

## ワークフロー

1. まず `ralph loops list` または `ralph loops list --json` で現在の状態を把握します。
2. 実行したい場合は、適切なコア設定とハットソースを指定して `ralph run ...` を実行します。
3. ループが停滞していたり不審な場合は、状態を変更する前に `logs`、`history`、`diff`
   を確認します。
4. ループが一時停止している場合は `.ralph/suspend-state.json` を読み、
   `ralph loops resume <id>` を使います。
5. ループがキュー待ち、または `needs-review` の場合は、まず差分を確認し、状況に応じて
   `merge`、`process`、`retry`、`discard` を使い分けます。
6. ハット、イベント、ツール呼び出し、パースエラー、パフォーマンスについて詳細な根拠が
   必要なときは診断を使います。

## ガードレール

- `.ralph` の状態ファイルを直接編集するより、CLI を優先します。
- タスクとメモリを正規のランタイムシステムとして扱い、スクラッチパッドを主要な状態モデルの
  中心に据えないでください。
- マージ前に差分を確認します。
- ロックやキューの成果物を削除するのは、対象プロセスが停止済みであることを確認できた場合のみ
  にします。
- `.ralph/` 配下の手動編集は最終手段の復旧手順であり、使用した場合は明示的にその旨を
  伝えてください。

## 必要に応じて参照するリファレンス

- コマンドのレシピと運用フロー: `references/commands.md`
- 診断ファイルと suspend-state の詳細: `references/diagnostics.md`
