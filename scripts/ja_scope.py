"""日本語ローカライズ対象スコープの定義（唯一の真実）。

ユーザー向けドキュメントのみを対象とする。`.ralph/` などのランタイム状態や
`specs/` などの内部設計メモ、エージェント向けの内部文書は対象外。

build_ja_viewers.py と check_ja_coverage.py の両方がこのリストを参照する。
"""

from __future__ import annotations

import glob
import os

# リポジトリルート（このファイルは <root>/scripts/ に置かれる）
ROOT = os.path.dirname(os.path.dirname(os.path.abspath(__file__)))

# 対象ディレクトリの再帰 glob（ルートからの相対）
SCOPE_GLOBS = [
    "docs/**/*.md",
    "skills/**/*.md",
    "examples/*.md",
]

# 対象とするルート直下の主要文書（明示列挙）
ROOT_DOCS = [
    "README.md",
    "CONTRIBUTING.md",
    "CODE_OF_CONDUCT.md",
    "DEVELOPMENT.md",
    "CHANGELOG.md",
]


def _is_generated(rel: str) -> bool:
    """生成物（.ja.md）自体はソースから除外する。"""
    return rel.endswith(".ja.md")


def source_markdown_files() -> list[str]:
    """翻訳対象となる英語ソース .md のリポジトリ相対パス一覧を返す。"""
    found: set[str] = set()
    for pattern in SCOPE_GLOBS:
        for path in glob.glob(os.path.join(ROOT, pattern), recursive=True):
            rel = os.path.relpath(path, ROOT)
            if not _is_generated(rel) and os.path.isfile(path):
                found.add(rel)
    for rel in ROOT_DOCS:
        if os.path.isfile(os.path.join(ROOT, rel)):
            found.add(rel)
    return sorted(found)


def ja_path_for(rel_md: str) -> str:
    """英語ソース `dir/X.md` に対応する日本語版 `dir/X.ja.md` を返す。"""
    assert rel_md.endswith(".md") and not rel_md.endswith(".ja.md")
    return rel_md[: -len(".md")] + ".ja.md"


def html_path_for(rel_ja_md: str) -> str:
    """日本語版 `dir/X.ja.md` に対応するビューア `dir/X.ja.html` を返す。"""
    assert rel_ja_md.endswith(".ja.md")
    return rel_ja_md[: -len(".ja.md")] + ".ja.html"


if __name__ == "__main__":
    files = source_markdown_files()
    print(f"対象ソース Markdown: {len(files)} 件")
    for f in files:
        print(f"  {f}")
