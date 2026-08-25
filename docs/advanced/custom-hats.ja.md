# カスタムハットの作成

!!! note "ドキュメント作成中"
    このページは作成中です。包括的なカスタムハットのドキュメントは、近日中にご確認ください。

## 概要

カスタムハットを使うと、AI エージェントの専門的な挙動モードを定義することで、Ralph の
オーケストレーション能力を拡張できます。

## クイックスタート

```yaml
hats:
  my-custom-hat:
    emoji: "🎯"
    system_prompt: "You are a specialized agent for..."
    triggers:
      - pattern: "custom-trigger"
```

## 関連項目

- [ハットとイベント](../concepts/hats-and-events.ja.md) - 中核概念
- [プリセット](../guide/presets.ja.md) - 組み込みのハットコレクションを使う
- [アーキテクチャ](architecture.ja.md) - システム設計の概要
