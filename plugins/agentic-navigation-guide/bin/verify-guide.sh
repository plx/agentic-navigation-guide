#!/usr/bin/env bash
#
# verify-guide.sh — Hook script for validating navigation guides
#
# Called by Claude Code hooks after file operations (PostToolUse) and
# at the end of a turn (Stop). Reads hook JSON from stdin, finds the
# nearest navigation guide, and verifies it.
#
# Exit codes:
#   0 — guide is valid (or no guide exists) — completely silent
#   2 — guide is invalid — stderr contains issues + repair suggestion
#
# Usage:
#   <hook-json> | verify-guide.sh          # narrow check (PostToolUse)
#   <hook-json> | verify-guide.sh --broad  # broad check (Stop)

set -euo pipefail

BROAD=false
if [[ "${1:-}" == "--broad" ]]; then
  BROAD=true
fi

# ---------------------------------------------------------------------------
# Parse hook input from stdin
# ---------------------------------------------------------------------------

INPUT="$(cat)"

# Extract cwd from hook JSON; fall back to PWD if jq unavailable or field missing
if command -v jq &>/dev/null; then
  CWD="$(echo "$INPUT" | jq -r '.cwd // empty' 2>/dev/null)" || true
fi
CWD="${CWD:-$PWD}"

# ---------------------------------------------------------------------------
# Locate the nearest navigation guide by walking up from CWD
# ---------------------------------------------------------------------------

GUIDE_NAME="AGENTIC_NAVIGATION_GUIDE.md"
GUIDE_PATH=""
GUIDE_ROOT=""

search_dir="$CWD"
while true; do
  if [[ -f "$search_dir/$GUIDE_NAME" ]]; then
    GUIDE_PATH="$search_dir/$GUIDE_NAME"
    GUIDE_ROOT="$search_dir"
    break
  fi
  parent="$(dirname "$search_dir")"
  if [[ "$parent" == "$search_dir" ]]; then
    break  # reached filesystem root
  fi
  search_dir="$parent"
done

# No guide found — nothing to verify, exit silently
if [[ -z "$GUIDE_PATH" ]]; then
  exit 0
fi

# ---------------------------------------------------------------------------
# Locate the agentic-navigation-guide binary
# ---------------------------------------------------------------------------

ANG_BIN=""
if command -v agentic-navigation-guide &>/dev/null; then
  ANG_BIN="agentic-navigation-guide"
elif [[ -f "$GUIDE_ROOT/Cargo.toml" ]] && command -v cargo &>/dev/null; then
  # Development fallback: build and run from source
  ANG_BIN="cargo run --quiet --release --manifest-path $GUIDE_ROOT/Cargo.toml --"
fi

if [[ -z "$ANG_BIN" ]]; then
  # Can't find the tool — exit silently rather than block the user
  exit 0
fi

# ---------------------------------------------------------------------------
# Narrow check: verify the guide against the filesystem
# ---------------------------------------------------------------------------

VERIFY_OUTPUT=""
VERIFY_EXIT=0
VERIFY_OUTPUT="$($ANG_BIN verify --guide "$GUIDE_PATH" --root "$GUIDE_ROOT" 2>&1)" || VERIFY_EXIT=$?

if [[ $VERIFY_EXIT -eq 0 ]]; then
  # Guide is valid
  if [[ "$BROAD" == "false" ]]; then
    exit 0
  fi
  # For broad check, continue to drift detection below
else
  # Guide has issues — report them
  # Build a concise problem description for the repair skill
  PROBLEMS=""
  while IFS= read -r line; do
    # Extract meaningful error lines (skip blank lines and headers)
    if [[ -n "$line" && "$line" != *"error"*":" && "$line" != *"warning"*":" ]]; then
      if [[ -n "$PROBLEMS" ]]; then
        PROBLEMS="$PROBLEMS; $line"
      else
        PROBLEMS="$line"
      fi
    fi
  done <<< "$VERIFY_OUTPUT"

  # Truncate if too long
  if [[ ${#PROBLEMS} -gt 200 ]]; then
    PROBLEMS="${PROBLEMS:0:197}..."
  fi

  RELATIVE_GUIDE="${GUIDE_PATH#$CWD/}"
  if [[ "$RELATIVE_GUIDE" == "$GUIDE_PATH" ]]; then
    RELATIVE_GUIDE="${GUIDE_PATH#$GUIDE_ROOT/}"
  fi

  cat >&2 <<EOF
Navigation guide is out of sync.

Issues:
$VERIFY_OUTPUT

Invoke the navigation-guide-repair skill to fix this:
  Use the Skill tool with skill: "agentic-navigation-guide:navigation-guide-repair", args: "$RELATIVE_GUIDE '$PROBLEMS'"
EOF
  exit 2
fi

# ---------------------------------------------------------------------------
# Broad check: detect drift (new files that should be in the guide)
# ---------------------------------------------------------------------------

if [[ "$BROAD" == "true" ]]; then
  # Get current filesystem listing (depth 2)
  DUMP_OUTPUT=""
  DUMP_OUTPUT="$($ANG_BIN dump --depth 2 --root "$GUIDE_ROOT" \
    --exclude .git --exclude node_modules --exclude target \
    --exclude build --exclude dist --exclude __pycache__ \
    --exclude .venv 2>/dev/null)" || true

  if [[ -z "$DUMP_OUTPUT" ]]; then
    exit 0
  fi

  # Extract paths from both guide and dump, compare for new items
  # Guide paths: lines matching "- <path>" pattern (POSIX-portable, no grep -P)
  GUIDE_PATHS="$(sed -n 's/^[[:space:]]*- \([^[:space:]#][^[:space:]#]*\).*/\1/p' "$GUIDE_PATH" | sort)"
  DUMP_PATHS="$(echo "$DUMP_OUTPUT" | sed -n 's/^[[:space:]]*- \([^[:space:]#][^[:space:]#]*\).*/\1/p' | sort)"

  # Find paths in dump but not in guide (potential additions)
  NEW_PATHS="$(comm -23 <(echo "$DUMP_PATHS") <(echo "$GUIDE_PATHS"))" || true

  # Filter out placeholder markers and common noise
  NEW_PATHS="$(echo "$NEW_PATHS" | grep -v '^\.\.\.$' | grep -v '^[[:space:]]*$')" || true

  if [[ -n "$NEW_PATHS" ]]; then
    COUNT="$(echo "$NEW_PATHS" | wc -l | tr -d ' ')"

    RELATIVE_GUIDE="${GUIDE_PATH#$CWD/}"
    if [[ "$RELATIVE_GUIDE" == "$GUIDE_PATH" ]]; then
      RELATIVE_GUIDE="${GUIDE_PATH#$GUIDE_ROOT/}"
    fi

    cat >&2 <<EOF
Navigation guide may be out of date — $COUNT new item(s) detected on disk but not in the guide.

New items:
$(echo "$NEW_PATHS" | head -20 | sed 's/^/  - /')

Consider running the audit-guide skill for a thorough review:
  Use the Skill tool with skill: "agentic-navigation-guide:audit-guide", args: "$RELATIVE_GUIDE"
EOF
    exit 2
  fi
fi

exit 0
