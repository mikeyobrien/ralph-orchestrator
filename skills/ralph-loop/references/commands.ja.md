# Ralph ループのコマンド

## 実行の開始

```bash
ralph run -c ralph.yml -H .ralph/hats/my-workflow.yml -p "Add OAuth login"
```

手早く事前確認するには `--dry-run` を使います。

```bash
ralph run -c ralph.yml -H .ralph/hats/my-workflow.yml -p "Add OAuth login" --dry-run
```

## ループの検査

```bash
ralph loops list
ralph loops list --json
ralph loops logs <id>
ralph loops logs <id> -f
ralph loops history <id>
ralph loops history <id> --json
ralph loops diff <id>
ralph loops diff <id> --stat
ralph loops attach <id>
```

呼び出し側が構造化された出力を求めている場合は `list --json` と `history --json`
を使います。

## マージキューの操作

```bash
ralph loops merge <id>
ralph loops process
ralph loops retry <id>
ralph loops discard <id> -y
ralph loops merge-button-state <id>
```

キュー待ち、または `needs-review` の作業に推奨されるフロー:

1. `ralph loops diff <id> --stat`
2. `ralph loops history <id>`
3. `ralph loops merge <id>` または `ralph loops retry <id>`
4. 作業を破棄すべき場合は `ralph loops discard <id> -y`

## 停止または再開

```bash
ralph loops stop <id>
ralph loops stop <id> --force
ralph loops resume <id>
ralph loops prune
```

`resume` は、ループが実際に一時停止しているときにのみ使います。このコマンドは冪等であり、
一時停止の境界で Ralph が消費するオペレーター信号を書き込みます。
