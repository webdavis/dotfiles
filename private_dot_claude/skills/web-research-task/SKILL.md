# Claude Web Research Task

## Purpose

This skill enables Claude Code to spawn a **dedicated background agent** that uses Claude in Chrome to
access Claude.ai's Research toggle, run deep research queries, and return comprehensive results.

**IMPORTANT**: This task MUST run as a background agent because Research takes 5 to 30+ minutes. Do NOT
skip this step or try to inline it into other workflows.

## Prerequisites

1. **Claude in Chrome Extension** installed and authenticated
1. **Claude Code** v2.0.60+ (for background agents)
1. **Paid Claude subscription** (Pro/Team/Max/Enterprise)
1. Chrome browser running

## How to Invoke

### Option 1: Background Agent (Recommended)

```bash
claude --chrome
```

Then in Claude Code:

```
Run the web research task as a background agent for: "Your research query here"
```

Press `Ctrl+Shift+B` or tell Claude to run it in background.

### Option 2: Slash Command

```
/research-via-web "Your query here"
```

## Workflow Steps

The background agent will:

1. **Open new Chrome tab** to https://claude.ai
1. **Wait for page load** (check for chat interface)
1. **Click "Research" toggle** (bottom left of chat interface)
1. **Enter the research query** in the chat input
1. **Submit and wait** for research to complete (monitor for "Research complete" or final output)
1. **Extract the full response** including citations
1. **Save to file**: `./docs/research/[timestamp]-[query-slug].md`
1. **Return summary** to main Claude Code session

## Critical Instructions for Claude

When executing this task:

```
YOU MUST:
1. Run this as a BACKGROUND AGENT (use run_in_background: true)
2. DO NOT attempt to speed this up or skip steps
3. WAIT for the Research feature to fully complete (can take 5 to 30 minutes)
4. Poll the page every 30 seconds to check completion status
5. Only extract results AFTER seeing the full research output

YOU MUST NOT:
1. Try to inline this into other tasks
2. Skip the Research toggle (regular chat is NOT the same)
3. Return partial results
4. Timeout before completion
```

## Expected Timeline

| Phase                 | Duration         |
| --------------------- | ---------------- |
| Navigate to claude.ai | 5 to 10 seconds  |
| Toggle Research       | 2 to 5 seconds   |
| Enter query           | 5 to 10 seconds  |
| Research execution    | 5 to 30 minutes  |
| Extract results       | 10 to 30 seconds |
| Save to file          | 2 to 5 seconds   |

**Total: 6 to 35 minutes** (mostly waiting for Research)

## Output Format

Results are saved as Markdown:

```markdown
# Research: [Query Title]

**Generated**: [Timestamp]
**Duration**: [X minutes]
**Source**: Claude.ai Research Feature

---

[Full research content with citations]

---

## Sources
[List of sources cited]
```

## Troubleshooting

| Issue                       | Solution                                                          |
| --------------------------- | ----------------------------------------------------------------- |
| Research toggle not visible | Ensure you have a paid Claude plan                                |
| Page not loading            | Check Chrome is running, extension is active                      |
| Timeout                     | Research can take 30+ min for complex queries; increase wait time |
| Login required              | Open Claude in Chrome sidebar first to authenticate               |

## Integration with Your Workflow

Add to your project's `CLAUDE.md`:

```markdown
## Research Tasks

When I need deep research, use the `/research-via-web` command.
This spawns a background agent that uses Claude.ai's Research feature.
DO NOT try to speed this up. Let it run to completion.
```
