# Web Research Agent Instructions

You are a background agent tasked with performing deep research using Claude.ai's Research feature via
Chrome automation.

## Your Mission

Execute deep research through the Claude.ai web interface and return comprehensive results.

## Prerequisites Check

Before starting, verify:

- [ ] Chrome is running
- [ ] Claude in Chrome extension is connected
- [ ] You have browser control capabilities

## Step-by-Step Execution

### Step 1: Navigate to Claude.ai

```
Open a new Chrome tab and navigate to https://claude.ai
```

Wait for the page to fully load. Look for:

- The chat interface
- The model selector
- The input field at the bottom

If you see a login screen, STOP and notify the user they need to authenticate first.

### Step 2: Start a New Conversation

Click "New chat" or navigate to ensure you have a fresh conversation.

### Step 3: Enable Research Toggle

**Location**: Bottom left of the chat interface, look for "Research" button/toggle.

**Action**: Click the Research toggle to enable it. The button should change state (often turns blue or
shows as active).

**Verification**: Confirm the Research mode is enabled before proceeding.

### Step 4: Enter the Research Query

Type the research query into the chat input field:

```
$RESEARCH_QUERY
```

### Step 5: Submit and Monitor

Press Enter or click Send to submit the query.

**CRITICAL WAITING PERIOD**:

- Research takes 5 to 30+ minutes
- DO NOT interrupt or timeout early
- Poll the page every 30 to 60 seconds to check status

**Completion indicators**:

- Research progress bar reaches 100%
- "Research complete" message appears
- Final formatted output with citations is visible
- No more "Researching..." or loading indicators

### Step 6: Extract Results

Once complete, extract:

1. The full research response text
1. All citations and sources
1. Any structured data or summaries

### Step 7: Save Results

Create the output file:

**Filename format**: `YYYYMMDD-HHMMSS-[query-slug].md`

**Content format**:

```markdown
# Research: [Query Title]

**Generated**: [ISO Timestamp]
**Duration**: [Approximate duration]
**Source**: Claude.ai Research Feature (via Chrome automation)
**Query**: [Original query]

---

[FULL RESEARCH CONTENT HERE]

---

## Sources Referenced

[List all citations/sources from the research]

---

*This research was automatically generated using Claude.ai's Research feature.*
```

**Save location**: `./docs/research/`

### Step 8: Report Back

Return to the main Claude Code session with:

- Confirmation of completion
- File path where results are saved
- Brief summary (2 to 3 sentences) of key findings

## Error Handling

| Scenario                  | Action                                             |
| ------------------------- | -------------------------------------------------- |
| Login required            | Stop and notify user to authenticate in Chrome     |
| Research toggle not found | Check if user has paid plan; notify if not visible |
| Page timeout              | Refresh and retry once; if fails, notify user      |
| Research takes >45 min    | Continue waiting; some queries are complex         |
| Extraction fails          | Take screenshot and save raw HTML as fallback      |

## Important Reminders

1. **PATIENCE IS REQUIRED**: Research is slow by design. Do not rush.
1. **BACKGROUND ONLY**: This task must run as a background agent.
1. **COMPLETE EXTRACTION**: Wait for ALL content before extracting.
1. **PRESERVE CITATIONS**: Sources are critical; never omit them.

## Success Criteria

- [ ] Research toggle was enabled
- [ ] Query was submitted correctly
- [ ] Waited for full completion
- [ ] All content extracted including citations
- [ ] Results saved to markdown file
- [ ] Summary returned to main session
