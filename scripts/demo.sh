#!/usr/bin/env bash
# The full lifecycle from docs/guide/06, executable: one mission, three
# agents, four tokens, against a throwaway store. Run via `just demo`.
set -uo pipefail

DEMO_DIR="$(mktemp -d)"
export WAGGLE_STORE="$DEMO_DIR/waggle.db"
trap 'rm -rf "$DEMO_DIR"' EXIT

W() { # W <actor> <args...> — run one waggle verb as an actor
  local sharer="$1"; shift
  WAGGLE_SHARER="$sharer" cargo run -q -p waggle-cli -- "$@" || true
}

tok()   { python3 -c 'import json,sys; print(json.load(sys.stdin)["result"]["token"])'; }
field() { python3 -c "import json,sys; print(json.dumps(json.load(sys.stdin)$1, indent=2))"; }

say() { printf '\n\033[1m%s\033[0m\n' "$*"; }

say "ACT 1 · The orchestrator mints the mission"
MISSION=$(W orchestrator mint --target "ws://mission/launch-brief-task.md" | tok)
echo "  handoff line (what each subagent receives, ~30 bytes):"
echo "    resolve $MISSION via waggle for your working context"

say "ACT 2 · The researcher resolves the mission, produces the report, mints a CHILD"
W researcher resolve --token "$MISSION" >/dev/null
REPORT=$(W researcher mint --target "ws://swarm/market-report.md" \
  --sharer researcher --parent "$MISSION" | tok)
echo "  lineage formed AT MINT (--parent):"
echo "    $MISSION  (mission)"
echo "     └── $REPORT  (market report)"

say "ACT 3 · The writer retrieves like a surgeon: map → resolve → slice"
echo "  map (~300 bytes — where am I?):"
W writer map --token "$REPORT" | field '["result"]["here"]'
echo "  resolve (the projection, not the blob):"
W writer resolve --token "$REPORT" | field '["result"]["body"]["inline"]["data"]'
echo "  query with a 256-byte budget — the reply names the bytes you avoided:"
W writer query --token "$REPORT" --max-bytes 256 | field '["result"]["slice"]'

say "ACT 4 · Work is reported; the funnel is the receipt"
W fact-checker record --token "$REPORT" --stage assess >/dev/null
W writer record --token "$REPORT" --stage run >/dev/null
BRIEF=$(W writer mint --target "ws://swarm/launch-brief.md" \
  --sharer writer --parent "$MISSION" | tok)
echo "  report funnel:"
W orchestrator funnel --token "$REPORT" | field '["result"]["stages"]'
echo "  the mission's children (the delegation tree, as data):"
W orchestrator query --token "$MISSION" --path /children | field '["result"]["slice"]'

say "ACT 5 · The correction — stale CAS refused, then supersede lands"
V2=$(W researcher mint --target "ws://swarm/market-report-v2.md" \
  --sharer researcher --parent "$MISSION" | tok)
echo "  a STALE expected-version (99) is refused, and the hint names the fix:"
W researcher mutate --token "$REPORT" --change "supersede=$V2" --expected-version 99 \
  | field '["hint"]'
W researcher mutate --token "$REPORT" --change "supersede=$V2" --expected-version 1 >/dev/null
echo "  the writer's LATE resolve now carries the pointer forward:"
W writer resolve --token "$REPORT" | field '["result"]["disposition"]'

say "ACT 6 · The orchestrator reads the story back"
W orchestrator map --token "$REPORT" | field '["result"]["here"]'
echo
echo "  (brief $BRIEF and correction $V2 live in the same tree under $MISSION)"
echo
echo "The store this ran against lived in $DEMO_DIR — and is gone now."
echo "Every line above was a real envelope from the real binary."
