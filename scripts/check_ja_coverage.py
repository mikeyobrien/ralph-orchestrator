#!/usr/bin/env python3
"""日本語ローカライズの検証ゲート。

1. スコープ内の各英語ソース `X.md` に対し `X.ja.md` と `X.ja.html` が存在するか（1:1）。
2. 各 `X.ja.md` にコード/URL/パスを除いた「未翻訳の英語散文」が残っていないか（ヒューリスティック）。

いずれかに違反があれば非ゼロ終了する（CI/ローカルの backpressure）。

使い方（.venv 前提）:
    python scripts/check_ja_coverage.py
    python scripts/check_ja_coverage.py --strict   # 未翻訳候補も失敗扱い（既定でも失敗扱い）
"""

from __future__ import annotations

import os
import re
import sys

import ja_scope as scope

# 未翻訳判定: コード/リンク等を除いた行に、日本語文字が無く英単語が多い場合に疑う。
_JP_RE = re.compile(r"[぀-ヿ㐀-鿿ｦ-ﾟ]")
_WORD_RE = re.compile(r"[A-Za-z]{2,}")
_CODEFENCE_RE = re.compile(r"^```")
_INLINE_CODE_RE = re.compile(r"`[^`]*`")
# Markdown 画像/バッジ（![alt](url)）は装飾。alt ごと除去する。
_IMAGE_RE = re.compile(r"!\[[^\]]*\]\([^)]*\)")
_LINK_URL_RE = re.compile(r"https?://\S+|\][^)]*\)|\]\([^)]*\)")
_HTML_TAG_RE = re.compile(r"<[^>]+>")

# 英語のまま残してよい語（技術用語・コマンド・固有名詞）。行判定の許容に使う。
ALLOWLIST = {
    "ralph", "orchestrator", "claude", "kiro", "gemini", "codex", "roo",
    "yaml", "yml", "json", "jsonl", "toml", "md", "html", "cli", "tui",
    "http", "https", "url", "id", "api", "sdk", "ci", "cd", "os", "ok",
    "git", "github", "true", "false", "null", "npm", "npx", "cargo",
    "llms", "txt", "hats", "hat", "loop", "loops", "memories", "tasks",
    # サポートされるバックエンド等の固有名詞（原文維持でよい識別子）
    "kiro", "acp", "gemini", "codex", "pi", "roo", "copilot", "opencode",
    "amp", "oauth", "login", "mikeyobrien", "vercel", "fastify", "trpc",
    "sqlite", "react", "vite", "ratatui", "pty", "stdio", "rpc",
}


def _strip_noise(line: str) -> str:
    line = _IMAGE_RE.sub(" ", line)
    line = _INLINE_CODE_RE.sub(" ", line)
    line = _LINK_URL_RE.sub(" ", line)
    line = _HTML_TAG_RE.sub(" ", line)
    return line


def suspect_untranslated(md_text: str) -> list[tuple[int, str]]:
    """未翻訳の疑いがある行を (行番号, 内容) で返す。"""
    suspects: list[tuple[int, str]] = []
    in_code = False
    in_frontmatter = False
    lines = md_text.splitlines()
    for i, raw in enumerate(lines, start=1):
        # frontmatter (--- ... ---) は英語維持の決定に従い判定対象外
        if i == 1 and raw.strip() == "---":
            in_frontmatter = True
            continue
        if in_frontmatter:
            if raw.strip() == "---":
                in_frontmatter = False
            continue
        if _CODEFENCE_RE.match(raw.strip()):
            in_code = not in_code
            continue
        if in_code:
            continue
        stripped = raw.strip()
        if not stripped:
            continue
        # 見出し記号/表の罫線/箇条書き記号のみは除外
        if re.fullmatch(r"[#>*\-=|:\s\d.]+", stripped):
            continue
        text = _strip_noise(stripped)
        if _JP_RE.search(text):
            continue  # 日本語を含む行はOK
        words = [w.lower() for w in _WORD_RE.findall(text)]
        meaningful = [w for w in words if w not in ALLOWLIST]
        # 意味のある英単語が3語以上連なる＝未翻訳の散文の疑い
        if len(meaningful) >= 3:
            suspects.append((i, stripped))
    return suspects


def main(argv: list[str]) -> int:
    sources = scope.source_markdown_files()
    missing_md: list[str] = []
    missing_html: list[str] = []
    untranslated: dict[str, list[tuple[int, str]]] = {}

    for rel_md in sources:
        rel_ja = scope.ja_path_for(rel_md)
        rel_html = scope.html_path_for(rel_ja)
        abs_ja = os.path.join(scope.ROOT, rel_ja)
        abs_html = os.path.join(scope.ROOT, rel_html)
        if not os.path.isfile(abs_ja):
            missing_md.append(rel_ja)
            continue
        if not os.path.isfile(abs_html):
            missing_html.append(rel_html)
        with open(abs_ja, encoding="utf-8") as f:
            text = f.read()
        s = suspect_untranslated(text)
        if s:
            untranslated[rel_ja] = s

    total = len(sources)
    done = total - len(missing_md)
    print(f"対象ソース: {total} 件 / 日本語版あり: {done} 件")
    print(f"HTMLビューア欠落: {len(missing_html)} 件 / 未翻訳疑いファイル: {len(untranslated)} 件\n")

    if missing_md:
        print(f"[欠落: .ja.md] {len(missing_md)} 件")
        for m in missing_md:
            print(f"  - {m}")
        print()
    if missing_html:
        print(f"[欠落: .ja.html] {len(missing_html)} 件")
        for m in missing_html:
            print(f"  - {m}")
        print()
    if untranslated:
        print(f"[未翻訳の疑い] {len(untranslated)} ファイル")
        for path, rows in untranslated.items():
            print(f"  {path}:")
            for ln, txt in rows[:8]:
                shown = txt if len(txt) <= 100 else txt[:97] + "..."
                print(f"    L{ln}: {shown}")
            if len(rows) > 8:
                print(f"    … 他 {len(rows) - 8} 行")
        print()

    failed = bool(missing_md or missing_html or untranslated)
    if failed:
        print("結果: 未完了（上記を解消してください）")
        return 1
    print("結果: 合格 — 欠落なし・未翻訳なし")
    return 0


if __name__ == "__main__":
    raise SystemExit(main(sys.argv[1:]))
