# Bencher MCP Server

Bencher hosts an MCP (Model Context Protocol) server so AI agents can query
benchmark data and submit results without the `bencher` CLI installed.
It is a Bencher Plus feature, served by the API server itself.
Docs: https://bencher.dev/docs/how-to/mcp/

**The CLI is the recommended route** when it is available, especially for
running benchmarks (`bencher run` handles adapters, iterations, and CI
integration). Use MCP when the CLI is not installed, when you want structured
tool output instead of shell parsing, or from MCP clients other than a shell.

## Connecting

| Deployment | URL |
|------------|-----|
| Bencher Cloud | `https://mcp.bencher.dev/mcp` |
| Self-Hosted (Plus) | `https://<api-host>/mcp` (default: `http://localhost:6610/mcp`) |

Authentication uses the same `Authorization: Bearer` credentials as the REST
API: a project API key (`bencher_run_*`) or a user API key (`bencher_user_*`).
Public projects can be read without any credential. OAuth is not yet
supported, so clients that only support OAuth for remote servers (e.g.
claude.ai custom connectors) cannot connect yet.

Claude Code:
```bash
claude mcp add --transport http bencher https://mcp.bencher.dev/mcp \
  --header "Authorization: Bearer $BENCHER_API_KEY"
```

Generic MCP client configuration:
```json
{
  "mcpServers": {
    "bencher": {
      "type": "http",
      "url": "https://mcp.bencher.dev/mcp",
      "headers": {
        "Authorization": "Bearer ${BENCHER_API_KEY}"
      }
    }
  }
}
```

## Tool Surface

The tool surface mirrors what a project API key (`bencher_run_*`) can do:
project-scoped reads and non-destructive writes. There are no delete or
rename tools, and nothing outside a single project. A user key gets the same
tool set applied across all of the user's projects.

| Tools | Purpose |
|-------|---------|
| `submit_run` | Submit raw benchmark harness output; the server parses it (adapter defaults to `magic`) and creates branches, testbeds, and thresholds on the fly |
| `query_perf`, `perf_image` | Query metrics over time; render a perf plot (JPEG) |
| `list_projects`, `view_project` | Project discovery |
| `list_reports`, `create_report`, `view_report` | Reports |
| `list_branches`, `create_branch`, `view_branch`, `update_branch` | Branches (no rename with a project key) |
| `list_testbeds`, `create_testbed`, `view_testbed`, `update_testbed` | Testbeds (no rename with a project key) |
| `list_benchmarks`, `create_benchmark`, `view_benchmark`, `update_benchmark` | Benchmarks (no rename with a project key) |
| `list_measures`, `create_measure`, `view_measure`, `update_measure` | Measures (no rename with a project key) |
| `list_thresholds`, `create_threshold`, `view_threshold`, `update_threshold` | Thresholds |
| `list_alerts`, `view_alert`, `update_alert` | Alerts (status-only updates with a project key) |
| `view_metric` | A single metric with full context |
| `list_plots`, `view_plot` | Saved dashboard plots |
| `list_jobs`, `view_job` | Bare metal runner jobs (Bencher Plus) |

Notes:
- Most tools take a `project` argument that accepts a project slug or UUID.
- List tools return `{ "data": [...], "total_count": N }` and accept the same
  pagination arguments as the REST API (`per_page`, `page`).
- `query_perf` takes comma-separated UUID lists for `branches`, `testbeds`,
  `benchmarks`, and `measures`; use the list tools to find UUIDs first.
- Tool errors carry the REST status code and message
  (e.g. `403: Project keys cannot rename testbeds`).

## Typical Investigation Flow

1. `view_project` to confirm the project.
2. `list_alerts` to find active performance alerts.
3. `view_alert` for the boundary that was exceeded.
4. `list_branches` / `list_testbeds` / `list_benchmarks` / `list_measures`
   to collect UUIDs.
5. `query_perf` to pull the metric history around the regression.

## Protocol Details

- Streamable HTTP transport, stateless: each POST is an independent JSON-RPC
  message answered with `application/json`. No SSE, no sessions.
- Protocol versions `2025-03-26` and newer are supported.
- Errors use two channels: invalid or expired credentials fail the whole
  request with HTTP `401`, like the REST API, while authorization and
  validation failures (`400`/`403`/`404`) come back as tool results with
  `isError: true` carrying the REST status code and message.
