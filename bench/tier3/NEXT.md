# Tier-3 resume plan (SWE-bench × 9 models)

Resume with: `claude --continue --dangerously-skip-permissions`, then
"continue the Tier-3 build".

## Confirmed working (this session)
- SWE-bench-Lite loads (300 instances) via `datasets`.
- `git clone` works; **Docker grading works on this Mac** — gold patch on
  `psf__requests-863` **resolved in ~59s** (arm64, prebuilt image, emulated).
- `rote` adapters `openai` + `anthropic` built, bound to vault keys
  `OPENAI_API_KEY` / `ANTHROPIC_API_KEY`.
- `bench/tier3/rote_models.py` — uniform client, both families return
  `(text, in_tokens, out_tokens)`. OpenAI usage exact; **Anthropic usage
  approximated** (rote strips it) — noted in results.

## Environment
- venv + swebench: `…/scratchpad/swebench-venv/bin/python` (has `datasets`,
  `swebench`).
- rote workspace for model calls: `~/.rote/rote/workspaces/cf-dns2`.
- rote bin: `/Users/chetanconikee/.local/bin/rote`.
- Models: Anthropic {`claude-opus-4-1-20250805`, `claude-sonnet-4-5-20250929`,
  `claude-haiku-4-5-20251001`}; OpenAI {`gpt-5`, `gpt-5-mini`, `gpt-4.1`,
  `gpt-4.1-mini`, `gpt-4o`, `gpt-4o-mini`}.

## Next steps
1. Build `bench/tier3/agent.py`: clone repo@base_commit; bounded JSON-command
   loop (`ls`/`read`/`search`/`submit`) → unified-diff patch; return patch +
   steps + tokens.
2. **Validation slice**: run `gpt-4o-mini` + `claude-haiku-4-5-20251001` on
   `psf__requests-863`; write predictions; grade each via swebench; record
   resolved + cost + steps. Measure per-run cost & wall-time.
3. If clean → scale to the **full 9-model sweep** over a small instance
   subset (start ~10 light instances: psf/requests, pallets/flask). Add the
   **waggle interrogation arm** vs copy baseline (the actual novelty).
4. Emit `paper/generated/tier3_*.{tex,dat}`; wire into paper §Evaluation.

## Fallback
If Docker grading is flaky at scale, grade by localization (patch touches
gold files/hunks) + `git apply` validity + cost — real cross-model signal,
no Docker. Say so plainly in results.
