#!/bin/bash
set -euo pipefail

# Global Codebase Rename: hats -> hats
# This script performs the mechanical transformation.
# Backpressure gate: cargo test must pass after execution.

cd "$(git rev-parse --show-toplevel)"

echo "=== Phase 1: Rename crate directories ==="
for dir in crates/hats-*; do
    new_dir="${dir/hats-/hats-}"
    echo "  git mv $dir -> $new_dir"
    git mv "$dir" "$new_dir"
done

echo "=== Phase 2: Rename special files ==="
# hatless.rs -> hatless.rs
if [ -f crates/hats-core/src/hatless.rs ]; then
    echo "  git mv hatless.rs -> hatless.rs"
    git mv crates/hats-core/src/hatless.rs crates/hats-core/src/hatless.rs
fi

# hats-tools.md -> hats-tools.md
if [ -f crates/hats-core/data/hats-tools.md ]; then
    echo "  git mv hats-tools.md -> hats-tools.md"
    git mv crates/hats-core/data/hats-tools.md crates/hats-core/data/hats-tools.md
fi

echo "=== Phase 3: Find-replace in source files ==="
# Order matters: do longer/more specific patterns first to avoid double-replacement

# Files to process (exclude .git, target, .hats working dir)
find_files() {
    find . \( -name .git -o -name target -o -path './.hats' \) -prune -o \
        \( -name '*.rs' -o -name '*.toml' -o -name '*.yml' -o -name '*.yaml' \
           -o -name '*.md' -o -name '*.json' -o -name '*.sh' -o -name '*.html' \
           -o -name '*.js' -o -name '*.ts' -o -name '*.css' -o -name '*.lock' \
           -o -name '*.txt' -o -name 'Makefile' -o -name '.gitignore' \) \
        -type f -print
}

# Phase 3a: npm scope
echo "  @hats -> @hats"
find_files | xargs sed -i 's/@hats/@hats/g'

# Phase 3b: GitHub repo URL (before general hats replacement)
echo "  hats (in URLs/names) -> hats"
find_files | xargs sed -i 's|hats|hats|g'

# Phase 3c: hatless (before general ralph_ replacement)
echo "  hatless -> hatless"
find_files | xargs sed -i 's/hatless/hatless/g'

# Phase 3d: Crate names with hyphens (hats-core -> hats-core etc.)
echo "  hats-proto -> hats-proto (and all other crate names)"
for crate in proto core adapters tui cli bench e2e telegram; do
    find_files | xargs sed -i "s/hats-${crate}/hats-${crate}/g"
done

# Phase 3e: Rust module/crate names with underscores
echo "  hats_proto -> hats_proto (and all other module names)"
for crate in proto core adapters tui cli bench e2e telegram; do
    find_files | xargs sed -i "s/ralph_${crate}/hats_${crate}/g"
done

# Phase 3f: HATS_ env vars -> HATS_
echo "  HATS_ -> HATS_ (env vars)"
find_files | xargs sed -i 's/HATS_/HATS_/g'

# Phase 3g: .hats directory -> .hats
echo '  ".hats" -> ".hats" (working directory)'
find_files | xargs sed -i 's/\.hats/\.hats/g'

# Phase 3h: hats.yml -> hats.yml (config files)
echo "  hats.yml -> hats.yml"
find_files | xargs sed -i 's/hats\.yml/hats\.yml/g'
find_files | xargs sed -i 's/hats\.pi\.yml/hats\.pi\.yml/g'
find_files | xargs sed -i 's/hats\.bot\.yml/hats\.bot\.yml/g'

# Phase 3i: Binary name "hats" in bin declarations and user-facing strings
echo '  [[bin]] name = "hats" -> "hats"'
find_files | xargs sed -i 's/name = "hats"/name = "hats"/g'

# Phase 3j: Standalone "hats" references in user-facing text
# Be surgical: "hats run" -> "hats run", "hats init" -> "hats init", etc.
echo "  hats <subcommand> -> hats <subcommand>"
for cmd in run init plan doctor preflight web bot clean task emit hats completions continue code-task; do
    find_files | xargs sed -i "s/hats ${cmd}/hats ${cmd}/g"
    find_files | xargs sed -i "s/\`hats ${cmd}/\`hats ${cmd}/g"
done

# Phase 3k: "Hats" -> "Hats"
echo '  "Hats" -> "Hats"'
find_files | xargs sed -i 's/Hats/Hats/g'

# Phase 3l: "Hats" standalone (in descriptions, docs)
# Be careful not to break "Hats Wiggum" historical references
echo '  "Hats" -> "Hats" (standalone, not "Hats Wiggum")'
find_files | xargs sed -i 's/\bRalph\b/Hats/g'

# Phase 3m: Remaining standalone "hats" that aren't part of compound words
# This catches things like "hats is" -> "hats is", "by hats" -> "by hats"
echo '  Remaining standalone hats -> hats'
find_files | xargs sed -i 's/\bralph\b/hats/g'

echo "=== Phase 4: Rename config files on disk ==="
# Rename any hats.yml/hats.pi.yml in the repo
for f in $(find . -name 'hats.yml' -o -name 'hats.pi.yml' -o -name 'hats.bot.yml' | grep -v .git | grep -v target); do
    new_f="${f/hats/hats}"
    if [ -f "$f" ]; then
        echo "  git mv $f -> $new_f"
        git mv "$f" "$new_f" 2>/dev/null || mv "$f" "$new_f"
    fi
done

echo "=== Phase 5: Rename test event_loop_ralph.rs ==="
if [ -f crates/hats-core/tests/event_loop_ralph.rs ]; then
    echo "  git mv event_loop_ralph.rs -> event_loop_hats.rs"
    git mv crates/hats-core/tests/event_loop_ralph.rs crates/hats-core/tests/event_loop_hats.rs
fi

echo "=== Done! Run 'cargo build' and 'cargo test' to verify ==="
