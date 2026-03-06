#!/usr/bin/env bash
# Sync embedded assets for crates.io packaging and consistency.
#
# Files referenced via include_str!() must live inside the crate directory to be
# included when publishing. Most embedded assets are mirrored from local source
# files. The PDD SOP is generated from its canonical GitHub source plus a small
# Ralph-specific addendum.
#
# Usage:
#   ./scripts/sync-embedded-files.sh                   # Sync files
#   ./scripts/sync-embedded-files.sh check             # Check if files are in sync (for CI)
#   ./scripts/sync-embedded-files.sh update-pdd-ref    # Pin PDD to latest upstream main and resync
#   ./scripts/sync-embedded-files.sh update-pdd-ref <branch>

set -euo pipefail

REPO_ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
PDD_SOURCE_CONFIG="crates/ralph-cli/sops/upstream/pdd.env"
PDD_ADDENDUM="crates/ralph-cli/sops/addendums/pdd-ralph.md"
PDD_DEST="crates/ralph-cli/sops/pdd.md"

# Define source -> destination mappings for direct file mirrors.
# Format: "source_path:dest_path"
MIRRORED_FILES=(
    # SOPs for ralph plan/task commands
    ".claude/skills/code-task-generator/SKILL.md:crates/ralph-cli/sops/code-task-generator.md"

    # Presets (canonical -> mirror for cargo install)
    "presets/bugfix.yml:crates/ralph-cli/presets/bugfix.yml"
    "presets/code-assist.yml:crates/ralph-cli/presets/code-assist.yml"
    "presets/debug.yml:crates/ralph-cli/presets/debug.yml"
    "presets/deploy.yml:crates/ralph-cli/presets/deploy.yml"
    "presets/docs.yml:crates/ralph-cli/presets/docs.yml"
    "presets/feature.yml:crates/ralph-cli/presets/feature.yml"
    "presets/fresh-eyes.yml:crates/ralph-cli/presets/fresh-eyes.yml"
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

require_command() {
    local command_name="$1"
    if ! command -v "$command_name" > /dev/null 2>&1; then
        echo -e "${RED}ERROR: Required command not found: $command_name${NC}"
        exit 1
    fi
}

load_pdd_source_config() {
    local config_path="$REPO_ROOT/$PDD_SOURCE_CONFIG"

    if [[ ! -f "$config_path" ]]; then
        echo -e "${RED}ERROR: PDD source config not found: $PDD_SOURCE_CONFIG${NC}"
        exit 1
    fi

    # shellcheck disable=SC1090
    source "$config_path"

    : "${PDD_UPSTREAM_REPO:?PDD_UPSTREAM_REPO must be set in $PDD_SOURCE_CONFIG}"
    : "${PDD_UPSTREAM_REF:?PDD_UPSTREAM_REF must be set in $PDD_SOURCE_CONFIG}"
    : "${PDD_UPSTREAM_PATH:?PDD_UPSTREAM_PATH must be set in $PDD_SOURCE_CONFIG}"
}

pdd_raw_url() {
    load_pdd_source_config
    printf 'https://raw.githubusercontent.com/%s/%s/%s' \
        "$PDD_UPSTREAM_REPO" "$PDD_UPSTREAM_REF" "$PDD_UPSTREAM_PATH"
}

pdd_blob_url() {
    load_pdd_source_config
    printf 'https://github.com/%s/blob/%s/%s' \
        "$PDD_UPSTREAM_REPO" "$PDD_UPSTREAM_REF" "$PDD_UPSTREAM_PATH"
}

resolve_latest_pdd_ref() {
    local branch="${1:-main}"

    require_command git
    load_pdd_source_config

    local resolved_ref
    resolved_ref="$(git ls-remote "https://github.com/${PDD_UPSTREAM_REPO}.git" "refs/heads/${branch}" | awk 'NR==1 {print $1}')"

    if [[ -z "$resolved_ref" ]]; then
        echo -e "${RED}ERROR: Failed to resolve upstream ref for ${PDD_UPSTREAM_REPO}@${branch}${NC}"
        exit 1
    fi

    printf '%s' "$resolved_ref"
}

write_pdd_source_config() {
    local ref="$1"
    local config_path="$REPO_ROOT/$PDD_SOURCE_CONFIG"

    load_pdd_source_config

    mkdir -p "$(dirname "$config_path")"
    cat > "$config_path" <<EOF
# Canonical upstream source for the bundled Ralph PDD SOP.
# Generated/updated via:
#   ./scripts/sync-embedded-files.sh update-pdd-ref [branch]
# Resync after manual edits with:
#   ./scripts/sync-embedded-files.sh

PDD_UPSTREAM_REPO="$PDD_UPSTREAM_REPO"
PDD_UPSTREAM_REF="$ref"
PDD_UPSTREAM_PATH="$PDD_UPSTREAM_PATH"
EOF
}

generate_pdd_sop() {
    local output_path="$1"
    local upstream_tmp
    upstream_tmp="$(mktemp)"

    require_command curl
    load_pdd_source_config

    local addendum_path="$REPO_ROOT/$PDD_ADDENDUM"
    if [[ ! -f "$addendum_path" ]]; then
        echo -e "${RED}ERROR: PDD addendum not found: $PDD_ADDENDUM${NC}"
        rm -f "$upstream_tmp"
        exit 1
    fi

    if ! curl -fsSL "$(pdd_raw_url)" -o "$upstream_tmp"; then
        echo -e "${RED}ERROR: Failed to fetch canonical PDD SOP from GitHub${NC}"
        echo "  URL: $(pdd_raw_url)"
        rm -f "$upstream_tmp"
        exit 1
    fi

    mkdir -p "$(dirname "$output_path")"
    {
        echo '<!-- GENERATED FILE: DO NOT EDIT -->'
        echo "<!-- Source: $(pdd_blob_url) -->"
        echo '<!-- Regenerate with: ./scripts/sync-embedded-files.sh -->'
        echo
        cat "$upstream_tmp"
        echo
        echo
        cat "$addendum_path"
    } > "$output_path"

    rm -f "$upstream_tmp"
}

sync_mirrored_files() {
    local changed=0

    for mapping in "${MIRRORED_FILES[@]}"; do
        local src="${mapping%%:*}"
        local dest="${mapping##*:}"
        local src_path="$REPO_ROOT/$src"
        local dest_path="$REPO_ROOT/$dest"

        if [[ ! -f "$src_path" ]]; then
            echo -e "${RED}ERROR: Source file not found: $src${NC}"
            exit 1
        fi

        mkdir -p "$(dirname "$dest_path")"

        if [[ ! -f "$dest_path" ]] || ! diff -q "$src_path" "$dest_path" > /dev/null 2>&1; then
            cp "$src_path" "$dest_path"
            echo -e "${GREEN}Synced: $src -> $dest${NC}"
            changed=1
        else
            echo -e "Up to date: $dest"
        fi
    done

    if [[ $changed -eq 1 ]]; then
        echo -e "\n${YELLOW}Mirrored files were synced.${NC}"
    else
        echo -e "\n${GREEN}All mirrored files are up to date.${NC}"
    fi
}

sync_generated_files() {
    local changed=0
    local dest_path="$REPO_ROOT/$PDD_DEST"
    local generated_tmp
    generated_tmp="$(mktemp)"

    generate_pdd_sop "$generated_tmp"

    if [[ ! -f "$dest_path" ]] || ! diff -q "$generated_tmp" "$dest_path" > /dev/null 2>&1; then
        mkdir -p "$(dirname "$dest_path")"
        cp "$generated_tmp" "$dest_path"
        echo -e "${GREEN}Generated: $PDD_DEST${NC}"
        echo "  Source: $(pdd_blob_url)"
        changed=1
    else
        echo -e "Up to date: $PDD_DEST"
    fi

    rm -f "$generated_tmp"

    if [[ $changed -eq 1 ]]; then
        echo -e "${YELLOW}Generated files were refreshed from canonical sources.${NC}"
    fi
}

sync_files() {
    sync_mirrored_files
    echo
    sync_generated_files
    echo
    echo -e "${GREEN}Embedded assets sync complete.${NC}"
}

update_pdd_ref() {
    local branch="${1:-main}"

    load_pdd_source_config
    local previous_ref="$PDD_UPSTREAM_REF"
    local next_ref
    next_ref="$(resolve_latest_pdd_ref "$branch")"

    if [[ "$previous_ref" == "$next_ref" ]]; then
        echo -e "${GREEN}PDD upstream ref already pinned to ${next_ref}${NC}"
        echo "  Source: https://github.com/${PDD_UPSTREAM_REPO}/tree/${branch}"
    else
        write_pdd_source_config "$next_ref"
        echo -e "${GREEN}Updated PDD upstream ref${NC}"
        echo "  Repo: ${PDD_UPSTREAM_REPO}"
        echo "  Branch: ${branch}"
        echo "  Old: ${previous_ref}"
        echo "  New: ${next_ref}"
    fi

    echo
    sync_files
}

check_mirrored_files() {
    local out_of_sync=0

    for mapping in "${MIRRORED_FILES[@]}"; do
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

    return $out_of_sync
}

check_generated_files() {
    local out_of_sync=0
    local dest_path="$REPO_ROOT/$PDD_DEST"
    local generated_tmp
    generated_tmp="$(mktemp)"

    generate_pdd_sop "$generated_tmp"

    if [[ ! -f "$dest_path" ]]; then
        echo -e "${RED}MISSING: $PDD_DEST${NC}"
        echo "  Source: $(pdd_blob_url)"
        out_of_sync=1
    elif ! diff -q "$generated_tmp" "$dest_path" > /dev/null 2>&1; then
        echo -e "${RED}OUT OF SYNC: $PDD_DEST${NC}"
        echo "  Source: $(pdd_blob_url)"
        echo "  Diff:"
        diff "$generated_tmp" "$dest_path" | head -20 || true
        out_of_sync=1
    else
        echo -e "${GREEN}✓${NC} $PDD_DEST"
    fi

    rm -f "$generated_tmp"
    return $out_of_sync
}

check_files() {
    local out_of_sync=0

    echo "Checking embedded assets are in sync..."
    echo

    if ! check_mirrored_files; then
        out_of_sync=1
    fi

    if ! check_generated_files; then
        out_of_sync=1
    fi

    echo

    if [[ $out_of_sync -eq 1 ]]; then
        echo -e "${RED}ERROR: Embedded assets are out of sync!${NC}"
        echo
        echo "Run './scripts/sync-embedded-files.sh' to sync them."
        echo
        echo "This check exists because files referenced via include_str!()"
        echo "must be inside the crate directory to be included when publishing"
        echo "to crates.io. Some files are mirrored from local sources, and the"
        echo "PDD SOP is generated from its canonical GitHub source plus a"
        echo "small Ralph addendum."
        exit 1
    else
        echo -e "${GREEN}All embedded assets are in sync.${NC}"
    fi
}

case "${1:-sync}" in
    check)
        check_files
        ;;
    sync|"")
        sync_files
        ;;
    update-pdd-ref)
        update_pdd_ref "${2:-main}"
        ;;
    *)
        echo "Usage: $0 [sync|check|update-pdd-ref [branch]]"
        echo "  sync            - Sync mirrored files and generated embedded assets (default)"
        echo "  check           - Check if embedded assets are in sync (for CI)"
        echo "  update-pdd-ref  - Pin PDD to latest upstream branch SHA and resync"
        exit 1
        ;;
esac
