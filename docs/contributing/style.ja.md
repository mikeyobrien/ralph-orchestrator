# コードスタイルガイド

!!! note "ドキュメント作成中"
    このページは作成中です。包括的なスタイルガイドラインは、近日中にご確認ください。

## 概要

Ralph Orchestrator は、プロジェクト固有の追加を伴いつつ Rust コミュニティの慣習に従います。

## Rust のスタイル

- [Rust API ガイドライン](https://rust-lang.github.io/api-guidelines/) に従う
- 整形には `cargo fmt` を使う
- lint には `cargo clippy` を使う

## pre-commit フック

```bash
# フックをインストールする
./scripts/setup-hooks.sh

# フックはコミット時に自動実行される（CI と同等）:
# - ./scripts/sync-embedded-files.sh check
# - cargo fmt --all -- --check
# - cargo clippy --all-targets --all-features -- -D warnings
# - cargo test
```

## ドキュメントのスタイル

- 現在形を使う（"added" ではなく "adds"）
- 行を 100 文字未満に保つ
- 公開 API には例を含める
- `mkdocs.yml` の `plugins.llmstxt.sections` を、ドキュメント IA の変更と同期させる
- llms マップの変更を `mkdocs build --strict` と
  `python scripts/validate_llms_txt.py site/llms.txt` で検証する

## 関連項目

- [開発環境のセットアップ](setup.ja.md) - 環境のセットアップ
- [テスト](testing.ja.md) - テストのガイドライン
- [PR の提出](pull-requests.ja.md) - PR のプロセス
