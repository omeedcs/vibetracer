---
name: vibetracer-review
description: Self-correction workflow — scrub through vibetracer edit history to identify and fix regressions introduced during AI-assisted coding
---

# vibetracer Self-Correction Review

Use this skill when tests fail or behavior regresses after a series of AI-assisted edits. It uses vibetracer's MCP tools to scrub through the edit timeline and surgically fix the regression at its source.

## Prerequisites

- vibetracer must be installed and recording (running as daemon or in TUI)
- The vibetracer MCP server must be configured:

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

## Workflow

### Phase 1: Load Context

1. Call `list_sessions` to find the active or most recent session
2. Call `get_timeline` with the session ID to get the full edit history
3. Note the total number of edits, which files were touched, and the edit range

### Phase 2: Identify Scope

1. Group edits by `operation_id` to understand logical units of work
2. Group edits by file to see which files changed most
3. Identify the "before" state (frame 1 or the start of the current work)

### Phase 3: Run Verification

1. Run the project's test suite or build command
2. If everything passes, report success and stop
3. If there are failures, note the specific errors and failing tests

### Phase 4: Bisect the Regression

1. Call `get_regression_window` with the relevant file filter to narrow candidates
2. Start a binary search through the candidate frames:
   a. Pick the midpoint frame
   b. Call `get_frame` at that point to see the file state
   c. Use `diff_frames` to compare the midpoint against the known-good state
   d. Assess whether the regression-causing change is before or after this point
   e. Narrow the window and repeat
3. Once you identify the exact frame that introduced the issue, call `diff_frames` between it and the previous frame to see exactly what changed

### Phase 5: Fix Surgically

1. Call `get_frame` at the frame just before the regression to see the intended state
2. Understand what the edit was trying to do (check the `intent` field)
3. Write a targeted fix that preserves the intent but corrects the error
4. Do NOT revert the entire edit — fix the specific issue

### Phase 6: Verify Fix

1. Re-run the test suite to confirm the regression is fixed
2. Run `get_timeline` again to confirm your fix was recorded
3. Report what was found, what frame introduced it, and what was fixed

## Tips

- Use `search_edits` with a regex pattern to quickly find frames that touched a specific function or variable
- If multiple regressions exist, fix them one at a time, re-running tests after each
- Use `subscribe_edits` if you want live notifications as new edits are recorded
- The `file_filter` parameter on `get_timeline` is useful for narrowing to a specific file's history
