# Coach engine

The desktop **Coach** page turns the archive's per-call rows into practice scores, anti-pattern findings, flow/pace analysis, a per-day session timeline, and AI code-output stats. Everything is computed locally and deterministically in `src/coach/` — no network calls, no LLM calls.

## Attribution

The scoring model, rule catalog, and flow/pace/timeline/output algorithms are ported from Microsoft's [AI-Engineering-Coach](https://github.com/microsoft/AI-Engineering-Coach) VS Code extension (MIT, Copyright (c) Microsoft Corporation) — see `NOTICE` at the repository root. The port is Rust-native: rules are code, not the reference's user-editable markdown DSL. Thresholds keep the reference defaults unless a deviation is listed below.

## Data model: calls → turns → sessions

tokens' parsers emit one `ParsedCall` per assistant API round; the reference evaluates rules over one row per *user turn*. `src/coach/sessions.rs` therefore collapses consecutive calls of a session that share the same prompt state (`user_message`, `prompt_chars`) into a `Turn` before rules run:

- token fields, tool calls, code blocks, and file lists are summed/concatenated across the turn's rounds;
- `elapsed_ms` is the max round latency (the last round's user→assistant gap ≈ full turn latency);
- two identical prompts sent back-to-back merge into one turn — an accepted simplification.

User think-time between turns is `think_gap_ms` (next turn start − next turn latency − current turn end), used by flow scoring and speed-accept.

`src/coach/signals.rs` gates rule denominators per tool so "no signal" never reads as "clean" (or as a finding): cancellation, file references, cache reads, and conversational-session shape are currently claude-code + codex only. The per-tool signal matrices live in `docs/development/tools/<name>.md`.

## Practice scores (`src/coach/score.rs`)

- Severity penalties: high = 12, medium = 7, low = 3, applied once per **triggered** rule — occurrence counts do not move the score.
- `score = max(0, round(100 · (1 − penalty / (rules_in_group × 12))))` per group.
- Weekly series: calls bucketed by local ISO week, the full rule set re-run per bucket. WoW compares the last two weekly scores; MoM compares the mean of the last 4 weeks against weeks 5–8 back. Fewer buckets → "–". The trailing 12 weekly scores per group ship in the payload as `trend` for the report-card sparklines.
- Composite grade: `composite_score` is the rules-weighted mean of the group scores — `round(Σ(score·rules_in_group) / 27)` — so a group carries the share of the overall grade its rules hold in the catalog. `grade_id` maps any 0–100 score to a letter id: A+ ≥ 97, A ≥ 90, B+ ≥ 85, B ≥ 80, C ≥ 70, D ≥ 60, else F (copy maps ids to letters via `coach.report.grade_labels`). The composite plus its grade ship as `CoachData.overall`; each group also carries its own `grade_id`.

## Rule catalog (27 rules, `src/coach/rules/`)

Groups: prompt-quality 8, session-hygiene 9, code-review 5, tool-mastery 5. User-facing wording lives in `src/copy/copy.json` under `coach.rules.<id>` (a test asserts the ids match). Signals: **P** prompt text/length, **T** timestamps, **K** tokens, **M** model, **L** tool calls, **C** code blocks, **F** file lists, **X** cancellation.

| id | group | severity | trigger (defaults) | signals |
| --- | --- | --- | --- | --- |
| lazy-prompting | prompt-quality | medium | >30% of prompts <30 chars, count >10 | P |
| caps-lock | prompt-quality | medium | ≥1 message ≥90% caps | P |
| frustration-signals | prompt-quality | medium | ≥2 messages with !!!/???/hostile phrases or ≥40% caps words | P |
| repeated-prompts | prompt-quality | medium | ≥3 near-duplicates (first-100-chars key); high >20 | P |
| low-constraint-usage | prompt-quality | medium | <8% of ≥40-char prompts contain constraints, ≥30 samples | P |
| verbose-output | prompt-quality | medium | >10% of turns: >5K output tokens from <200-char prompts, count >10 | P K |
| verbose-prompt-no-compression | prompt-quality | low | >20% of prompts ≥800 chars with 2+ filler words, count >15 | P |
| excessive-file-context | prompt-quality | medium | ≥10 turns referencing ≥30 files | F |
| session-drift | session-hygiene | medium | >3 sessions covering ≥4 work types in ≥5 turns | P |
| late-night-coding | session-hygiene | low | >10 turns between 00:00–05:00 local | T |
| weekend-overwork | session-hygiene | low | >25% weekend turns, >20 weekend turns | T |
| mega-sessions | session-hygiene | high | any session ≥50 turns | — |
| abandon-sessions | session-hygiene | low | >40% single-turn sessions, >10 abandoned | — |
| high-cancellation | session-hygiene | medium | >15% interrupted turns; high >30% | X |
| broken-flow-state | session-hygiene | medium | >60% of ≥5 scored days fragmented; high >80% | T |
| slow-responses | session-hygiene | low | >5 turns over 5 min (elapsed) | T |
| runaway-agent-loops | session-hygiene | high | ≥3 turns with 40+ tool calls | L |
| copy-paste-blindness | code-review | high | ≥3 sessions: ≥50 AI LoC, no refinement afterwards | C P F |
| speed-accept | code-review | high | ≥5 times: next message <15s after ≥20 AI LoC | C T |
| vibe-coding | code-review | high | ≥3 sessions: ≥100 AI LoC from ≤5 unstructured prompts | C P |
| tunnel-vision | code-review | low | >95% of turns in one project (≥3 projects, ≥50 turns) | — |
| no-language-exploration | code-review | low | no new language in 8+ ISO weeks (≥4 active weeks); high >12 | C T |
| mcp-tool-bloat | tool-mastery | medium | ≥3 sessions with >40 distinct tools | L |
| model-overreliance | tool-mastery | medium | >80% of turns on one model, <3 models, >10 turns | M |
| reasoning-effort-overuse | tool-mastery | medium | >50% of effort-tagged turns at high/max, >20 tagged | M |
| premium-waste | tool-mastery | medium | >10 trivial/lookup turns on premium models | P M C L |
| cache-hit-starvation | tool-mastery | medium | >20 turns >5K input tokens with <10% cache-read share | K |

### Deviations from the reference

- **runaway-agent-loops** raises the per-turn tool threshold from 15 to 40: tokens' turns aggregate full agentic CLI loops where 15+ tool calls is routine.
- **slow-responses** raises the latency threshold from 30s to 5 minutes for the same reason — a turn spans the whole agentic run, and 300s matches the flow score's slowest latency band.
- **mcp-tool-bloat** implements the documented semantics (distinct tools per session); the reference's DSL referenced a nonexistent field and could never fire.
- **premium-waste** merges the reference's `premium-waste` and `premium-for-lookup-questions`, and derives "premium" from tokens' own pricing book (output rate ≥ $10/M tokens) instead of a hardcoded model-tier table.
- **verbose-prompt-no-compression** drops the "compression skill installed" exemption (no installed-skills signal) and matches filler words on the stored 500-char prompt prefix.
- Regex patterns are reimplemented as word-boundary phrase matching in `src/coach/text.rs` (no regex dependency); semantics match, including quirks such as `\btest\b` not matching "tests".
- Content rules run on the 500-char `user_message` prefix; length rules use the full `prompt_chars`.

### Skipped rules

Reference rules whose inputs tokens does not ingest (most flagged `requiresIdeContext` upstream): yolo-mode, no-plan-mode, no-skills, no-slash-commands, no-custom-instructions, auto-approve-terminal, agent-mode-for-asks, no-devcontainer, auto-avoidance, no-file-context, instruction-bloat, context-engineering-gaps, no-spec-driven-development, no-spec-structure, low-markdown-ratio, agentic-no-tools — those signals only exist in VS Code chat logs (tool confirmations, agent modes, instruction files) or presume IDE-style sessions. **profanity** is skipped as a judgment call: scanning prompts for swearing reads as surveillance in a usage dashboard. All LLM-dependent reference features (Skill Finder, Learning Center, Context Health AI review) are permanently out of scope under the no-network rule.

## Flow, pace, timeline, output

- **Flow** (`flow.rs`): per session (≥3 timestamped turns) `score = 0.40·rapid-follow-up rate (≤30s) + 0.30·median-gap band + 0.15·duration band + 0.15·density band`; labels deep ≥70 / moderate ≥45 / shallow ≥25 / else fragmented. Days merge session spans into work blocks split at >15 min gaps. The summary counts deep (≥70) and fragmented (<25) days; both counts and the daily scores ship in the payload for the KPI panel and recent-flow sparkline.
- **Pace** (`pace.rs`): late-night = local hour ≥22 or <5; weekend = Sat/Sun; streak = consecutive active days; alerts on streak ≥14, rising 3-week trends (+20% band), late-night rate >0.15, weekend rate >0.25; risk high at 3+ alerts or (streak ≥14 ∧ late-night rising).
- **Timeline** (`timeline.rs`): per local day, one row per session, blocks split at >15 min gaps, max concurrency via a start/end event sweep.
- **Output** (`output.rs`): folds per-call `code_blocks` (fences + Write/Edit payloads, merged by language) into LoC by language/day/project/model, and names tools contributing no code-output signal.
- **Projects** (`projects.rs`): per-project detail for the Activity → Projects cards. Estimated active time reuses the timeline block model (call timestamps split at >15 min gaps, each block spanning at least one minute, summed per session). Languages fold turn `code_blocks` by LoC; hot files count per-call `edited_files` (paths shown project-relative, else the trailing two segments) and ship the top 3. The work-pattern chip is a display heuristic of this port, not an upstream rule: weekend share of timestamped turns ≥60% reads "mostly weekends", ≤20% "mostly weekdays", else "weekends + weekdays"; a daypart (mornings 05–11, afternoons 12–16, evenings 17–21, late nights 22–04, matching the pace module's late-night window) is appended when it carries ≥40% of timestamped turns. All labels resolve through `coach.timeline.pattern_*` copy ids.

## Wiring

`timeline::daily_turn_counts` buckets turns by local day (a turn lands on its first timestamped call's day), windowed by `timeline::grid_window_days` anchored at today — a trailing 9 weeks for scoped periods, 53 weeks for All Time; the counts ship as `timeline_grid` with a per-day `in_period` flag and drive the daily-bars day picker above the session table. The day picker is built from tool/project-filtered calls that ignore the period filter — same scope as `coach_timeline` — because context days outside the period stay visible (dimmed bars) on the strip. `coach_timeline` and the AI-Output project ranking resolve project names through the dashboard's `project_label` short labels, so the same project reads identically across pages.

`Ingested::coach` / `Ingested::coach_timeline` (`src/ingest/pipeline.rs`) filter by period/tool/project and delegate to `coach::coach_data` / `coach::coach_timeline`, which build the `CoachData` / `CoachTimelineDay` payloads in `src/data/mod.rs`. `App::coach_for` / `App::coach_timeline_for` (`src/app.rs`) memoize per filter key in the generation-keyed `QueryCache` (bounding the `data::leak` growth exactly like Analytics). The desktop exposes `get_coach(period)` and `get_coach_timeline(day)` Tauri commands; after a timeline row is selected, the existing `get_session_detail(key)` command supplies its call-level inspector. The Work Hours view deliberately reuses the memoized `get_analytics(period)` hour×weekday matrix and the dashboard activity timeline rather than introducing a second aggregation with different semantics. The Coach page fetches on the `period|tool|project|data_generation|currency` key. Sample mode (Shift+D) serves `data::coach_sample()`.

The archive v4 enrichment columns that feed all of this are documented in `docs/development/architecture.md` and per-parser in `docs/development/tools/<name>.md`.
