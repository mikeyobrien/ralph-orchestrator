#!/usr/bin/env python3
"""日本語 Markdown (`*.ja.md`) から自己完結・インタラクティブ・レスポンシブな
HTML ビューア (`*.ja.html`) を生成する。

- 外部CDN/ネットワークを一切使わない（CSS/JS/シンタックスハイライトを全てインライン）。
- 各 `.ja.md` の隣に同名の `.ja.html` を出力する。
- ビューアUI: サイドバー目次（スクロール連動）・ライト/ダークテーマ切替（localStorage 永続化）・
  コードコピー・ページ内検索・スムーススクロール・レスポンシブ（モバイルはハンバーガー）。

使い方（.venv 前提）:
    python scripts/build_ja_viewers.py            # スコープ内の全 .ja.md を再生成
    python scripts/build_ja_viewers.py path/to/file.ja.md ...   # 個別指定
"""

from __future__ import annotations

import html as html_lib
import re
import sys

import markdown
from markdown.extensions.toc import TocExtension
from pygments.formatters import HtmlFormatter

import ja_scope as scope  # noqa: E402  (same dir on sys.path)

sys.path.insert(0, scope.ROOT + "/scripts")

# Pygments のスタイルCSS（ライト/ダーク）をビルド時にインライン化する。
_PYGMENTS_LIGHT = HtmlFormatter(style="default").get_style_defs(".codehilite")
_PYGMENTS_DARK = HtmlFormatter(style="monokai").get_style_defs(".codehilite")


_FRONTMATTER_RE = re.compile(r"\A---\n.*?\n---\n", re.DOTALL)


def strip_frontmatter(md_text: str) -> str:
    """先頭の YAML frontmatter（--- ... ---）をビューア表示から除去する。

    frontmatter は英語のトリガー用メタデータであり、閲覧者向けの本文ではないため
    HTML ビューアには描画しない（.ja.md ファイル自体には残す）。
    """
    return _FRONTMATTER_RE.sub("", md_text, count=1)


def render_markdown(md_text: str) -> str:
    """Markdown 本文を HTML へ変換する。"""
    md = markdown.Markdown(
        extensions=[
            "fenced_code",
            "codehilite",
            "tables",
            "attr_list",
            "sane_lists",
            "md_in_html",
            TocExtension(anchorlink=True, permalink=False, toc_depth="2-4"),
        ],
        extension_configs={
            "codehilite": {"guess_lang": False, "css_class": "codehilite"},
        },
    )
    body = md.convert(md_text)
    return body


# 相対リンクの張り替え: `foo.ja.md` -> `foo.ja.html`, `foo.md` -> `foo.ja.html`
_HREF_RE = re.compile(r'(<a\s[^>]*href=")([^"]+)(")', re.IGNORECASE)


def _rewrite_links(body: str) -> str:
    def repl(m: re.Match) -> str:
        pre, href, post = m.group(1), m.group(2), m.group(3)
        if "://" in href or href.startswith("#") or href.startswith("mailto:"):
            return m.group(0)
        # アンカー分離
        anchor = ""
        if "#" in href:
            href, anchor = href.split("#", 1)
            anchor = "#" + anchor
        if href.endswith(".ja.md"):
            href = href[: -len(".ja.md")] + ".ja.html"
        elif href.endswith(".md"):
            href = href[: -len(".md")] + ".ja.html"
        return f"{pre}{href}{anchor}{post}"

    return _HREF_RE.sub(repl, body)


def _extract_title(md_text: str, fallback: str) -> str:
    for line in md_text.splitlines():
        s = line.strip()
        if s.startswith("# "):
            return s[2:].strip()
    return fallback


def build_page(title: str, body_html: str) -> str:
    """レンダリング済み本文をインタラクティブ・レスポンシブな殻で包む。"""
    esc_title = html_lib.escape(title)
    return f"""<!doctype html>
<html lang="ja">
<head>
<meta charset="utf-8">
<meta name="viewport" content="width=device-width, initial-scale=1">
<title>{esc_title} — Ralph Orchestrator 日本語ドキュメント</title>
<style>
{_BASE_CSS}
/* --- Pygments (light) --- */
{_PYGMENTS_LIGHT}
/* --- Pygments (dark) --- */
:root[data-theme="dark"] .codehilite,
@media (prefers-color-scheme: dark) {{}}
</style>
<style id="pygments-dark" media="not all">
{_PYGMENTS_DARK}
</style>
</head>
<body>
<header class="topbar">
  <button id="menuBtn" class="icon-btn" aria-label="目次を開閉">☰</button>
  <span class="brand">Ralph Orchestrator <small>日本語ドキュメント</small></span>
  <div class="spacer"></div>
  <input id="search" class="search" type="search" placeholder="ページ内を検索…" aria-label="ページ内検索">
  <button id="themeBtn" class="icon-btn" aria-label="テーマ切替">🌓</button>
</header>
<div class="layout">
  <nav id="toc" class="toc" aria-label="目次"><div class="toc-title">目次</div><ul id="tocList"></ul></nav>
  <main id="content" class="content">
{body_html}
  </main>
</div>
<div id="backdrop" class="backdrop"></div>
<script>
{_BASE_JS}
</script>
</body>
</html>
"""


_BASE_CSS = r"""
:root{
  --bg:#ffffff; --fg:#1f2328; --muted:#57606a; --border:#d0d7de;
  --link:#6f42c1; --accent:#8250df; --code-bg:#f6f8fa; --topbar:#faf8ff;
  --sidebar:#fbfaff; --mark:#fff3bf;
}
:root[data-theme="dark"]{
  --bg:#0d1117; --fg:#e6edf3; --muted:#9198a1; --border:#30363d;
  --link:#d2a8ff; --accent:#bc8cff; --code-bg:#161b22; --topbar:#161b22;
  --sidebar:#0f141b; --mark:#5c4a00;
}
@media (prefers-color-scheme: dark){
  :root:not([data-theme="light"]){
    --bg:#0d1117; --fg:#e6edf3; --muted:#9198a1; --border:#30363d;
    --link:#d2a8ff; --accent:#bc8cff; --code-bg:#161b22; --topbar:#161b22;
    --sidebar:#0f141b; --mark:#5c4a00;
  }
}
*{box-sizing:border-box}
html{scroll-behavior:smooth}
body{
  margin:0; background:var(--bg); color:var(--fg);
  font-family:-apple-system,BlinkMacSystemFont,"Segoe UI","Hiragino Kaku Gothic ProN","Noto Sans JP",Meiryo,sans-serif;
  line-height:1.75; font-size:16px;
}
.topbar{
  position:sticky; top:0; z-index:30; display:flex; align-items:center; gap:.6rem;
  padding:.5rem .9rem; background:var(--topbar); border-bottom:1px solid var(--border);
  backdrop-filter:saturate(1.2) blur(4px);
}
.brand{font-weight:700} .brand small{color:var(--muted); font-weight:500; font-size:.75em}
.spacer{flex:1}
.icon-btn{
  border:1px solid var(--border); background:transparent; color:var(--fg);
  border-radius:8px; padding:.35rem .55rem; cursor:pointer; font-size:1rem; line-height:1;
}
.icon-btn:hover{border-color:var(--accent)}
.search{
  border:1px solid var(--border); background:var(--bg); color:var(--fg);
  border-radius:8px; padding:.4rem .6rem; min-width:8rem; max-width:16rem;
}
.layout{display:flex; align-items:flex-start; max-width:1200px; margin:0 auto}
.toc{
  position:sticky; top:56px; align-self:flex-start; width:270px; flex:0 0 270px;
  max-height:calc(100vh - 56px); overflow:auto; padding:1rem .75rem;
  background:var(--sidebar); border-right:1px solid var(--border);
}
.toc-title{font-size:.8rem; text-transform:uppercase; letter-spacing:.05em; color:var(--muted); margin-bottom:.5rem}
.toc ul{list-style:none; margin:0; padding:0}
.toc li{margin:.05rem 0}
.toc a{display:block; text-decoration:none; color:var(--muted); padding:.2rem .5rem; border-radius:6px; font-size:.9rem}
.toc a:hover{background:rgba(130,80,223,.1); color:var(--fg)}
.toc a.active{color:var(--accent); background:rgba(130,80,223,.14); font-weight:600}
.toc .lvl-3{padding-left:1rem} .toc .lvl-4{padding-left:1.75rem; font-size:.85rem}
.content{flex:1 1 auto; min-width:0; padding:1.5rem 2rem 6rem; max-width:820px}
.content h1{font-size:1.9rem; border-bottom:2px solid var(--border); padding-bottom:.3rem; margin-top:0}
.content h2{font-size:1.45rem; border-bottom:1px solid var(--border); padding-bottom:.25rem; margin-top:2rem}
.content h3{font-size:1.2rem; margin-top:1.6rem}
.content a{color:var(--link)}
.content code{background:var(--code-bg); padding:.15em .4em; border-radius:6px; font-size:.88em;
  font-family:"JetBrains Mono",ui-monospace,SFMono-Regular,Menlo,Consolas,monospace}
.content pre{background:var(--code-bg); border:1px solid var(--border); border-radius:10px;
  padding:1rem; overflow-x:auto; position:relative}
.content pre code{background:none; padding:0}
.content blockquote{border-left:4px solid var(--accent); margin:1rem 0; padding:.2rem 1rem; color:var(--muted)}
.content table{border-collapse:collapse; width:100%; display:block; overflow-x:auto}
.content th,.content td{border:1px solid var(--border); padding:.5rem .75rem}
.content th{background:var(--code-bg)}
.content img{max-width:100%; height:auto}
.content .headerlink{opacity:0; text-decoration:none; margin-left:.35rem; color:var(--muted)}
.content h1:hover .headerlink,.content h2:hover .headerlink,
.content h3:hover .headerlink,.content h4:hover .headerlink{opacity:1}
mark{background:var(--mark); color:inherit; border-radius:3px}
.copy-btn{
  position:absolute; top:.5rem; right:.5rem; font-size:.72rem; padding:.2rem .5rem;
  border:1px solid var(--border); background:var(--bg); color:var(--muted);
  border-radius:6px; cursor:pointer; opacity:0; transition:opacity .15s;
}
pre:hover .copy-btn{opacity:1}
.copy-btn.copied{color:var(--accent); border-color:var(--accent)}
.backdrop{display:none; position:fixed; inset:56px 0 0 0; background:rgba(0,0,0,.4); z-index:20}
@media (max-width:820px){
  .toc{position:fixed; top:56px; left:0; bottom:0; transform:translateX(-100%);
       transition:transform .2s ease; z-index:25; width:80%; max-width:320px}
  .toc.open{transform:translateX(0)}
  .backdrop.show{display:block}
  .content{padding:1.25rem 1.1rem 5rem}
  .search{min-width:5rem}
}
@media (min-width:821px){ #menuBtn{display:none} }
"""

_BASE_JS = r"""
(function(){
  var root=document.documentElement;
  // --- テーマ ---
  var darkStyle=document.getElementById('pygments-dark');
  function applyTheme(t){
    if(t==='dark'||t==='light'){root.setAttribute('data-theme',t);}
    else{root.removeAttribute('data-theme');}
    var isDark = t==='dark' || (t!=='light' && window.matchMedia('(prefers-color-scheme: dark)').matches);
    if(darkStyle){ darkStyle.media = isDark ? 'all' : 'not all'; }
  }
  var saved=localStorage.getItem('ralph-ja-theme')||'auto';
  applyTheme(saved);
  document.getElementById('themeBtn').addEventListener('click',function(){
    var cur=localStorage.getItem('ralph-ja-theme')||'auto';
    var next=cur==='auto'?'light':cur==='light'?'dark':'auto';
    localStorage.setItem('ralph-ja-theme',next); applyTheme(next);
  });
  window.matchMedia('(prefers-color-scheme: dark)').addEventListener('change',function(){
    if((localStorage.getItem('ralph-ja-theme')||'auto')==='auto')applyTheme('auto');
  });

  var content=document.getElementById('content');

  // --- 目次生成 ---
  var tocList=document.getElementById('tocList');
  var heads=content.querySelectorAll('h2, h3, h4');
  var items=[];
  heads.forEach(function(h){
    if(!h.id){h.id=(h.textContent||'').trim().replace(/\s+/g,'-');}
    var li=document.createElement('li');
    var a=document.createElement('a');
    a.href='#'+h.id; a.textContent=h.textContent.replace('¶','').trim();
    a.className='lvl-'+h.tagName.substring(1);
    li.appendChild(a); tocList.appendChild(li);
    items.push({a:a,h:h});
  });

  // --- スクロール連動ハイライト ---
  if('IntersectionObserver' in window && items.length){
    var byId={}; items.forEach(function(it){byId[it.h.id]=it.a;});
    var obs=new IntersectionObserver(function(entries){
      entries.forEach(function(e){
        if(e.isIntersecting){
          items.forEach(function(it){it.a.classList.remove('active');});
          var a=byId[e.target.id]; if(a){a.classList.add('active');
            a.scrollIntoView({block:'nearest'});}
        }
      });
    },{rootMargin:'0px 0px -75% 0px'});
    heads.forEach(function(h){obs.observe(h);});
  }

  // --- コードコピー ---
  content.querySelectorAll('pre').forEach(function(pre){
    var btn=document.createElement('button');
    btn.className='copy-btn'; btn.type='button'; btn.textContent='コピー';
    btn.addEventListener('click',function(){
      var code=pre.querySelector('code'); var text=code?code.innerText:pre.innerText;
      navigator.clipboard.writeText(text).then(function(){
        btn.textContent='コピー済み'; btn.classList.add('copied');
        setTimeout(function(){btn.textContent='コピー'; btn.classList.remove('copied');},1500);
      });
    });
    pre.appendChild(btn);
  });

  // --- モバイル目次開閉 ---
  var toc=document.getElementById('toc'), backdrop=document.getElementById('backdrop');
  function closeToc(){toc.classList.remove('open'); backdrop.classList.remove('show');}
  document.getElementById('menuBtn').addEventListener('click',function(){
    toc.classList.toggle('open'); backdrop.classList.toggle('show');
  });
  backdrop.addEventListener('click',closeToc);
  tocList.addEventListener('click',function(e){ if(e.target.tagName==='A') closeToc(); });

  // --- ページ内検索（ハイライト） ---
  var search=document.getElementById('search');
  function clearMarks(){
    content.querySelectorAll('mark[data-s]').forEach(function(m){
      var t=document.createTextNode(m.textContent); m.parentNode.replaceChild(t,m);
    });
    content.normalize();
  }
  var timer;
  search.addEventListener('input',function(){
    clearTimeout(timer);
    timer=setTimeout(function(){
      clearMarks();
      var q=search.value.trim(); if(q.length<2)return;
      var rx=new RegExp(q.replace(/[.*+?^${}()|[\]\\]/g,'\\$&'),'gi');
      var walker=document.createTreeWalker(content,NodeFilter.SHOW_TEXT,{
        acceptNode:function(n){
          if(!n.nodeValue.trim())return NodeFilter.FILTER_REJECT;
          var p=n.parentNode.nodeName;
          if(p==='SCRIPT'||p==='STYLE'||p==='MARK')return NodeFilter.FILTER_REJECT;
          return NodeFilter.FILTER_ACCEPT;
        }
      });
      var nodes=[],n; while(n=walker.nextNode())nodes.push(n);
      var first=null;
      nodes.forEach(function(node){
        if(!rx.test(node.nodeValue))return; rx.lastIndex=0;
        var span=document.createElement('span');
        span.innerHTML=node.nodeValue.replace(rx,function(m){return '<mark data-s="1">'+m+'</mark>';});
        node.parentNode.replaceChild(span,node);
        while(span.firstChild)span.parentNode.insertBefore(span.firstChild,span);
        span.parentNode.removeChild(span);
      });
      first=content.querySelector('mark[data-s]');
      if(first)first.scrollIntoView({block:'center',behavior:'smooth'});
    },200);
  });
})();
"""


def build_one(rel_ja_md: str) -> str:
    """1 つの `.ja.md` を生成し、書き込んだ HTML の相対パスを返す。"""
    import os

    abs_md = os.path.join(scope.ROOT, rel_ja_md)
    with open(abs_md, encoding="utf-8") as f:
        md_text = f.read()
    title = _extract_title(md_text, os.path.basename(rel_ja_md))
    body = _rewrite_links(render_markdown(strip_frontmatter(md_text)))
    page = build_page(title, body)
    rel_html = scope.html_path_for(rel_ja_md)
    abs_html = os.path.join(scope.ROOT, rel_html)
    with open(abs_html, "w", encoding="utf-8") as f:
        f.write(page)
    return rel_html


def main(argv: list[str]) -> int:
    import os

    if argv:
        targets = [os.path.relpath(os.path.abspath(a), scope.ROOT) for a in argv]
    else:
        targets = [
            scope.ja_path_for(s)
            for s in scope.source_markdown_files()
            if os.path.isfile(os.path.join(scope.ROOT, scope.ja_path_for(s)))
        ]
    count = 0
    for t in targets:
        if not t.endswith(".ja.md"):
            print(f"スキップ（.ja.md ではない）: {t}")
            continue
        if not os.path.isfile(os.path.join(scope.ROOT, t)):
            print(f"見つかりません: {t}")
            continue
        out = build_one(t)
        count += 1
        print(f"生成: {out}")
    print(f"\n完了: {count} 件のビューアを生成しました。")
    return 0


if __name__ == "__main__":
    import os

    raise SystemExit(main(sys.argv[1:]))
