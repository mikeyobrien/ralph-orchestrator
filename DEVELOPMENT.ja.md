# 開発ガイド

このガイドは、Ralph Orchestrator のスペック駆動開発ワークフローを説明します。すべての
変更はスペックを通じて流れます。スペックが信頼できる唯一の情報源（source of truth）です。

## クイックスタート

### 前提条件

- [Nix](https://nixos.org/download.html) パッケージマネージャ
- [devenv](https://devenv.sh/getting-started/)
- [direnv](https://direnv.net/docs/installation.html)（任意だが推奨）

### セットアップ

**オプション A: direnv を使う（推奨）**

```bash
# リポジトリをクローンする
cd ralph-orchestrator

# direnv に開発環境を有効化させる
direnv allow

# すべてのツール（rustfmt, clippy, just など）が自動的に使えるようになる
```

**オプション B: nix develop を使う**

```bash
cd ralph-orchestrator

# 開発シェルに入る
nix develop

# このシェル内ですべてのツールが利用できる
```

**オプション C: Nix なし（非推奨）**

Nix を使えない場合は、手動で次をインストールする必要があります。
- `rustup component add rustfmt clippy` で Rust ツールチェーン
- [just](https://github.com/casey/just) コマンドランナー

### 開発ワークフロー

```bash
# git フックをインストールする（初回のみ）
./scripts/setup-hooks.sh

# すべてのチェックを実行する（fmt, lint, test）
just check

# コードを整形する
just fmt

# lint を実行する
just lint

# テストを実行する
just test
```

**重要:** pre-commit フックは、整形または lint に失敗するコミットをブロックします。
コミット前に `just check` を実行してください。

## 中核原則

> **スペックは契約であり、ドキュメントではない。** 実装はスペックに従います。スペックが
> 実装に従うのではありません。

```
スペック → レビュー → ドッグフーディング → 実装 → 検証 → 完了
```

---

## ワークフローの概要

| 変更の種類 | 入力 | プロセス | 出力 |
|-------------|-------|---------|--------|
| **新機能** | アイデア/要件 | スペック作成 → Ralph が実装 | 動作する機能 |
| **機能の変更** | スペックの更新 | ギャップ分析 → Ralph が対応 | 更新された実装 |
| **バグ修正** | バグ報告 | ISSUES.md → Ralph が修正 → スペック更新 | 修正された挙動 + リグレッションガード |

---

## 新機能のワークフロー

Ralph に新しい能力を追加するとき。

### ステップ 1: スペックを作成する

`specs/` に新しいスペックファイルを作成します。

```bash
# 命名規則: <feature-name>.spec.md
touch specs/my-feature.spec.md
```

**必須のスペック構造:**

```markdown
---
status: draft
gap_analysis: null
related:
  - other-spec.spec.md
---

# 機能名

## 概要
[この機能が何をし、なぜ必要かの 1〜2 段落の説明]

## 設計
[どう動くか、主要な決定、設定オプション]

## 受け入れ基準

### 基準名
- **前提（Given）** [前提条件]
- **操作（When）** [アクション]
- **結果（Then）** [期待される結果]

[テスト可能な各挙動について繰り返す]
```

**ガイドライン:**
- スペックにコード例を書かない（実装の詳細）
- 内部の仕組みではなく、観測可能な挙動に焦点を当てる
- 各受け入れ基準は独立してテスト可能であるべき
- 機能が相互作用する場合は `related:` スペックを参照する

### ステップ 2: スペックをドッグフーディングする

実装の前に、スペック自体を検証します。

```bash
# 初めて実装するつもりでスペックを読む
# 問いかける: 「このスペックとコードベースだけでこれを構築できるか？」
```

**チェックリスト:**
- [ ] すべての受け入れ基準がテスト可能である
- [ ] 曖昧な要件がない
- [ ] YAGNI チェック: すべての機能が実際に必要か？
- [ ] KISS チェック: これは最も単純な解決策か？

準備ができたら `status: review` に更新します。

### ステップ 3: Ralph を実行して実装する

```bash
# オプション A: 組み込みのスペック実装プロンプトを使う
ralph start --prompt prompts/implement-spec-delta.md

# オプション B: 焦点を絞った PROMPT.md を作成する
cat > /tmp/ralph-impl/PROMPT.md << 'EOF'
./specs/my-feature.spec.md のスペックを実装する

## ルール
1. コードを書く前にスペックを完全に読む
2. スペックが要求するものだけを実装する
3. 各受け入れ基準にテストを追加する
4. バックプレッシャーを実行する: cargo check && cargo test && cargo clippy

## 完了
すべての受け入れ基準が通ったら LOOP_COMPLETE を出力する。
EOF

cd /tmp/ralph-impl && ralph start
```

### ステップ 4: 実装を検証する

```bash
# すべてのテストを実行する
cargo test

# 実装を手作業でドッグフーディングする
# ハッピーパス、エラーケース、エッジケースを試す
```

### ステップ 5: スペックのステータスを更新する

```yaml
---
status: implemented
gap_analysis: 2026-01-14
---
```

---

## 機能変更のワークフロー

既存の機能を更新するとき。

### ステップ 1: スペックを更新する

新しく望む挙動を反映するようスペックを変更します。

```bash
# スペックを編集する
vim specs/existing-feature.spec.md

# 受け入れ基準を追加/変更する
# アーキテクチャが変わる場合は設計セクションを更新する
```

### ステップ 2: ギャップ分析を実行する

ギャップ分析は、スペックと実装の差異を特定します。

```bash
# オプション A: 完全な自動ギャップ分析
ralph start --prompt prompts/spec-sync.md

# オプション B: Ralph を使った手動ギャップ分析
cat > /tmp/ralph-gap/PROMPT.md << 'EOF'
スペックと実装の間のギャップ分析を行う。

## プロセス
1. ./specs/ の status != draft のすべてのスペックを読む
2. 各受け入れ基準について、実装が存在することを検証する
3. ギャップを GAPS.md に記録する

## ギャップの分類
- **Breaking**: スペックは X と言うが、コードは Y を行う
- **Missing**: スペックは機能を記述しているが、実装がない
- **Incomplete**: 機能は存在するがスペックに一致しない
- **Untested**: 挙動は存在するがテストがない

## 出力
発見事項を GAPS.md に作成/更新し、その後 LOOP_COMPLETE。
EOF

cd /tmp/ralph-gap && ralph start
```

### ステップ 3: GAPS.md をレビューする

ギャップ分析の後、出力をレビューします。

```markdown
# GAPS.md の構造
## 概要
| 優先度 | 問題 | スペック | ステータス |
|----------|-------|------|--------|
| P0 | 重大なバグ | spec.md | NEW |
| P1 | 欠落した機能 | spec.md | TODO |
| P2 | 軽微な問題 | spec.md | BACKLOG |

## 詳細
[各ギャップの詳細な説明]
```

**優先度レベル:**
- **P0**: 破壊的変更または重大なバグ — 直ちに修正する
- **P1**: 欠落した必須機能 — リリース前に修正する
- **P2**: 軽微なギャップ — 都合のよいときに対応する
- **P3**: あると望ましい — 将来の拡張

### ステップ 4: Ralph を実行してギャップに対応する

```bash
cat > /tmp/ralph-fix/PROMPT.md << 'EOF'
GAPS.md で特定されたギャップに対応する

## 優先順位
1. まずすべての P0（破壊的）問題を修正する
2. 次に P1（欠落）機能
3. 各修正の後にバックプレッシャー: cargo check && cargo test

## プロセス
- ギャップの説明を読む
- 関連するスペックのセクションを見つける
- 修正を実装する
- テストを追加/更新する
- ギャップを解決済みとしてマークする

## 完了
すべての P0 と P1 のギャップが解決したら LOOP_COMPLETE。
EOF

cd /tmp/ralph-fix && ralph start
```

### ステップ 5: ギャップ分析の日付を更新する

```yaml
---
status: implemented
gap_analysis: 2026-01-14  # 今日の日付
---
```

---

## バグ修正のワークフロー

報告された問題を修正するとき。

### ステップ 1: ISSUES.md に記録する

バグを `ISSUES.md` に追加します。

```markdown
## アクティブな問題

### [BUG-001] 簡単な説明
- **報告日（Reported）**: 2026-01-14
- **深刻度（Severity）**: P0/P1/P2
- **症状（Symptoms）**: ユーザーが観測すること
- **期待（Expected）**: 本来起こるべきこと
- **スペック参照（Spec reference）**: 正しい挙動を定義するスペック（あれば）
- **ステータス（Status）**: NEW → IN_PROGRESS → FIXED → VERIFIED
```

### ステップ 2: Ralph を実行して修正する

```bash
cat > /tmp/ralph-bugfix/PROMPT.md << 'EOF'
ISSUES.md に記述されたバグを修正する: [BUG-001]

## プロセス
1. 問題の説明を読む
2. バグを再現する（失敗するテストを書く）
3. 根本原因を見つける
4. コードを修正する
5. テストが通ることを検証する
6. 完全なテストスイートを実行する

## 重要
- 修正の前に、失敗するテストが必ず存在すること
- これがリグレッションを防ぐ

## 完了
バグが修正され、かつテストが通ったら LOOP_COMPLETE。
EOF

cd /tmp/ralph-bugfix && ralph start
```

### ステップ 3: リグレッション防止のためにスペックを更新する

**重要**: スペックが正しい挙動を捉えていることを確認します。

```bash
# この挙動のスペックが存在するか確認する
grep -r "relevant keyword" specs/

# スペックは存在するがバグのケースをカバーしていない場合:
# スペックに受け入れ基準を追加する

# スペックが存在しない場合:
# これがスペックに値するのか、単にテストだけで十分かを検討する
```

**スペックに追加する:**

```markdown
### エッジケース: [バグの説明]
- **前提（Given）** [バグを引き起こした条件]
- **操作（When）** [バグを露出させたアクション]
- **結果（Then）** [バグではなく、正しい挙動]
```

### ステップ 4: ISSUES.md を更新する

```markdown
### [BUG-001] 簡単な説明
- **ステータス（Status）**: VERIFIED
- **解決（Resolution）**: コミット abc123 で修正
- **リグレッションテスト（Regression test）**: spec-name.spec.md に追加
```

---

## クイックリファレンス

### コマンド

```bash
# 新機能の実装
ralph start --prompt prompts/implement-spec-delta.md

# 完全なギャップ分析
ralph start --prompt prompts/spec-sync.md

# スペックのステータスを確認する
grep -r "^status:" specs/*.spec.md

# ギャップ分析が未実施のスペックを探す
grep -l "gap_analysis: null" specs/*.spec.md
```

### スペックのステータスのライフサイクル

```
draft → review → approved → implemented → deprecated
  │        │         │            │
  │        │         │            └─ 定期的にギャップ分析を実行する
  │        │         └─ Ralph が実装する
  │        └─ ドッグフーディングして洗練する
  └─ 初期作成
```

### ファイルの場所

| ファイル | 用途 |
|------|---------|
| `specs/*.spec.md` | 機能の仕様 |
| `ISSUES.md` | バグ追跡とギャップ分析の結果 |
| `GAPS.md` | ギャップ分析の実行結果 |
| `prompts/spec-sync.md` | 完全なギャップ分析用の Ralph プロンプト |
| `prompts/implement-spec-delta.md` | スペック実装用の Ralph プロンプト |
| `CLAUDE.md` | エージェントの指示（ドッグフーディングのプロセス） |

### バックプレッシャーのコマンド

コード変更後は必ず実行します。

```bash
cargo check           # 型チェック
cargo test            # テストの実行
cargo clippy -- -D warnings  # lint
```

---

## アンチパターン

### ❌ スペックなしの実装
```
悪い:  「ちょっとこの機能をサッと追加しちゃおう」
良い:  「まずスペックを作ろう」
```

### ❌ 実装後のスペック
```
悪い:  「作ったから、これから文書化しよう」
良い:  「スペックが挙動を定義し、実装がそれに従う」
```

### ❌ ギャップ分析の省略
```
悪い:  「スペックを更新したから、たぶん大丈夫」
良い:  「ギャップ分析を実行して、実装が一致することを検証する」
```

### ❌ リグレッションテストなしのバグ修正
```
悪い:  「バグを直した、次へ」
良い:  「バグを直し、テストを追加し、スペックを更新した」
```

### ❌ 過剰な作り込み
```
悪い:  「ついでにこれもリファクタリングしよう…」
良い:  「スペック/issue が要求するものだけを直す」
```

---

## Ralph ループでのワークフロー

### 分離されたディレクトリで Ralph を実行する

**重要**: ワークスペースを汚さないよう、Ralph ループは必ず一時ディレクトリで実行します。

```bash
# 分離されたワークスペースを作成する
WORK_DIR=$(mktemp -d)
cp -r . "$WORK_DIR"
cd "$WORK_DIR"

# Ralph を実行する
ralph start

# 変更をレビューし、欲しいものをチェリーピックする
```

### 並列ワークフロー

大規模なギャップ分析では、複数の Ralph インスタンスを実行します。

```bash
# ターミナル 1: P0 の問題を修正する
WORK1=$(mktemp -d) && cp -r . "$WORK1" && cd "$WORK1"
ralph start --prompt "Fix P0 gaps from GAPS.md"

# ターミナル 2: P1 の問題を修正する（独立）
WORK2=$(mktemp -d) && cp -r . "$WORK2" && cd "$WORK2"
ralph start --prompt "Fix P1 gaps from GAPS.md"
```

---

## 挙動の検証

重要な挙動については、挙動検証カタログを使います。

```bash
# 特定の挙動を検証する
ralph /verify-behaviors --category planner

# 単一の挙動を検証する
ralph /verify-behaviors --id PL-007

# スペック変更後に挙動カタログを更新する
ralph /update-behaviors
```

完全なカタログは `specs/behavioral-verification.spec.md` を参照してください。

---

## 付録: スペックテンプレート

```markdown
---
status: draft
gap_analysis: null
related: []
---

# 機能名

## 概要

[機能とその目的の簡単な説明]

## 設計

### 設定

[設定オプションがあれば]

### 挙動

[機能がどう動くか]

## 受け入れ基準

### ハッピーパス
- **前提（Given）** [前提条件]
- **操作（When）** [アクション]
- **結果（Then）** [期待される結果]

### エラー処理
- **前提（Given）** [エラー条件]
- **操作（When）** [アクション]
- **結果（Then）** [グレースフルな処理]

### エッジケース
- **前提（Given）** [エッジケース]
- **操作（When）** [アクション]
- **結果（Then）** [正しい挙動]
```
