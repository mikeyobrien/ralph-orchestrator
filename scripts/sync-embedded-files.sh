#!/usr/bin/env bash
# Sync mirrored files for crates.io packaging and consistency
#
# Files referenced via include_str!() must be inside the crate directory to be
# included when publishing. Additionally, presets are mirrored from /presets/
# to /crates/ralph-cli/presets/ so cargo install users get the same presets.
# This script syncs source files to their crate-local copies.
#
# Usage:
#   ./scripts/sync-embedded-files.sh        # Sync files
#   ./scripts/sync-embedded-files.sh check  # Check if files are in sync (for CI)

set -euo pipefail

REPO_ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"

# Define source -> destination mappings
# Format: "source_path:dest_path"
EMBEDDED_FILES=(
    # SOPs for ralph plan/task commands
    ".claude/skills/pdd/SKILL.md:crates/ralph-cli/sops/pdd.md"
    ".claude/skills/code-task-generator/SKILL.md:crates/ralph-cli/sops/code-task-generator.md"

    # Presets (canonical -> mirror for cargo install)
    "presets/bugfix.yml:crates/ralph-cli/presets/bugfix.yml"
    "presets/code-assist.yml:crates/ralph-cli/presets/code-assist.yml"
    "presets/debug.yml:crates/ralph-cli/presets/debug.yml"
    "presets/deploy.yml:crates/ralph-cli/presets/deploy.yml"
    "presets/docs.yml:crates/ralph-cli/presets/docs.yml"
    "presets/feature.yml:crates/ralph-cli/presets/feature.yml"
    "presets/gap-analysis.yml:crates/ralph-cli/presets/gap-analysis.yml"
    "presets/hatless-baseline.yml:crates/ralph-cli/presets/hatless-baseline.yml"
    "presets/minimal/amp.yml:crates/ralph-cli/presets/minimal/amp.yml"
    "presets/minimal/builder.yml:crates/ralph-cli/presets/minimal/builder.yml"
    "presets/minimal/claude.yml:crates/ralph-cli/presets/minimal/claude.yml"
    "presets/minimal/code-assist.yml:crates/ralph-cli/presets/minimal/code-assist.yml"
    "presets/minimal/codex.yml:crates/ralph-cli/presets/minimal/codex.yml"
    "presets/minimal/gemini.yml:crates/ralph-cli/presets/minimal/gemini.yml"
    "presets/minimal/kiro.yml:crates/ralph-cli/presets/minimal/kiro.yml"
    "presets/minimal/opencode.yml:crates/ralph-cli/presets/minimal/opencode.yml"
    "presets/minimal/preset-evaluator.yml:crates/ralph-cli/presets/minimal/preset-evaluator.yml"
    "presets/minimal/smoke.yml:crates/ralph-cli/presets/minimal/smoke.yml"
    "presets/minimal/test.yml:crates/ralph-cli/presets/minimal/test.yml"
    "presets/pdd-to-code-assist.yml:crates/ralph-cli/presets/pdd-to-code-assist.yml"
    "presets/pr-review.yml:crates/ralph-cli/presets/pr-review.yml"
    "presets/refactor.yml:crates/ralph-cli/presets/refactor.yml"
    "presets/research.yml:crates/ralph-cli/presets/research.yml"
    "presets/review.yml:crates/ralph-cli/presets/review.yml"
    "presets/spec-driven.yml:crates/ralph-cli/presets/spec-driven.yml"
)

# Colors for output
RED='\033[0;31m'
GREEN='\033[0;32m'
YELLOW='\033[1;33m'
NC='\033[0m' # No Color

sync_files() {
    local changed=0

    for mapping in "${EMBEDDED_FILES[@]}"; do
        local src="${mapping%%:*}"
        local dest="${mapping##*:}"
        local src_path="$REPO_ROOT/$src"
        local dest_path="$REPO_ROOT/$dest"

        if [[ ! -f "$src_path" ]]; then
            echo -e "${RED}ERROR: Source file not found: $src${NC}"
            exit 1
        fi

        # Create destination directory if needed
        mkdir -p "$(dirname "$dest_path")"

        # Check if files differ
        if [[ ! -f "$dest_path" ]] || ! diff -q "$src_path" "$dest_path" > /dev/null 2>&1; then
            cp "$src_path" "$dest_path"
            echo -e "${GREEN}Synced: $src -> $dest${NC}"
            changed=1
        else
            echo -e "Up to date: $dest"
        fi
    done

    if [[ $changed -eq 1 ]]; then
        echo -e "\n${YELLOW}Files were synced. Don't forget to commit the changes!${NC}"
    else
        echo -e "\n${GREEN}All embedded files are up to date.${NC}"
    fi
}

check_files() {
    local out_of_sync=0

    echo "Checking embedded files are in sync..."
    echo ""

    for mapping in "${EMBEDDED_FILES[@]}"; do
        local src="${mapping%%:*}"
        local dest="${mapping##*:}"
        local src_path="$REPO_ROOT/$src"
        local dest_path="$REPO_ROOT/$dest"

        if [[ ! -f "$src_path" ]]; then
            echo -e "${RED}ERROR: Source file not found: $src${NC}"
            exit 1
        fi

        if [[ ! -f "$dest_path" ]]; then
            echo -e "${RED}MISSING: $dest${NC}"
            echo "  Source: $src"
            out_of_sync=1
        elif ! diff -q "$src_path" "$dest_path" > /dev/null 2>&1; then
            echo -e "${RED}OUT OF SYNC: $dest${NC}"
            echo "  Source: $src"
            echo "  Diff:"
            diff "$src_path" "$dest_path" | head -20 || true
            out_of_sync=1
        else
            echo -e "${GREEN}✓${NC} $dest"
        fi
    done

    echo ""

    if [[ $out_of_sync -eq 1 ]]; then
        echo -e "${RED}ERROR: Embedded files are out of sync!${NC}"
        echo ""
        echo "Run './scripts/sync-embedded-files.sh' to sync them."
        echo ""
        echo "This check exists because files referenced via include_str!()"
        echo "must be inside the crate directory to be included when publishing"
        echo "to crates.io. Source files live elsewhere for better organization,"
        echo "so we must keep copies in sync."
        exit 1
    else
        echo -e "${GREEN}All embedded files are in sync.${NC}"
    fi
}

# Main
case "${1:-sync}" in
    check)
        check_files
        ;;
    sync|"")
        sync_files
        ;;
    *)
        echo "Usage: $0 [sync|check]"
        echo "  sync  - Sync source files to crate-local copies (default)"
        echo "  check - Check if files are in sync (for CI)"
        exit 1
        ;;
esac
