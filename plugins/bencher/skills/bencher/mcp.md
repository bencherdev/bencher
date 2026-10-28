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

Bencher is in the Claude connectors directory
(https://claude.ai/directory/bencher), which serves claude.ai, Claude Desktop,
Claude Mobile, and Claude Code. Claude users add the Bencher connector and
sign in with their Bencher account. Claude Code uses the connectors added in
claude.ai when it is logged in with a claude.ai subscription. The connector
reaches Bencher Cloud only.

Bencher is in the OpenAI plugins directory
(https://chatgpt.com/plugins/plugin_asdk_app_6ad5be4f4e108191b3447b372bf3244e),
which serves ChatGPT and Codex. ChatGPT and Codex users add the Bencher plugin
and sign in with their Bencher account. In Codex, the plugin also brings this
skill. The plugin reaches Bencher Cloud only.

Use the URL instead with other MCP clients, with Claude Code without a
claude.ai subscription login, with a self-hosted server, and in CI:

| Deployment | URL |
|------------|-----|
| Bencher Cloud | `https://mcp.bencher.dev/mcp` |
| Self-Hosted (Plus) | `https://<api-host>/mcp` (default: `http://localhost:6610/mcp`) |

There are three ways to call it:

- **OAuth sign-in**: clients with MCP OAuth support (Claude Code, and ChatGPT
  and claude.ai with a self-hosted server added by its URL) connect with no
  credential and sign in when asked.
  The client opens Bencher's consent page in the user's browser, where the user
  signs in and approves the app. The agent never handles a key. A self-hosted
  server offers this only once its `plus.oauth` config section is set.
- **API key**: the same `Authorization: Bearer` credentials as the REST API, a
  project API key (`bencher_run_*`) or a user API key (`bencher_user_*`).
- **Anonymous**: public projects can be read without any credential.

Claude Code without the connector, signing in with OAuth (run `/mcp` in
Claude Code to sign in when a tool asks):
```bash
claude mcp add --transport http bencher https://mcp.bencher.dev/mcp
```

Claude Code with an API key:
```bash
claude mcp add --transport http bencher https://mcp.bencher.dev/mcp \
  --header "Authorization: Bearer $BENCHER_API_KEY"
```

Generic MCP client configuration with an API key:
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

## OAuth

- Scopes: `projects:read` covers the read tools (`list_*`, `view_*`,
  `query_perf`, `perf_image`), private projects included. `projects:write`
  covers every other tool and includes read. A request naming neither gets
  `projects:read`. `openid`, `email`, and `profile` (OpenID Connect) are
  offered only when the server has `plus.oauth.signing_key`. The user approves
  every connection, for all of the requested scopes or none.
- A token acts as its user: project roles still decide every project, and it
  never carries server admin authority.
- Access tokens (`bencher_oauth_*`) last 1 hour. Refresh tokens rotate on every
  use and lapse after 30 days unused; presenting a rotated refresh token again
  revokes the connection. A connection lasts at most one year, and one
  approved with a user API key ends when that key is revoked or expires.
- OAuth access tokens are for the MCP server (and the OAuth UserInfo
  endpoint): REST endpoints refuse them with `401`.
- Users see and revoke connections under Connected Apps in the console
  (`/console/users/apps`), while logged in (not with an API key).
- Discovery: protected resource metadata at
  `/.well-known/oauth-protected-resource/mcp`, authorization server metadata at
  `/.well-known/oauth-authorization-server`. Clients register with a Client ID
  Metadata Document (no dynamic client registration), use PKCE (`S256`), and
  never authenticate (public clients only).

## Anonymous Access and Rate Limits

- Anonymous requests share a rate limit per IP address, so everyone behind one
  address (a hosted client's egress included) shares it. Signed-in requests
  are charged to the user, or to a project key's project, never to the IP.
- With OAuth configured, an anonymous tool call that needs a signed-in user
  gets a sign-in challenge instead of an error: reading a private project (a
  nonexistent project or ID answers identically, so a mistyped ID within a
  public project also challenges), any tool that writes except `submit_run` to
  an unclaimed project, `list_projects` for an organization's projects, and any
  tool call over the per-IP limit.
- With OAuth configured, every tool declares `_meta.securitySchemes`: `noauth`
  when it works anonymously, and `oauth2` with the scope it needs.

## Tool Surface

The tool surface mirrors what a project API key (`bencher_run_*`) can do:
project-scoped reads and writes, with no delete tools. There are no rename
tools, and nothing outside a single project. A user key or an OAuth token gets
the same tool set applied across all of the user's projects.

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
- A malformed, invalid, or expired credential fails the whole request with
  HTTP `401` (a well-formed JWT that fails validation is a `400`), so an OAuth
  client refreshes its token or signs in again.
- With OAuth configured, a tool that needs sign-in, or a scope the token lacks,
  answers with a sign-in challenge (HTTP `401` or `403`). Some clients receive it
  as a tool result with `isError: true` instead.
- Other authorization and validation failures (`400`/`403`/`404`) come back
  as tool results with `isError: true` carrying the REST status code and
  message.
