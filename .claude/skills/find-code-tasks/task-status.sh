#!/usr/bin/env bash
#
# task-status.sh - Display frontmatter status of all code-task files
#
# Usage:
#   ./tools/task-status.sh              # Show all tasks
#   ./tools/task-status.sh --pending    # Filter by status
#   ./tools/task-status.sh --json       # Output as JSON
#   ./tools/task-status.sh --summary    # Show counts only
#

set -euo pipefail

TASKS_DIR="${TASKS_DIR:-.ralph/tasks}"
FORMAT="table"
FILTER_STATUS=""

# Parse arguments
while [[ $# -gt 0 ]]; do
    case $1 in
        --pending|--in_progress|--completed)
            FILTER_STATUS="${1#--}"
            shift
            ;;
        --json)
            FORMAT="json"
            shift
            ;;
        --summary)
            FORMAT="summary"
            shift
            ;;
        --help|-h)
            echo "Usage: task-status.sh [OPTIONS]"
            echo ""
            echo "Options:"
            echo "  --pending      Show only pending tasks"
            echo "  --in_progress  Show only in-progress tasks"
            echo "  --completed    Show only completed tasks"
            echo "  --json         Output as JSON array"
            echo "  --summary      Show status counts only"
            echo "  --help         Show this help message"
            exit 0
            ;;
        *)
            echo "Unknown option: $1" >&2
            exit 1
            ;;
    esac
done

# Extract frontmatter from a file
# Returns: status|created|started|completed|filename
extract_frontmatter() {
    local file="$1"
    local filename=$(basename "$file" .code-task.md)

    # Check if file starts with ---
    if ! head -1 "$file" | grep -q '^---$'; then
        echo "pending|null|null|null|$filename"
        return
    fi

    # Extract frontmatter block (between first and second ---)
    # Use awk for portability (macOS head doesn't support -n -1)
    local frontmatter=$(awk 'NR==1 && /^---$/ {found=1; next} found && /^---$/ {exit} found {print}' "$file")

    # Parse YAML fields (simple key: value parsing)
    local status=$(echo "$frontmatter" | grep -E '^status:' | sed 's/status:[[:space:]]*//' || echo "pending")
    local created=$(echo "$frontmatter" | grep -E '^created:' | sed 's/created:[[:space:]]*//' || echo "null")
    local started=$(echo "$frontmatter" | grep -E '^started:' | sed 's/started:[[:space:]]*//' || echo "null")
    local completed=$(echo "$frontmatter" | grep -E '^completed:' | sed 's/completed:[[:space:]]*//' || echo "null")

    # Default empty values to null
    [[ -z "$status" ]] && status="pending"
    [[ -z "$created" ]] && created="null"
    [[ -z "$started" ]] && started="null"
    [[ -z "$completed" ]] && completed="null"

    echo "$status|$created|$started|$completed|$filename"
}

# Find command: prefer fd if available (faster), fall back to find
if command -v fd &>/dev/null; then
    find_tasks() {
        fd --type f --extension code-task.md --print0 . "$TASKS_DIR" 2>/dev/null | sort -z
    }
else
    find_tasks() {
        find "$TASKS_DIR" -name "*.code-task.md" -print0 2>/dev/null | sort -z
    }
fi

# Collect all task data
declare -a TASKS=()
while IFS= read -r -d '' file; do
    data=$(extract_frontmatter "$file")

    # Apply status filter if set
    if [[ -n "$FILTER_STATUS" ]]; then
        file_status=$(echo "$data" | cut -d'|' -f1)
        [[ "$file_status" != "$FILTER_STATUS" ]] && continue
    fi

    TASKS+=("$data")
done < <(find_tasks)

# Output based on format
case $FORMAT in
    table)
        # Status symbols
        symbol() {
            case $1 in
                completed)   echo "✓" ;;
                in_progress) echo "●" ;;
                pending)     echo "○" ;;
                blocked)     echo "■" ;;
                *)           echo "?" ;;
            esac
        }

        echo "TASKS STATUS"
        echo "════════════════════════════════════════════════════════════════"
        printf "%-3s %-40s %-12s %-12s\n" "" "TASK" "STATUS" "DATE"
        echo "────────────────────────────────────────────────────────────────"

        for task in "${TASKS[@]}"; do
            IFS='|' read -r status created started completed filename <<< "$task"
            sym=$(symbol "$status")

            # Show most relevant date
            case $status in
                completed)   date="$completed" ;;
                in_progress) date="$started" ;;
                *)           date="$created" ;;
            esac
            [[ "$date" == "null" ]] && date="-"

            printf "%-3s %-40s %-12s %-12s\n" "$sym" "$filename" "$status" "$date"
        done

        echo "────────────────────────────────────────────────────────────────"
        echo "Total: ${#TASKS[@]} tasks"
        ;;

    json)
        echo "["
        first=true
        for task in "${TASKS[@]}"; do
            IFS='|' read -r status created started completed filename <<< "$task"

            [[ "$first" == "true" ]] && first=false || echo ","

            # Convert null strings to JSON null
            [[ "$created" == "null" ]] && created="null" || created="\"$created\""
            [[ "$started" == "null" ]] && started="null" || started="\"$started\""
            [[ "$completed" == "null" ]] && completed="null" || completed="\"$completed\""

            printf '  {"task": "%s", "status": "%s", "created": %s, "started": %s, "completed": %s}' \
                "$filename" "$status" "$created" "$started" "$completed"
        done
        echo ""
        echo "]"
        ;;

    summary)
        pending=0
        in_progress=0
        completed=0
        blocked=0

        for task in "${TASKS[@]}"; do
            status=$(echo "$task" | cut -d'|' -f1)
            case $status in
                pending)     ((pending++)) || true ;;
                in_progress) ((in_progress++)) || true ;;
                completed)   ((completed++)) || true ;;
                blocked)     ((blocked++)) || true ;;
            esac
        done

        echo "Task Summary"
        echo "────────────"
        echo "○ Pending:     $pending"
        echo "● In Progress: $in_progress"
        echo "✓ Completed:   $completed"
        [[ $blocked -gt 0 ]] && echo "■ Blocked:     $blocked"
        echo "────────────"
        echo "  Total:       ${#TASKS[@]}"
        ;;
esac
