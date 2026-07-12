"""Uniform model client over the `rote` CLI (design doc 22 §4, Tier-3).

Both families are reached through `rote` adapters (openai, anthropic) that
read the API keys from the vault — curl is sandbox-blocked, so this is the
supported path. The client normalises the two request/response shapes into a
single `(text, usage)` return so the agent loop treats every model the same.
"""

from __future__ import annotations

import json
import random
import re
import os
import subprocess
import time
from dataclasses import dataclass

# The rote workspace the adapter calls run from (rote needs an active ws).
ROTE_WS = "/Users/chetanconikee/.rote/rote/workspaces/cf-dns2"
ROTE_BIN = "/Users/chetanconikee/.local/bin/rote"

# Which family each model id belongs to.
ANTHROPIC = {
    "claude-opus-4-1-20250805",
    "claude-sonnet-4-5-20250929",
    "claude-haiku-4-5-20251001",
}
OPENAI = {
    "gpt-5",
    "gpt-5-mini",
    "gpt-4.1",
    "gpt-4.1-mini",
    "gpt-4o",
    "gpt-4o-mini",
}


@dataclass
class Reply:
    text: str
    in_tokens: int
    out_tokens: int


def _rote(args: list[str]) -> str:
    """Run a rote command in the workspace and return stdout."""
    out = subprocess.run(
        [ROTE_BIN, *args],
        cwd=ROTE_WS,
        capture_output=True,
        text=True,
        timeout=int(os.environ.get("ROTE_TIMEOUT", "180")),
    )
    return out.stdout + out.stderr


def _resp_id(raw: str) -> str | None:
    m = re.search(r"@(\d+)", raw)
    return f"@{m.group(1)}" if m else None


def _query_text(rid: str) -> str:
    """The `.content[0].text` value, stripped of rote's trailing chatter."""
    raw = _rote(["query", rid, ".content[0].text"])
    for marker in ("[HINT]", "Shell cwd"):
        i = raw.find(marker)
        if i != -1:
            raw = raw[:i]
    return raw.strip()


def _first_json(s: str):
    """Decode the first JSON value in `s` (ignoring trailing text)."""
    s = s.strip()
    obj, _ = json.JSONDecoder().raw_decode(s)
    return json.loads(obj) if isinstance(obj, str) else obj


def _approx_tokens(*parts: str) -> int:
    return sum(len(p) for p in parts) // 4


# rote surfaces transport/API failures as the response *text*; treat these as
# errors to retry, never as the model's reply.
_FAIL = (
    "HTTP execution failed",
    "error sending request",
    "Tool '",
    "not found in tools.json",
    "rate_limit",
    "overloaded",
)


def _is_failure(text: str) -> bool:
    return any(f in text for f in _FAIL)


def call(model: str, system: str, user: str, max_tokens: int = 2048, retries: int = 4) -> Reply:
    """One completion, with retry on transient transport/API failures."""
    last = ""
    for attempt in range(retries):
        try:
            r = _call_once(model, system, user, max_tokens)
            if not _is_failure(r.text):
                return r
            last = r.text
        except Exception as e:  # transport blew up entirely
            last = f"{type(e).__name__}: {e}"
        time.sleep(2 * (attempt + 1) + random.random())
    raise RuntimeError(f"{model}: {last[:180]}")


def _call_once(model: str, system: str, user: str, max_tokens: int) -> Reply:
    if model in ANTHROPIC:
        body = {
            "anthropic-version": "2023-06-01",
            "model": model,
            "max_tokens": max_tokens,
            "system": system,
            "messages": [{"role": "user", "content": user}],
        }
        rid = _resp_id(_rote(["anthropic_call", "messages", json.dumps(body), "-s"]))
        if not rid:
            raise RuntimeError(f"no response id for {model}")
        # The anthropic adapter surfaces only the assistant text, and rote
        # prints it JSON-quoted — decode it back to the raw string (otherwise
        # every escape and newline is mangled). Usage is stripped by the
        # adapter, so token counts are approximated (noted in the results).
        raw = _query_text(rid)
        try:
            val = json.loads(raw)
            text = val if isinstance(val, str) else raw
        except Exception:
            text = raw
        return Reply(text, _approx_tokens(system, user), _approx_tokens(text))

    if model in OPENAI:
        body = {
            "model": model,
            "messages": [
                {"role": "system", "content": system},
                {"role": "user", "content": user},
            ],
            "max_completion_tokens": max_tokens,
        }
        rid = _resp_id(_rote(["openai_call", "chat", json.dumps(body), "-s"]))
        if not rid:
            raise RuntimeError(f"no response id for {model}")
        obj = _first_json(_query_text(rid))
        choice = (obj.get("choices") or [{}])[0]
        text = (choice.get("message") or {}).get("content") or ""
        u = obj.get("usage", {})
        return Reply(text, u.get("prompt_tokens", 0), u.get("completion_tokens", 0))

    raise ValueError(f"unknown model: {model}")


if __name__ == "__main__":
    import sys

    m = sys.argv[1] if len(sys.argv) > 1 else "gpt-4o-mini"
    r = call(m, "You are terse.", "Reply with exactly one word: ok", max_tokens=20)
    print(f"{m}: text={r.text!r} in={r.in_tokens} out={r.out_tokens}")
