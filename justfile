# Automated GitHub issue resolution with Claude Code
#
# Exit codes for have-claude-tackle-next-issue-with-labels:
#   0 = successfully launched Claude to tackle an issue
#   2 = no matching issues found ("nothing to do")
#   1 = error

set fallback := false

# Tackle the highest-priority open issue matching the given labels
have-claude-tackle-next-issue-with-labels *labels:
    #!/usr/bin/env bash
    set -euo pipefail

    labels="{{labels}}"
    if [[ -z "$labels" ]]; then
        echo "Error: at least one label is required"
        exit 1
    fi

    # Build --label flags for gh CLI
    label_args=()
    for label in $labels; do
        label_args+=(--label "$label")
    done

    # Fetch open issues with all specified labels
    issues=$(gh issue list "${label_args[@]}" --state open \
        --json number,title,labels,body,url --limit 100)

    # Sort by priority (P1 < P2 < P3 < no priority), tiebreak by issue number
    next=$(echo "$issues" | jq '
        if length == 0 then null
        else
            sort_by([
                (.labels
                    | map(select(.name | test("^P[0-9]+$")))
                    | if length > 0 then (.[0].name[1:] | tonumber) else 999 end),
                .number
            ]) | .[0]
        end
    ')

    if [[ "$next" == "null" || -z "$next" ]]; then
        echo "Nothing to do; no remaining issues with labels: $labels"
        exit 2
    fi

    # Extract issue details
    number=$(echo "$next" | jq -r '.number')
    title=$(echo "$next" | jq -r '.title')
    url=$(echo "$next" | jq -r '.url')
    body=$(echo "$next" | jq -r '.body')

    echo "=== Tackling issue #${number}: ${title} ==="
    echo "    ${url}"
    echo ""

    # Build prompt for Claude
    # NOTE: cannot use $(cat <<'DELIM' ... DELIM) because bash's parser matches
    # parentheses inside the heredoc against the $(...) before the heredoc starts.
    read -r -d '' prompt <<'PROMPT_TEMPLATE' || true
    You are being asked to resolve GitHub issue #__NUMBER__ at __URL__.

    The full issue body is appended below. Read the issue, review the codebase,
    perform any necessary experiments, and then proceed to address it.

    Work on the current branch -- do NOT create a new branch or switch branches.

    When finished, use the `/codex:rescue` skill to ask Codex to review your work
    for completeness and correctness. If it reports any issues, fix them and ask for
    a re-review, continuing until the review is clean.

    Once the issue is fixed and all review feedback addressed, you MUST complete
    all of the following steps before exiting:

    1. Stage and commit all changes with a brief message explaining the work and
       mentioning the issue, e.g. "Fix foo bar (gh issue: #__NUMBER__)". Add a
       fuller explanation in the commit body when the one-line summary is not
       sufficiently detailed.
    2. Push the current branch to the remote.
    3. If there is no open pull request for this branch, create one.
    4. Post a comment on issue #__NUMBER__ explaining the work done, including any
       modifications made in response to review feedback. The comment should start
       with "Addressed via [`<short-hash>`](<commit-url>) in [PR #<n>](<pr-url>)"
       where the commit hash links to the commit on GitHub and the PR reference
       links to the pull request.
    5. Close issue #__NUMBER__ as completed.

    Do NOT skip the commit, push, or issue-comment steps -- they are required.

    Alternatively, if after investigation you discover the issue cannot be fixed
    at all -- e.g. fundamental limitation, or would require refactoring well beyond
    the intended scope -- you may instead:

    1. Use the `/codex:rescue` skill to get a second opinion confirming infeasibility.
    2. Post a detailed comment on the issue explaining why it cannot be addressed.
    3. Close the issue as "not planned".

    This escape hatch should rarely be needed -- these issues are expected to be
    tractable.

    --- ISSUE #__NUMBER__: __TITLE__ ---

    __BODY__
    PROMPT_TEMPLATE

    # Substitute placeholders with actual issue data
    prompt="${prompt//__NUMBER__/$number}"
    prompt="${prompt//__URL__/$url}"
    prompt="${prompt//__TITLE__/$title}"
    prompt="${prompt//__BODY__/$body}"

    # Launch Claude in autonomous mode
    claude -p "$prompt" --dangerously-skip-permissions

# Convenience: tackle next plugin issue (always includes claude-code-plugin label)
tackle-next-plugin-issue *labels:
    just have-claude-tackle-next-issue-with-labels claude-code-plugin {{labels}}

# Tackle all open issues matching the given labels, one at a time
have-claude-tackle-all-issues-with-labels *labels:
    #!/usr/bin/env bash
    set -euo pipefail

    iteration=0
    while true; do
        iteration=$((iteration + 1))

        if [[ $iteration -gt 1 ]]; then
            echo ""
        fi

        set +e
        just have-claude-tackle-next-issue-with-labels {{labels}}
        code=$?
        set -e

        case $code in
            0)
                echo ""
                echo "=== Issue resolved (iteration #${iteration}). Checking for more... ==="
                ;;
            2)
                echo ""
                if [[ $iteration -eq 1 ]]; then
                    echo "=== Nothing to do — no matching issues found. ==="
                else
                    echo "=== All done! Resolved $((iteration - 1)) issue(s). ==="
                fi
                exit 0
                ;;
            *)
                echo ""
                echo "=== Error (exit code $code) on iteration #${iteration}. Stopping. ==="
                exit $code
                ;;
        esac
    done

# Convenience: tackle all plugin issues (always includes claude-code-plugin label)
tackle-all-plugin-issues *labels:
    just have-claude-tackle-all-issues-with-labels claude-code-plugin {{labels}}
