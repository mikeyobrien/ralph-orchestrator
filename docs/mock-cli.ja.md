# Mock CLI: 費用のかからない E2E テスト

## 課題

実際の AI バックエンド（Claude、Kiro、Gemini など）に対して E2E テストを実行することには、
いくつかの問題があります。

1. **コスト**: 各テスト実行が API クレジットを消費する
2. **速度**: ネットワークの遅延と API のレート制限が CI/CD を遅くする
3. **信頼性**: ネットワークの問題と API の可用性がテストの安定性に影響する
4. **決定性**: AI の応答は変動し、テストを非決定的にする

チームには、次のような E2E テストを実行する方法が必要です。
- 費用がかからない（API 呼び出しなし）
- 速く実行される（ネットワーク遅延なし）
- 決定的である（毎回同じ出力）
- 完全なオーケストレーションループを検証する（ユニットテストだけでなく）

## 解決策の概要

`mock-cli` サブコマンドは、実際の AI バックエンドを呼び出す代わりに、事前記録された JSONL
カセットをリプレイします。これにより次が可能になります。

- **ゼロコストのテスト**: API 呼び出しなし、クレジット消費なし
- **高速な実行**: 即時または加速されたリプレイ（10 倍以上の速度）
- **決定的な出力**: 同じカセット = 毎回同じ出力
- **完全な統合**: PTY を通じて完全なオーケストレーションループをテストする

モック CLI は、Ralph が期待するのと同じコマンドラインインターフェースを実装することで、
実際のバックエンドのドロップイン代替として機能します。

## 仕組み

### アーキテクチャ

```
ralph-e2e --mock
    │
    ├─ カスタムバックエンドで ralph.yml を書き込む
    │  cli:
    │    backend: custom
    │    command: ralph-e2e
    │    args: ["mock-cli", "--cassette", "path/to/cassette.jsonl"]
    │
    └─ ralph run（オーケストレーター）
        │
        └─ spawn: ralph-e2e mock-cli --cassette cassettes/e2e/connect.jsonl
            │
            ├─ SessionPlayer: JSONL カセットを読む
            │   └─ ux.terminal.write イベントを抽出する
            │
            ├─ 出力を stdout にリプレイする（PTY 経由）
            │
            └─ WhitelistExecutor: 承認されたローカルコマンドを実行する
                └─ ralph task add, ralph tools memory add など
```

### カセットの形式

カセットは、SessionRecorder からのタイムスタンプ付きイベントを含む JSONL ファイルです。

```jsonl
{"ts":1000,"event":"ux.terminal.write","data":{"bytes":"UE9ORw==","stdout":true,"offset_ms":0}}
{"ts":1100,"event":"bus.publish","data":{"command":"ralph task add 'test'"}}
{"ts":1200,"event":"ux.terminal.write","data":{"bytes":"RG9uZQ==","stdout":true,"offset_ms":200}}
```

各行は、次を持つ JSON オブジェクトです。
- `ts`: ミリ秒単位の Unix タイムスタンプ
- `event`: イベント種別（例: `ux.terminal.write`, `bus.publish`）
- `data`: イベント固有のペイロード

モック CLI は次を抽出します。
1. **ターミナルの書き込み**（`ux.terminal.write`）→ stdout にリプレイ
2. **コマンド**（command フィールドを持つ `bus.publish`）→ 許可されていれば実行

### カセットの命名規則

カセットは `cassettes/e2e/` に保存され、次の解決順を持ちます。

1. **バックエンド固有**: `<scenario-id>-<backend>.jsonl`
   - 例: `connect-claude.jsonl`, `task-add-kiro.jsonl`
   - バックエンド固有の挙動が異なるときに使う

2. **汎用のフォールバック**: `<scenario-id>.jsonl`
   - 例: `connect.jsonl`, `task-add.jsonl`
   - 挙動がバックエンド間で同一のときに使う

どちらも存在しない場合、テストは明確なエラーで即座に失敗します。

## 使い方ガイド

### モードでの E2E テストの実行

```bash
# モックバックエンドですべての E2E テストを実行する（ゼロコスト）
ralph-e2e --mock

# 加速されたリプレイで実行する（10 倍速）
ralph-e2e --mock --mock-speed 10.0

# 即時リプレイで実行する（遅延なし）
ralph-e2e --mock --mock-speed 0.0

# 特定のシナリオを実行する
ralph-e2e --mock --filter connect

# カスタムのカセットディレクトリ（既定: cassettes/e2e）
ralph-e2e --mock --cassette-dir /path/to/cassettes
```

### モック CLI の直接呼び出し

モック CLI は通常 Ralph によってカスタムバックエンドとして呼び出されますが、テストのために
直接実行できます。

```bash
# 基本的なリプレイ
ralph-e2e mock-cli --cassette cassettes/e2e/connect.jsonl

# 速度調整付き（10 倍速）
ralph-e2e mock-cli --cassette cassettes/e2e/connect.jsonl --speed 10.0

# コマンド実行の許可リスト付き
ralph-e2e mock-cli \
  --cassette cassettes/e2e/task-add.jsonl \
  --allow "ralph task add,ralph tools memory add"

# バージョンを確認する（バックエンドの可用性チェック用）
ralph-e2e mock-cli --version
```

### 前提条件

1. **カセットファイル**: `cassettes/e2e/` ディレクトリに存在しなければならない
2. **Ralph をインストール済み**: 許可リストのコマンド実行に必要
3. **ワークスペースのセットアップ**: モック CLI はシナリオのワークスペースディレクトリで実行
   される

### 新しいカセットの記録

新しいシナリオ用のカセットを作成するには:

```bash
# 実際のバックエンドとセッション記録で E2E テストを実行する
ralph run --record-session cassettes/e2e/my-scenario-claude.jsonl

# または記録を有効にした E2E ハーネスを使う
# （実装の詳細: E2E ハーネスは --record フラグをサポートすべき）
```

## API リファレンス

### コマンドラインインターフェース

```
ralph-e2e mock-cli [OPTIONS]

OPTIONS:
    --cassette <PATH>       JSONL カセットファイルへのパス（必須）
    --speed <FLOAT>         リプレイ速度の倍率（既定: 0.0 = 即時）
                           1.0 = リアルタイム, 10.0 = 10 倍速
    --allow <CSV>           許可リストに入れるコマンドプレフィックス（カンマ区切り）
                           例: "ralph task add,ralph tools memory add"
    --version              バージョンを表示して終了する（可用性チェック用）
    -h, --help             ヘルプ情報を表示する
```

### 終了コード

| コード | 意味 |
|------|---------|
| 0 | 成功 - カセットのリプレイに成功 |
| 1 | カセットファイルが見つからないか読めない |
| 2 | カセットの解析エラー（無効な JSONL） |
| 3 | リプレイエラー（出力中の I/O 失敗） |
| 4 | コマンド実行エラー（許可リストのコマンドが失敗） |

### 環境変数

| 変数 | 説明 | 既定 |
|----------|-------------|---------|
| `RALPH_MOCK_ALLOW` | コマンドの許可リスト（`--allow` を上書きする） | なし |

### カセット解決 API

`CassetteResolver` は、カセット解決へのプログラム的なアクセスを提供します。

```rust
use ralph_e2e::mock::{CassetteResolver, MockConfig};
use ralph_e2e::Backend;

// リゾルバを作成する
let resolver = CassetteResolver::new("cassettes/e2e");

// シナリオ + バックエンドのカセットを解決する
let path = resolver.resolve("connect", Backend::Claude)?;
// 返す: cassettes/e2e/connect-claude.jsonl（または connect.jsonl フォールバック）

// すべての候補パスを取得する（デバッグ用）
let candidates = resolver.candidates("connect", Backend::Claude);
// 返す: ["cassettes/e2e/connect-claude.jsonl", "cassettes/e2e/connect.jsonl"]
```

### モック設定 API

```rust
use ralph_e2e::mock::MockConfig;

// 既定の設定（即時リプレイ、標準の許可リスト）
let config = MockConfig::default();

// カスタム設定
let config = MockConfig::new("/custom/cassettes")
    .with_speed(10.0)
    .with_allow_commands("ralph task add,ralph task close");

// コマンド実行を無効にする
let config = MockConfig::default().without_commands();
```

## エッジケースと制限

### こういうときどうなる…

#### カセットが欠けているとき？

**挙動**: テストは明確なエラーメッセージで即座に失敗する

```
Error: cassette not found for scenario 'connect' backend 'claude'
Tried:
  - cassettes/e2e/connect-claude.jsonl
  - cassettes/e2e/connect.jsonl
```

**解決策**: このシナリオのカセットを記録するか、汎用のフォールバックを使う

#### カセットに無効な JSONL が含まれるとき？

**挙動**: 行番号と詳細付きの解析エラー

```
Error: cassette parse error in cassettes/e2e/connect.jsonl
Line 5: expected value at line 1 column 1
```

**解決策**: カセットの形式を検証するか、再記録する

#### コマンドが許可リストにないとき？

**挙動**: コマンドはスキップされ、stderr に警告が出る

```
[mock-cli] Skipping non-whitelisted command: rm -rf /
```

**解決策**: 安全で必要なら、コマンドを許可リストに追加する

#### 許可リストのコマンドが失敗するとき？

**挙動**: 警告を記録し、リプレイを継続する（致命的ではない）

```
[mock-cli] Warning: command 'ralph task close invalid-id' exited with status 1
```

**根拠**: リプレイ中のコマンド失敗は、シナリオが明示的にそれを確認しない限り、テストを
壊すべきではない

#### カセットにターミナルの書き込みがないとき？

**挙動**: モック CLI は何も出力せず、正常に終了する

**ユースケース**: 出力検証なしで副作用（タスク、メモリ）のみをテストするシナリオ

#### 速度が負のとき？

**挙動**: 0.0（即時リプレイ）にクランプされる

```rust
let config = MockConfig::default().with_speed(-5.0);
assert_eq!(config.speed, 0.0);
```

#### 複数のバックエンドが同じカセットを使うとき？

**挙動**: 汎用のカセット（`<scenario>.jsonl`）がすべてのバックエンドに使われる

**ユースケース**: バックエンドの挙動が同一のシナリオ（例: 接続チェック）

### 制限

1. **シェル機能なし**: コマンドの許可リストはパイプ、リダイレクト、変数展開をサポート
   **しない**
   - ✅ 許可: `ralph task add 'test'`
   - ❌ 不許可: `ralph task add 'test' | grep foo`

2. **ネットワークアクセスなし**: モック CLI は実際の API 呼び出しやネットワークリクエストを
   行えない
   - ネットワークを要する統合テストには実際のバックエンドモードを使う

3. **タイミングの近似**: リプレイのタイミングは近似であり、正確ではない
   - E2E 検証には十分だが、パフォーマンスのベンチマークには向かない

4. **コマンド実行は同期的**: 許可リストのコマンドは順次実行される
   - 並列実行やバックグラウンドプロセスはない

5. **PTY の制限**: モック CLI は PTY に出力し、ANSI エスケープシーケンスに影響することが
   ある
   - ほとんどのターミナル出力は正しく動作するが、複雑な TUI の相互作用は異なることがある

## 例

### 例 1: 基本的な接続テスト

**シナリオ**: Ralph がバックエンドに接続し出力を受け取れることを検証する

**カセット**（`cassettes/e2e/connect.jsonl`）:
```jsonl
{"ts":1000,"event":"ux.terminal.write","data":{"bytes":"UE9ORw==","stdout":true,"offset_ms":0}}
```

**使い方**:
```bash
ralph-e2e --mock --filter connect
```

**期待**: テストが通り、出力に "PONG" が含まれる

### 例 2: 副作用を伴うタスク作成

**シナリオ**: Ralph が `ralph task add` でタスクを作成できることを検証する

**カセット**（`cassettes/e2e/task-add.jsonl`）:
```jsonl
{"ts":1000,"event":"ux.terminal.write","data":{"bytes":"Q3JlYXRpbmcgdGFzaw==","stdout":true,"offset_ms":0}}
{"ts":1100,"event":"bus.publish","data":{"command":"ralph task add 'test task' -p 1"}}
{"ts":1200,"event":"ux.terminal.write","data":{"bytes":"VGFzayBjcmVhdGVk","stdout":true,"offset_ms":100}}
```

**使い方**:
```bash
ralph-e2e mock-cli \
  --cassette cassettes/e2e/task-add.jsonl \
  --allow "ralph task add"
```

**期待**: 
- 出力に "Creating task" と "Task created" が含まれる
- `.agent/tasks.jsonl` に新しいタスクエントリが含まれる

### 例 3: CI 向けの加速リプレイ

**シナリオ**: CI パイプラインで完全な E2E スイートを素早く実行する

**使い方**:
```bash
# すべてのテストを 10 倍速で実行する（遅延なし）
ralph-e2e --mock --mock-speed 0.0
```

**期待**: すべてのテストが分単位ではなく秒単位で完了する

### 例 4: バックエンド固有の挙動

**シナリオ**: Claude 固有の出力形式をテストする

**カセット**:
- `cassettes/e2e/format-claude.jsonl`（Claude 固有）
- `cassettes/e2e/format-kiro.jsonl`（Kiro 固有）
- `cassettes/e2e/format.jsonl`（汎用のフォールバック）

**使い方**:
```bash
# バックエンド固有のカセットで実行する
ralph-e2e --mock --filter format
```

**期待**: 各バックエンドは固有のカセットを使い、欠けていれば汎用にフォールバックする

### 例 5: エラーシナリオのテスト

**シナリオ**: Ralph がバックエンドのタイムアウトをグレースフルに扱うことを検証する

**カセット**（`cassettes/e2e/timeout-handling.jsonl`）:
```jsonl
{"ts":1000,"event":"ux.terminal.write","data":{"bytes":"U3RhcnRpbmc=","stdout":true,"offset_ms":0}}
{"ts":31000,"event":"ux.terminal.write","data":{"bytes":"VGltZW91dA==","stdout":true,"offset_ms":30000}}
```

**使い方**:
```bash
ralph-e2e --mock --filter timeout-handling --mock-speed 10.0
```

**期待**: テストがタイムアウト処理を検証する（10 倍速で 3 秒）

## トラブルシューティング

### 問題: 「cassette not found」エラー

**症状**:
```
Error: cassette not found for scenario 'my-test' backend 'claude'
```

**解決策**:
1. カセットファイルが存在するか確認する: `ls cassettes/e2e/my-test*.jsonl`
2. 命名規則を検証する: `<scenario-id>-<backend>.jsonl` または `<scenario-id>.jsonl`
3. 実際のバックエンドで新しいカセットを記録する
4. 汎用のカセットを使う（バックエンドの接尾辞を削除する）

### 問題: カセットの解析エラー

**症状**:
```
Error: cassette parse error in cassettes/e2e/test.jsonl
```

**解決策**:
1. JSONL 形式を検証する: `jq . cassettes/e2e/test.jsonl`
2. 末尾のカンマや無効な JSON を確認する
3. カセットを最初から再記録する

### 問題: コマンドが実行されない

**症状**: 期待した副作用（タスク、メモリ）が存在しない

**解決策**:
1. 許可リストにコマンドが含まれるか検証する: `--allow "ralph task add"`
2. カセットにコマンド付きの `bus.publish` イベントが含まれるか確認する
3. コマンドが正しい形式か確認する（シェル機能なし）
4. 詳細ログで実行し、スキップされたコマンドを確認する

### 問題: 出力が実際のバックエンドと異なる

**症状**: モックの出力が実際のバックエンドの挙動と一致しない

**解決策**:
1. 最新のバックエンドバージョンでカセットを再記録する
2. バックエンド固有のカセットを確認する: `<scenario>-<backend>.jsonl`
3. カセットが同じ環境（PTY か非 PTY か）で記録されたか検証する

### 問題: モードでは通るが実際のバックエンドで失敗する

**症状**: モックテストは通るが、実際の E2E テストが失敗する

**根本原因**: カセットが古いか、実際の挙動を反映していない

**解決策**:
1. 現在のバックエンドでカセットを再記録する
2. 実際の E2E テストを定期的に実行する（例: 夜間）
3. 速いフィードバックにはモードを、検証には実際のモードを使う

## ベストプラクティス

### モードを使う場面

✅ **モードを使う**:
- 開発中の速いフィードバック
- CI/CD パイプライン（コストと速度）
- リグレッションテスト（決定的な出力）
- エラーシナリオのテスト（タイムアウト、失敗）

❌ **モードを使わない**:
- 新しいバックエンド統合の検証
- 実際の AI 挙動の変化のテスト
- パフォーマンスのベンチマーク
- ネットワークに依存するシナリオ

### カセットの管理

1. **バージョン管理**: 再現性のためにカセットを git にコミットする
2. **命名規則**: 説明的なシナリオ ID を使う
3. **バックエンド固有**: 挙動が異なるときのみ作成する
4. **定期的な更新**: バックエンドの挙動が変わったら再記録する
5. **最小のカセット**: カセットを小さく焦点を絞って保つ

### 許可リストの安全性

1. **最小権限の原則**: 必要なコマンドのみを許可リストに入れる
2. **破壊的なコマンドなし**: `rm`、`mv` などを決して許可リストに入れない
3. **プレフィックスマッチ**: 具体的なプレフィックスを使う（`ralph` ではなく `ralph task add`）
4. **定期的なレビュー**: 不要なエントリがないか許可リストを監査する

### テスト戦略

1. **速度のためのモック**: すべてのコミットでモックテストを実行する
2. **検証のための実物**: 実際の E2E テストを夜間または週次で実行する
3. **ハイブリッドアプローチ**: ほとんどのシナリオはモック、重要な経路は実物
4. **カセットの鮮度**: カセットを四半期ごと、またはバックエンドの更新時に再記録する
