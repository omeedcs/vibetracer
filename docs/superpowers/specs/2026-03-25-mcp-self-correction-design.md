# vibetracer MCP Server & Self-Correction Skill

**Date:** 2026-03-25
**Status:** Approved

## Problem

When an AI coding assistant makes a series of edits, it loses context about how the code evolved. If something breaks 10 edits in, the AI has no good way to pinpoint which change introduced the regression. vibetracer already captures the full edit timeline with frame-by-frame granularity — it just needs to be exposed back to the AI.

## Solution

Two parts:

1. **MCP Server** — `vibetracer mcp` subcommand exposing trace data as MCP tools over stdio
2. **Claude Skill** — `skills/vibetracer-review.md` orchestrating the self-correction workflow

## Architecture Decisions

- **Subcommand, not separate crate.** Shares the existing data model directly, single binary distribution.
- **Read-only MCP server.** The AI orchestrates bisect/restore logic itself. vibetracer provides the data.
- **Thin MCP layer.** No intermediate API types — tools return data derived directly from internal types (EditEvent, SessionMeta, etc.). A stable API layer can be extracted later if external clients depend on response shapes.
- **Pagination from the start.** All list-returning tools accept `offset`/`limit` params and stream JSONL lazily.
- **Live streaming included.** `subscribe_edits` connects to the daemon socket for real-time edit notifications.
- **Skill shipped with vibetracer.** Lives in `skills/` directory in this repo, not in an external skill pack.

## Module Structure

```
src/mcp/
├── mod.rs           # MCP server setup, tool routing
├── transport.rs     # stdio JSON-RPC transport (MCP protocol)
├── tools.rs         # Tool definitions and handlers
├── streaming.rs     # Live edit subscription via daemon socket
└── pagination.rs    # Shared offset/limit JSONL reader

skills/
└── vibetracer-review.md   # Claude skill for self-correction workflow
```

## MCP Tools

### `list_sessions`

List recorded trace sessions.

- **Params:** `limit?: u32`, `offset?: u32`
- **Returns:** `{ sessions: [{id, project_path, started_at, mode, agent_count, edit_count}], total_count: u32 }`
- **Implementation:** Reads `meta.json` from each session directory, counts edits from `edits.jsonl` line count.

### `get_timeline`

Get the edit timeline for a session.

- **Params:** `session_id: string`, `limit?: u32` (default 100, max 1000), `offset?: u32`, `file_filter?: string`
- **Returns:** `{ edits: [{id, ts, file, kind, lines_added, lines_removed, agent_label, operation_id, intent}], total_count: u32 }`
- **Implementation:** Streams `edits.jsonl` with pagination. Optional file path glob filter.

### `get_frame`

Get the exact state of files at a specific point in the timeline.

- **Params:** `session_id: string`, `frame_id: u32`, `file?: string`
- **Returns:** `{ files: [{path, content, hash}] }`
- **Implementation:** Does NOT replay diffs sequentially. Uses the shortcut: every EditEvent stores `after_hash`, and the snapshot store has full content for that hash. Algorithm:
  1. Collect all files touched up to frame N
  2. For each file, find the last edit at or before frame N
  3. Read content from snapshot store via `after_hash`
  4. For files in checkpoint but with no edits, read from checkpoint hash
- If `file` specified, returns just that file. Otherwise returns all files touched up to that frame.

### `diff_frames`

Diff between any two points in the timeline.

- **Params:** `session_id: string`, `frame_a: u32`, `frame_b: u32`, `file?: string`
- **Returns:** `{ diffs: [{path, diff}] }` — unified diff format
- **Implementation:** Calls `get_frame` for both points, diffs results using the `similar` crate. Optionally scoped to a single file.

### `search_edits`

Find frames where a specific function/line was modified.

- **Params:** `session_id: string`, `query: string`, `limit?: u32`, `offset?: u32`
- **Returns:** `{ edits: [{id, ts, file, kind, patch, intent}], total_count: u32 }`
- **Implementation:** Treated as a regex pattern (falls back to literal substring if the regex is invalid). Matched against patch text, file path, and intent field. Streams with pagination.

### `get_regression_window`

Get the candidate frames for bisecting a regression.

- **Params:** `session_id: string`, `file?: string`, `start_frame?: u32`, `end_frame?: u32`
- **Returns:** `{ frames: [{frame_id, file, patch, before_hash, after_hash}] }`
- **Implementation:** Filters edits by file and frame range. Returns the minimal set of frames the AI needs to inspect. Does NOT run tests — the AI orchestrates the bisect.

### `subscribe_edits`

Subscribe to live edit notifications from an active session.

- **Params:** `session_id: string`
- **Returns:** Initiates MCP notification stream. Each new edit sends `notifications/tools/edit_event` with the EditEvent payload.
- **Implementation:** See Live Streaming section.

## Live Streaming

### Daemon-Side Changes

Add two new message types to the existing Unix socket protocol in `hook_listener.rs`:

- **`Subscribe { session_id }`** — client wants edit notifications for this session
- **`EditNotification { event: EditEvent }`** — daemon pushes to each subscriber when a new edit is recorded

The daemon tracks subscriber connections. When a new edit is recorded, iterate subscribers and write the notification JSON. Subscribers are connected sockets that stay open.

### MCP-Side

When `subscribe_edits` is called:
1. Connect to `daemon.sock` as a client
2. Send `Subscribe` message
3. Forward each `EditNotification` as an MCP notification over stdio

### Backpressure

Buffer up to 100 notifications. If the client isn't reading fast enough, drop oldest. Edits are always recoverable via `get_timeline`.

### Cleanup

If daemon socket disconnects or MCP client disconnects, clean up gracefully. No retry logic — the AI can call `subscribe_edits` again.

## Pagination Module

Shared module for streaming JSONL with offset/limit:

```rust
pub struct PageParams {
    pub offset: u32,
    pub limit: u32,   // default 100, max 1000
}

pub fn read_edits_paged(
    jsonl_path: &Path,
    params: &PageParams,
    filter: Option<&dyn Fn(&EditEvent) -> bool>,
) -> Result<(Vec<EditEvent>, u32)>  // (events, total_count)
```

Streaming line reader — reads line by line, skips `offset` entries, collects up to `limit` matching entries, counts total. Never loads the full file into memory. The `filter` closure handles file path filtering, search queries, etc.

Returns `total_count` so the client knows whether more pages exist.

## Frame Reconstruction

`get_frame` uses a shortcut that avoids sequential diff replay:

1. Every `EditEvent` stores `after_hash` pointing into the content-addressed snapshot store
2. Find the last edit for each file at or before the target frame
3. Look up the content directly via `after_hash`
4. For files in the nearest checkpoint that have no subsequent edits, use the checkpoint's hash

This makes `get_frame` O(edits up to frame) for the scan but O(1) per file for content retrieval.

## Claude Skill

File: `skills/vibetracer-review.md`

### Workflow

1. **Load context** — Call `list_sessions` to find the active/latest session, then `get_timeline` to get the edit history
2. **Identify scope** — Group edits by operation/agent, build a model of what changed and in what order
3. **Run verification** — Execute the project's test suite or build command to find current failures
4. **If failures, bisect:**
   - Call `get_regression_window` to narrow candidate frames
   - Use `diff_frames` to inspect changes at suspicious points
   - Use `get_frame` to see exact file state at candidate frames
   - Identify the frame that introduced the regression
5. **Fix surgically** — Use `get_frame` to see the state before the bad edit, understand the intent, fix at the source
6. **Verify** — Re-run tests to confirm the fix

### MCP Server Configuration

Users add to their `.claude.json` or `claude_desktop_config.json`:

```json
{
  "mcpServers": {
    "vibetracer": {
      "command": "vibetracer",
      "args": ["mcp"]
    }
  }
}
```

## Error Handling

| Condition | Behavior |
|-----------|----------|
| Session not found | MCP error `-32602` with descriptive message |
| Frame out of range | Error with valid range (e.g., "frame 150 requested but session has 87 edits") |
| Active session, no edits | `get_timeline` returns empty array |
| Corrupt JSONL lines | Skip malformed lines, log warning, don't fail request |
| Daemon not running (`subscribe_edits`) | Error: "daemon is not active, start with `vibetracer daemon start`" |
| Concurrent writes to JSONL | Safe — JSONL append is atomic for reasonable line sizes, pagination reader handles truncated trailing lines |

## MCP Protocol

Implement JSON-RPC 2.0 over stdio per the MCP specification. No external MCP SDK dependency — the protocol is straightforward:

- Read JSON-RPC messages from stdin (newline-delimited)
- Write JSON-RPC responses/notifications to stdout
- Handle `initialize`, `tools/list`, `tools/call` methods
- Send `notifications/tools/edit_event` for streaming

## Testing Strategy

- **Unit tests:** Pagination module, frame reconstruction logic, tool handler input validation
- **Integration tests:** Spin up MCP server as a child process, send JSON-RPC over stdio, verify responses against known session data
- **Streaming test:** Start daemon, start MCP server, trigger file edits, verify notifications arrive
- **Edge cases:** Empty sessions, single-edit sessions, large sessions (generate synthetic JSONL), corrupt lines
