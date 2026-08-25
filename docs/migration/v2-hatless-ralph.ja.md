# 移行ガイド: v2.0 Hatless Ralph

このガイドは、「Hatless Ralph」アーキテクチャを導入した v2.0 への、v1.x からの移行に
役立ちます。

## 何が変わったか

**v1.x**: Ralph は、オーケストレーターにハードコードされたハット（planner、builder）を
かぶっていました。

**v2.0**: Ralph は恒常的なコーディネーターです。ハットは任意で設定可能です。Ralph は既定で
すべてのイベントを処理します。

## 破壊的変更

1. **既定のハットなし**: 空の設定 = ソロ Ralph モード（ハットなし）
2. **JSONL イベント**: イベントは出力の XML ではなく
   `.ralph/events-YYYYMMDD-HHMMSS.jsonl` に書き込まれる
3. **ハットごとのバックエンド**: 各ハットが独自のバックエンドを指定できる
4. **Planner の削除**: 自動の planner ハットはない
5. **イベントディレクトリの移動**: イベントは今では `.agent/`（エージェントの状態）ではなく
   `.ralph/`（オーケストレーターのメタデータ）にある

## 移行の手順

### ソロモード（ハットなし）

**変更前（v1.x）:**
```yaml
cli:
  backend: claude
```

**変更後（v2.0）:**
```yaml
cli:
  backend: claude
# hats セクションなし = Ralph がすべてを処理する
```

Ralph はすべてのプロンプトを直接受け取り、イベントを
`.ralph/events-YYYYMMDD-HHMMSS.jsonl` に書き込みます。

### マルチハットモード

**変更前（v1.x）:**
```yaml
cli:
  backend: claude
# planner と builder ハットは自動だった
```

**変更後（v2.0）:**
```yaml
cli:
  backend: claude

hats:
  - name: builder
    triggers: ["build.task"]
    publishes: ["build.done", "build.blocked"]
    backend: claude
    default_publishes: "build.done"
    instructions: |
      You're building. Pick ONE task from scratchpad.
```

### ハットごとのバックエンド

**v2.0 の新機能**: 各ハットが異なるバックエンドを使えます。

```yaml
cli:
  backend: claude  # Ralph の既定

hats:
  - name: builder
    backend: gemini  # このハットは Gemini を使う
    triggers: ["build.task"]
    
  - name: reviewer
    backend:
      type: kiro
      agent: codex  # カスタムエージェント付きの Kiro
    triggers: ["review.request"]
```

### 既定の公開

**v2.0 の新機能**: ハットは、イベントを書き忘れた場合のフォールバックイベントを指定
できます。

```yaml
hats:
  - name: builder
    triggers: ["build.task"]
    default_publishes: "build.done"
```

builder がイベントを書かずに完了した場合、Ralph は自動的に `build.done` を注入します。

## イベント形式

**変更前（v1.x）**: エージェント出力の XML イベント
```xml
<event topic="build.done">
tests: pass
lint: pass
typecheck: pass
audit: pass
coverage: pass
</event>
```

**変更後（v2.0）**: `.ralph/events-YYYYMMDD-HHMMSS.jsonl` の JSONL
```bash
# 推奨: 安全な JSON 整形のために ralph emit を使う
ralph emit build.done "tests: pass, lint: pass, typecheck: pass, audit: pass, coverage: pass"
```

各実行は、一意のタイムスタンプ付きイベントファイルを作成します。イベントを安全に書き込む
には `ralph emit` を使ってください。

## よくある設定

### 機能開発（マルチハット）

```yaml
cli:
  backend: claude

hats:
  - name: builder
    triggers: ["build.task"]
    publishes: ["build.done", "build.blocked"]
    backend: claude
    default_publishes: "build.done"
    
  - name: tester
    triggers: ["test.request"]
    publishes: ["test.pass", "test.fail"]
    backend: gemini
```

### リサーチ（ソロモード）

```yaml
cli:
  backend: claude
# ハットなし - Ralph がすべてを行う
```

### 混合バックエンド

```yaml
cli:
  backend: claude

hats:
  - name: fast-tasks
    backend: gemini
    triggers: ["quick.task"]
    
  - name: complex-tasks
    backend: claude
    triggers: ["complex.task"]
```

## 検証

設定をテストします。
```bash
ralph validate ralph.yml
```

## ロールバック

v1.x の挙動にロールバックする必要がある場合は、プリセットを使います。
```bash
ralph run --preset feature
```

プリセットは、厳選されたマルチハット設定を提供します。
