# CLI ツールの例

ファイル整理のための Python 製 CLI ツールを作成してください。

## 要件

1. argparse を使ったコマンドラインインターフェース
2. コマンド:
   - `organize photos` - 撮影日で写真を整理する
   - `organize documents` - ドキュメントを種類別に振り分ける
   - `organize downloads` - ダウンロードフォルダを整理する
   - `organize --custom <pattern>` - カスタム整理ルール

3. 機能:
   - 変更をプレビューするドライラン モード
   - 取り消し（Undo）機能
   - 大きな操作向けのプログレスバー
   - 設定ファイルのサポート（`~/.file_organizer.yml`）
   - ファイルへのロギング

4. 整理ルール:
   - 写真: EXIF データに基づく 年/月 フォルダ
   - ドキュメント: 拡張子別のフォルダ（`pdf/`, `docx/`, `txt/`）
   - ダウンロード: 古いファイルをアーカイブし、種類別にまとめる
   - カスタム: ユーザー定義のパターン

5. 安全性:
   - ファイルを決して削除しない
   - 移動前にバックアップを作成する
   - 重複するファイル名を処理する
   - ファイルのパーミッションを維持する

`file_organizer.py` として保存し、補助モジュールを添えてください。
- `organizers/photo_organizer.py`
- `organizers/document_organizer.py`
- `utils/config.py`
- `utils/backup.py`

依存関係を記した `requirements.txt` を含めてください。

オーケストレーターは、すべてのコンポーネントが実装されテストされるまでイテレーションを
継続します。
