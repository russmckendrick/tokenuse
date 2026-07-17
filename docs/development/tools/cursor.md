# Cursor

Cursor is ingested as one reconstructed conversation stream, not as independent database, AgentKv, transcript, and tracking imports. The adapter emits one canonical `ParsedCall` per user request so Agent activity reaches the archive, session views, pricing, and Coach without double counting.

> Status: implemented in `src/tools/cursor/`. The implementation is based on Cursor's [Agent overview](https://cursor.com/docs/agent/overview), [Models & Pricing](https://cursor.com/docs/models-and-pricing), and the reverse-engineered [`store.db` format](https://github.com/cipher982/longhouse/blob/main/docs/specs/cursor-transcript-format.md).

## Discovery

The adapter discovers one aggregate `SessionSource`. Its fingerprint covers every relevant file plus live SQLite WAL/SHM metadata, so a change anywhere reparses the joined corpus once.

| Source | Default location | Purpose |
| --- | --- | --- |
| Cursor state | macOS `~/Library/Application Support/Cursor/User/globalStorage/state.vscdb`; Linux `~/.config/Cursor/User/globalStorage/state.vscdb`; Windows `%APPDATA%/Cursor/User/globalStorage/state.vscdb` | Composer metadata, ordered bubbles, request context, tokens, timestamps, AgentKv |
| Agent transcripts | `~/.cursor/projects/<workspace>/agent-transcripts/**/*.{jsonl,txt}` | Lossy prompt/assistant/tool fallback and legacy sessions |
| AI tracking | `~/.cursor/ai-tracking/ai-code-tracking.db` | Conversation model/mode/time plus project and edited-file fallback |
| Agent chat store | `~/.cursor/chats/<workspace>/<conversation>/store.db` | Current complete ordered user, assistant, reasoning, tool-call, and tool-result stream |

`CURSOR_AGENT_HOME` replaces `~/.cursor`. A copied corpus may place `state.vscdb` directly at `<root>/state.vscdb`; projects, tracking, and chats are then resolved below the same root. Supplied/private corpora are diagnostic input only and must never become fixtures.

SQLite is opened read-only with `mode=ro`, which sees a live WAL without checkpointing or blocking Cursor. A cold/locked database falls back to `immutable=1`.

## Reconstruction

The composer/conversation UUID is the canonical session id. Modern bubble JSON frequently has no `conversationId`; the adapter instead parses `bubbleId:<composer-id>:<bubble-id>`.

`composerData:<composer-id>.fullConversationHeadersOnly` supplies bubble order. Bubbles absent from that list are ordered by their real timestamp and then key. Bubble type `1` begins a user turn; types `0` and `2` are accepted assistant/tool variants. Each user bubble owns the following assistant/tool bubbles up to the next user bubble.

Data is merged in this precedence:

1. Bubbles: explicit token totals, exact message timestamps, `turnDurationMs`, model, mode, and tool-former records.
2. `store.db`: complete ordered prompt/assistant/reasoning text and structured tool calls/results.
3. AgentKv: provider model, tool names/arguments, commands, edit/read paths, generated code, and tool execution duration.
4. `messageRequestContext:<composer>:<bubble>`: explicitly attached/referenced files, diffs, current-file context, and deleted files.
5. JSONL/TXT: prompt, assistant text, and tools only when a richer source lacks them.
6. `ai-code-tracking.db`: model, mode, project, edited-file, and session-time fallback.

Project attribution additionally consults the per-workspace storage tree (`<User>/workspaceStorage/<hash>/`): each hash pairs a `workspace.json` folder URI with a per-workspace `state.vscdb` whose ItemTable (`composer.composerData`, renamed `composer.composerHeaders` in newer builds) lists the composers opened in that workspace. `file://` folders decode to plain absolute paths so the shared git-root project folding applies; multi-root and folderless windows have no `folder` key and keep the existing fallback chain. The mapping sits between the store-session project and the discovery fallback.

Model precedence is narrower and intentional:

1. AgentKv assistant block `providerOptions.cursor.modelName`
2. Bubble `modelInfo.modelName`
3. Composer `modelConfig.modelName`
4. Tracking `conversation_summaries.model` / `ai_code_hashes.model`
5. Store root `lastUsedModel`
6. `cursor-auto`

The final assistant timestamp is the call timestamp. If no assistant timestamp exists, the user timestamp is retained. `turnDurationMs` wins for elapsed time; otherwise the adapter uses the non-negative user-to-final-assistant difference. It never invents spacing between turns.

## Current `store.db`

Each conversation store contains:

```sql
CREATE TABLE meta (key TEXT PRIMARY KEY, value TEXT);
CREATE TABLE blobs (id TEXT PRIMARY KEY, data BLOB);
```

The decoder:

1. hex-decodes `meta['0']` as JSON;
2. reads `latestRootBlobId`;
3. parses protobuf field `1` as the ordered repeated list of 32-byte blob ids;
4. resolves each blob and decodes complete JSON `user`, `assistant`, and `tool` messages;
5. preserves text, reasoning/redacted reasoning, tool calls, tool results, model/request ids, and available execution duration.

Unknown protobuf fields and JSON block types are skipped without aborting other messages or sessions. Malformed stores and the older chunked Composer 1 layout are not partially guessed: the store is skipped and the JSONL/TXT fallback remains eligible. Store-only turns use the root/meta update time for period attribution with `timestamp_quality = session`; no per-turn spacing is fabricated.

## Enrichment and files

Cursor calls populate prompt/response length, tools, Bash commands, fenced/generated code LoC, edited files, referenced files, elapsed time, interaction mode, and cancellation state where available. Shared JSONL helpers normalize languages, merge code blocks, deduplicate paths, and enforce collection caps.

Only explicit context counts as a referenced file:

- attachments and current-file context;
- Read/Grep/Glob/search tool arguments;
- diffs, edits, and deleted files.

Project-layout inventories, terminal inventories, arbitrary tool-result/output strings, and incidental path-looking text are excluded. Write/Edit/Delete-style tool arguments become `edited_files`; generated content also contributes a language/LoC code block.

Composer modes map to `agent`, `chat`, `plan`, or `unknown` using `forceMode`, `unifiedMode`, `isAgentic`, and plan-execution flags. `composerData.status == aborted` marks only the final reconstructed turn canceled. A canceled individual tool-former record is ignored and does not cancel the turn.

## Tokens, time, and quality

Explicit bubble totals are preferred. A missing input or output side alone is estimated as `ceil(chars / 4)`; cache buckets stay zero because Cursor does not expose reliable local cache counts.

### Context-meter input credits

Current Cursor builds write `{0,0}` per-bubble token counts, so chars/4 was the only input signal. The composer's own context meter — `composerData.promptTokenBreakdown.totalUsedTokens`, falling back to `contextTokensUsed` — is Cursor's real input figure for the conversation. When the meter is present and **no** bubble in the conversation carries explicit tokens:

- per-turn chars/4 **input** estimates are dropped (turns keep their output side), and
- one **input credit** call is emitted per conversation: `dedup_key = cursor:composer-input:<composer-id>`, anchored at `composerData.createdAt` (`timestamp_quality = session`) so the credited day stays stable across re-parses, `token_quality = estimated`.

The meter is the *latest* context-window snapshot, not a per-turn sum: growth after the anchor stays uncounted. This undercounts versus the Cursor admin console but never double counts. Any explicit bubble tokens (older builds) win outright and disable the credit.

On duplicate-key archive inserts, Cursor rows refresh their token columns (and the cost computed from them) when the counts changed — reconstruction is authoritative, so a reparse that moves input onto the meter credit, or picks up token counts Cursor filled in after the fact, updates the archived row in place.

Archive v5 records provenance:

| Field | Cursor values |
| --- | --- |
| `interaction_mode` | `agent`, `chat`, `plan`, `unknown` |
| `token_quality` | `exact` when all populated sides are explicit; `estimated` when chars/4 supplies them; `mixed` when only one side is estimated; otherwise `unknown` |
| `timestamp_quality` | `exact` for state message times; `session` for store/tracking session times; `file` for transcript mtime; otherwise `unknown` |

Pricing is always tool-aware. Cursor Auto and official Cursor first-party model rows are looked up in the Cursor scope; unknown/observed-only models use the documented global fallback. See [Model normalisation](../models.md).

## Canonical dedup and archive backfill

Canonical keys are stable and path-independent:

- state-backed: `cursor:composer:<composer-id>:<request-id-or-user-bubble-id>`;
- store-backed: `cursor:chat:<conversation-id>:<request-id-or-turn-index>`;
- transcript-only: `cursor:transcript:<conversation-id>:<turn-index>`;
- context-meter credit: `cursor:composer-input:<composer-id>` (one per conversation).

Each reconstructed call carries transient exact legacy keys for the bubble rows, `cursor:agentKv:<request-id>`, and old path-based transcripts it replaces. Archive insertion and deletion share one transaction. Only those listed rows are deleted after the canonical row is accepted; old rows whose source turn cannot be reconstructed remain untouched.

## Transcript capture (archive v7)

Every reconstructed turn also stores its full user message and assistant response text for Scrollback search, via the two archive-only `ParsedCall` fields (`transcript_user` / `transcript_assistant`) written to the archive's `transcripts` table during sync and never loaded back into memory; the display `user_message` stays truncated at 500 chars. Reasoning and redacted-reasoning blocks accumulate in their own buffer and are excluded from the captured assistant text (note that Cursor's `response_chars` *does* count reasoning length — the transcript does not). When a canonical row supersedes legacy bubble/AgentKv/transcript rows, the superseded rows' transcript entries are deleted in the same transaction. The adapter's fingerprint version is `cursor-v5-transcripts`; the v5 bump forces the one-time re-parse that backfills full text into existing archives.

## Coach

Cursor supports conversational-session and file-reference capabilities. It contributes to prompt quality, session shape, model use, tool loops, code output, project activity, and file-context findings whenever those fields exist.

Cursor remains excluded from cache-read and cancellation denominators: zero cache tokens mean “not locally reported”, and composer abort support is not yet broad enough for a cancellation-rate denominator. Timing-sensitive Coach calculations require `timestamp_quality == exact`; session/file timestamps still appear in period-filtered call lists but do not influence flow, pace, late-night/weekend, speed-accept, activity timelines, or language-over-time findings.

Model-overreliance groups by the shared canonical model identity, so thinking/effort/fast suffixes do not manufacture diversity. Premium checks use `(tool, model)` pricing so Cursor first-party rates are evaluated instead of a global fallback.

## Known gaps

- Current `store.db` has ordered messages but no per-message clock. Only state-backed joins have exact turn timing.
- Legacy chunked stores are deliberately not decoded; JSONL/TXT is the safe fallback.
- Store protobuf branch/subagent fields other than the linear field-1 transcript are not yet interpreted.
- Tracking timestamps are session/code-generation anchors, not exact request times.
- Cache reads/writes are not locally attributable and remain zero.
