# /research-via-web

Execute deep research using Claude.ai's Research feature via Chrome automation.

## Usage

```
/research-via-web "Your research query here"
```

## Behavior

This command MUST be run as a **background agent**. When invoked:

1. Spawns a dedicated background agent
1. Uses Claude in Chrome to navigate to claude.ai
1. Enables the Research toggle
1. Submits the query
1. Waits for full completion (5 to 30 minutes)
1. Extracts and saves results to `./docs/research/`

## CRITICAL: Do Not Skip Background Execution

Research takes significant time. This command is designed to run autonomously while you continue other
work.

**Correct invocation:**

```
Run /research-via-web as a background agent: "Latest Laravel 12 features and migration guide"
```

**Or press Ctrl+Shift+B** after invoking to send to background.

## Arguments

| Argument | Required | Description                    |
| -------- | -------- | ------------------------------ |
| query    | Yes      | The research question or topic |

## Examples

```
/research-via-web "Compare Inertia.js vs Livewire for Laravel SPAs in 2026"
```

```
/research-via-web "SharePoint Framework SPFx best practices and performance optimization"
```

```
/research-via-web "SQL Server query optimization techniques for large datasets"
```

## Output Location

Results saved to:

```
./docs/research/YYYYMMDD-HHMMSS-query-slug.md
```

## Checking Status

While the background agent runs:

```
Check status of background agents
```

Or use `/bashes` to see all background tasks.
