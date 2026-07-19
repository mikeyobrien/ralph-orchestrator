# ループ検出

## 概要

Ralph Orchestrator は、エージェントが反復的なサイクルで行き詰まるのを防ぐため、自動の
ループ検出を含んでいます。この機能は、ファジー文字列マッチを使って最近のエージェント出力を
比較し、エージェントが似た応答を繰り返し生成しているときを検出します。

## 仕組み

`SafetyGuard` クラスは、直近 5 つのエージェント出力のスライディングウィンドウを維持します。
各成功イテレーションの後、現在の出力が、高速なファジー文字列マッチライブラリ
[rapidfuzz](https://github.com/rapidfuzz/RapidFuzz) を使ってこの履歴と比較されます。

### 検出アルゴリズム

1. 各成功イテレーションの後、エージェントの出力が捕捉される
2. 出力が、保存された直近 5 つの出力と比較される
3. いずれかの比較が 90% の類似度しきい値を超えると、ループが検出される
4. 現在の出力が履歴に追加される（容量に達していれば最古のものが削除される）
5. ループが検出されると、オーケストレーターは警告を記録して終了する

#### スライディングウィンドウの図

```
                                         🔄 Sliding Window (deque maxlen=5)

┌────────────┐     ┌──────────┐     ┌──────────┐     ┌──────────┐     ┌──────────┐     ┌──────────┐  evicted   ╭───╮
│ New Output │ ──> │ Output 5 │ ──> │ Output 4 │ ──> │ Output 3 │ ──> │ Output 2 │ ──> │ Output 1 │ ─────────> │ X │
└────────────┘     └──────────┘     └──────────┘     └──────────┘     └──────────┘     └──────────┘            ╰───╯
```

<details>
<summary>graph-easy のソース</summary>

```
graph { label: "🔄 Sliding Window (deque maxlen=5)"; flow: east; }
[ New Output ] -> [ Output 5 ] -> [ Output 4 ] -> [ Output 3 ] -> [ Output 2 ] -> [ Output 1 ]
[ Output 1 ] -- evicted --> [ X ] { shape: rounded; }
```

</details>

### 類似度しきい値

既定のしきい値は **90% の類似度**（0.9 の比率）です。これは業界のベストプラクティスに
基づいて選ばれました。

- **0.95**: 厳しすぎる - ほぼ同一の出力しか捕捉しない
- **0.90**: バランスが取れている - バリエーションを許しつつ反復的なパターンを捕捉する（推奨）
- **0.85**: 緩い - 誤検出率が高い

#### 決定フロー

```
              🔍 Loop Detection Decision

                         ╭────────────────────╮
                         │   Current Output   │
                         ╰────────────────────╯
                           │
                           │
                           ∨
                         ┌────────────────────┐
                         │ Compare to History │ <┐
                         └────────────────────┘  │
                           │                     │
                           │                     │
                           ∨                     │
╔═══════════════╗  yes   ┌────────────────────┐  │
║ LOOP DETECTED ║ <───── │   ratio >= 90%?    │  │ yes
╚═══════════════╝        └────────────────────┘  │
                           │                     │
                           │ no                  │
                           ∨                     │
                         ┌────────────────────┐  │
                         │   More outputs?    │ ─┘
                         └────────────────────┘
                           │
                           │ no
                           ∨
                         ┌────────────────────┐
                         │   Add to History   │
                         └────────────────────┘
                           │
                           │
                           ∨
                         ╭────────────────────╮
                         │      Continue      │
                         ╰────────────────────╯
```

<details>
<summary>graph-easy のソース</summary>

```
graph { label: "🔍 Loop Detection Decision"; flow: south; }
[ Current Output ] { shape: rounded; } -> [ Compare to History ]
[ Compare to History ] -> [ ratio >= 90%? ]
[ ratio >= 90%? ] -- yes --> [ LOOP DETECTED ] { border: double; }
[ ratio >= 90%? ] -- no --> [ More outputs? ]
[ More outputs? ] -- yes --> [ Compare to History ]
[ More outputs? ] -- no --> [ Add to History ]
[ Add to History ] -> [ Continue ] { shape: rounded; }
```

</details>

## 例

```python
# Example of how loop detection works internally

from rapidfuzz import fuzz

# Agent outputs from iterations 1-3
outputs = [
    "Let me check the database for user information...",
    "I'll query the database to find the user data...",
    "Checking the database for user information...",  # Similar to #1
]

# Similarity check
ratio = fuzz.ratio(outputs[0], outputs[2]) / 100.0
# Result: ~0.91 (91% similar) - LOOP DETECTED
```

## 設定

現在、ループ検出は固定のパラメータを使います。

| パラメータ        | 値 | 説明                          |
| ---------------- | ----- | ------------------------------------ |
| `loop_threshold` | 0.9   | 類似度しきい値（90%）           |
| `recent_outputs` | 5     | 比較する出力の数 |

将来のバージョンでは、これらを設定オプションとして公開するかもしれません。

## 他の安全機能との相互作用

ループ検出は、他の安全機構と並行して動作します。

1. **イテレーション上限**: 最大イテレーション（既定: 100）
2. **実行時間上限**: 最大時間（既定: 4 時間）
3. **コスト上限**: 最大コスト（既定: $10）
4. **連続失敗の上限**: 連続失敗の最大（既定: 5）
5. **ループ検出**: 類似度ベースの出力比較

オーケストレーターは、これらの条件の**いずれか**が満たされると終了します。

### 統合アーキテクチャ

次の図は、ループ検出がメインのオーケストレーションループとどう統合されるかを示します。

```
            ⚙️ SafetyGuard in Orchestration Loop

                               ╭─────────────────────╮
  ┌──────────────────────────> │   Start Iteration   │ <┐
  │                            ╰─────────────────────╯  │
  │                              │                      │
  │                              │                      │
  │                              ∨                      │
  │                            ┌─────────────────────┐  │
  │                            │ SafetyGuard.check() │  │
  │                            └─────────────────────┘  │
  │                              │                      │
  │                              │                      │
  │                              ∨                      │
  │  ╔════════════════╗  no    ┌─────────────────────┐  │
  │  ║  STOP: Limit   ║ <───── │     Limits OK?      │  │
  │  ╚════════════════╝        └─────────────────────┘  │
  │                              │                      │
  │                              │ yes                  │
  │                              ∨                      │
  │                            ┌─────────────────────┐  │
  │                            │  Check Completion   │  │
  │                            └─────────────────────┘  │
  │                              │                      │
  │                              │                      │
  │                              ∨                      │
  │  ╔════════════════╗  yes   ┌─────────────────────┐  │
  │  ║   STOP: Done   ║ <───── │   TASK_COMPLETE?    │  │ no
  │  ╚════════════════╝        └─────────────────────┘  │
  │                              │                      │
  └────┐                         │ no                   │
       │                         ∨                      │
       │                       ┌─────────────────────┐  │
       │                       │    Execute Agent    │  │
       │                       └─────────────────────┘  │
       │                         │                      │
       │                         │                      │
       │                         ∨                      │
     ┌────────────────┐  no    ┌─────────────────────┐  │
     │ Handle Failure │ <───── │      Success?       │  │
     └────────────────┘        └─────────────────────┘  │
                                 │                      │
                                 │ yes                  │
                                 ∨                      │
                               ┌─────────────────────┐  │
                               │    detect_loop()    │  │
                               └─────────────────────┘  │
                                 │                      │
                                 │                      │
                                 ∨                      │
                               ┌─────────────────────┐  │
                               │     Loop Found?     │ ─┘
                               └─────────────────────┘
                                 │
                                 │ yes
                                 ∨
                               ╔═════════════════════╗
                               ║     STOP: Loop      ║
                               ╚═════════════════════╝
```

<details>
<summary>graph-easy のソース</summary>

```
graph { label: "⚙️ SafetyGuard in Orchestration Loop"; flow: south; }
[ Start Iteration ] { shape: rounded; } -> [ SafetyGuard.check() ]
[ SafetyGuard.check() ] -> [ Limits OK? ]
[ Limits OK? ] -- no --> [ STOP: Limit ] { border: double; }
[ Limits OK? ] -- yes --> [ Check Completion ]
[ Check Completion ] -> [ TASK_COMPLETE? ]
[ TASK_COMPLETE? ] -- yes --> [ STOP: Done ] { border: double; }
[ TASK_COMPLETE? ] -- no --> [ Execute Agent ]
[ Execute Agent ] -> [ Success? ]
[ Success? ] -- no --> [ Handle Failure ]
[ Handle Failure ] -> [ Start Iteration ]
[ Success? ] -- yes --> [ detect_loop() ]
[ detect_loop() ] -> [ Loop Found? ]
[ Loop Found? ] -- yes --> [ STOP: Loop ] { border: double; }
[ Loop Found? ] -- no --> [ Start Iteration ]
```

</details>

## ループ検出が発動するとき

ループ検出は、次のシナリオで役立ちます。

- **エージェントが同じタスクで行き詰まる**: 同じアクションを繰り返し試みる
- **振動**: エージェントが 2 つの似たアプローチの間で切り替わる
- **API エラー**: 一貫したリトライメッセージ
- **プレースホルダーの応答**: エージェントが似た「作業中」メッセージを返す

## ロギング

ループ検出が発動すると、次が表示されます。

```
WARNING - Loop detected: 92.3% similarity to previous output
WARNING - Breaking loop due to repetitive agent outputs
```

## ループ検出のリセット

ループ検出の履歴は、次のときに自動的にクリアされます。

- 新しいオーケストレーションセッションが始まる
- `SafetyGuard.reset()` が呼ばれる
- オーケストレーターが完了する（成功または失敗）

## 依存

ループ検出には `rapidfuzz` パッケージが必要です。

```bash
pip install "rapidfuzz>=3.0.0,<4.0.0"
```

rapidfuzz がインストールされていない場合、ループ検出はデバッグログメッセージとともに
グレースフルにスキップされます。

## ベストプラクティス

1. **ループを監視する**: ログでループ検出の警告に注意する
2. **プロンプトを改善する**: ループが頻繁に起きる場合は、タスクの説明を洗練する
3. **タスクの完全性を確認する**: タスクに明確な完了基準があることを確認する
4. **完了マーカーを使う**: 完了したら `- [x] TASK_COMPLETE` を追加する

## 関連トピック

- [安全機構](../guide/overview.ja.md#safety-features)
- [トラブルシューティング](../reference/troubleshooting.ja.md)
- [用語集: ループ検出](../reference/glossary.ja.md#l)
