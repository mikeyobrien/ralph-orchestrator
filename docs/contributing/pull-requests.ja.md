# プルリクエストの提出

!!! note "ドキュメント作成中"
    このページは作成中です。包括的な PR ガイドラインは、近日中にご確認ください。

## 概要

Ralph Orchestrator にプルリクエストを提出するためのガイドラインです。

## 提出前に

1. **テストを実行する**: `cargo test`
2. **整形を確認する**: `cargo fmt --check`
3. **clippy を実行する**: `cargo clippy`
4. 必要なら**ドキュメントを更新する**

## PR チェックリスト

- [ ] テストがローカルで通る
- [ ] コードがスタイルガイドに従っている
- [ ] ドキュメントが更新されている
- [ ] コミットメッセージが明確である
- [ ] PR の説明が変更内容を説明している

## PR テンプレート

```markdown
## Summary
Brief description of changes

## Test Plan
How to verify the changes work

## Related Issues
Fixes #123
```

## 関連項目

- [開発環境のセットアップ](setup.ja.md) - 環境のセットアップ
- [コードスタイル](style.ja.md) - スタイルガイドライン
- [テスト](testing.ja.md) - テストのガイドライン
