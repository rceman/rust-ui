#!/usr/bin/env bash
# token-budget-gate.sh — Stop-hook for the per-file token budget.
#
# Fires every time the agent finishes a turn. Runs
# tools/check-file-tokens.sh; on violations it blocks the stop once with
# the worst offenders + instructions to invoke the token-budget skill.
# `stop_hook_active` prevents infinite blocking: the second Stop event
# (hook already active) is always allowed through, so the agent may
# acknowledge and still end its turn.

stdin_json="$(cat)"

# A stop hook is already active → this is the follow-up stop; let it pass
# so the agent isn't trapped in a block loop.
if printf '%s' "$stdin_json" | grep -q '"stop_hook_active"[^,}]*true'; then
    exit 0
fi

cd "$(dirname "$0")/.." || exit 0

violations="$(tools/check-file-tokens.sh 2>/dev/null)"
status=$?

if [ $status -eq 0 ]; then
    exit 0
fi

top="$(printf '%s\n' "$violations" | grep '^FAIL' | head -10)"
count="$(printf '%s\n' "$violations" | grep -c '^FAIL')"

python3 - "$count" <<EOF
import json, sys
top = """$top"""
reason = (
    f"TOKEN BUDGET: {sys.argv[1]} file(s) exceed the 5000-token limit "
    "(tokens = bytes/4). Invoke the token-budget skill and refactor the "
    "worst offender, or explicitly acknowledge deferral to the user.\n" + top
)
print(json.dumps({"decision": "block", "reason": reason}))
EOF
