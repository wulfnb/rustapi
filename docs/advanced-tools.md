# Optional API tools

These tools run locally without an account. The desktop toolbar opens **API tools**,
**Protocols**, **Visual flows** and **Self-hosted tools**. The headless commands below
accept `--workspace DIR`; use the same directory as the desktop. Authentication and
encrypted cookies/tokens are described in [authentication](authentication.md).

## Specifications, contracts and documentation

In **API tools**, open or paste an OpenAPI 3.0/3.1 JSON/YAML specification. Lint it,
import its operations, save edits or compare another specification. Drafts recover
locally after restart. Saving over the opened file checks that its contents have
not changed on disk. Import omissions require review before a collection is saved.

```sh
rustapi spec lint api.yaml
rustapi spec format api.yaml api.json
rustapi spec drift previous.yaml current.yaml
rustapi import api.yaml --workspace ./workspace
```

Imports generate tag folders, server variables, parameter/body examples, supported
authentication and response contract assertions. Contracts check response status,
media type and JSON Schema. Drift reports added, removed or changed operations,
resolved schema changes, path parameters, servers and security definitions.
Changes are conservative review signals, not a semantic compatibility guarantee.

The parser is bounded to 8 MiB and resolves local, nonrecursive references. External
and recursive references are rejected. This is a practical lint/import subset, not
a complete OpenAPI validator. Complex parameter serialization, callbacks/webhooks
and non-JSON response schemas require manual review. Request-schema validation and
automatic updates of an existing collection from a changed specification are not
implemented; import into a new collection and review the differences.

Select a saved collection and **Generate offline Markdown + HTML documentation**,
or run:

```sh
rustapi docs ./workspace/collections/COLLECTION_ID.json ./api-docs
```

This creates `index.html` and Markdown in a new directory, with requests and saved
examples. HTML is script-free, works offline and escapes source content. Known
credential fields are redacted by default. `--include-credentials` deliberately
includes them. Review free-form descriptions and payloads before distributing files;
redaction cannot identify every application-specific secret. Existing output
directories are never replaced. Publish these files with your static host, or enable
the team server's optional documentation endpoint described below.

## Example-backed mocks

Capture/save response examples first. In **API tools**, select the collection and
create mock rules, then expand the rules editor to edit, save and start them.

```sh
rustapi mock create ./workspace/collections/COLLECTION_ID.json mock.json
rustapi mock serve mock.json
```

The server prints its loopback URL. Rules match in order by method, path, query,
headers, body substring and/or JSON Pointer values. Paths accept `:id`, `{id}` and a
terminal `*`. `X-RustAPI-Example` selects a named rule. Responses retain example
headers and exact base64-encoded bytes. Configure `delay_ms` (0–30000), status
200–599, or `disconnect: true` to simulate faults. Unmatched requests return 404.
There are at most 1000 rules, 1 MiB per response and 16 simultaneous connections.
Mocks bind to loopback and stop with the app/process; they are not a hosted service.

## Saved runs, datasets and schedules

Expand **Saved runs, datasets and scheduling** in **API tools**. Choose a collection
saved inside the workspace, import a CSV/JSON dataset, configure an environment,
iterations and assertions, then save or execute the profile. Paths are relative to
the workspace; traversal and symlinks are rejected. Secrets remain vault references.

A profile file can omit default fields:

```json
{
  "id": "33493155-1a26-4e22-b5be-4de542373d7c",
  "name": "API regression",
  "collection": "collections/COLLECTION_ID.json",
  "dataset": "datasets/users.csv",
  "options": {"iterations": 0, "stop_on_failure": true},
  "interval_seconds": 3600,
  "alert_url_env": "RUSTAPI_ALERT_URL"
}
```

```sh
rustapi automation dataset users.csv users.csv --workspace ./workspace
rustapi automation save profile.json --workspace ./workspace
rustapi automation list --workspace ./workspace
rustapi automation run PROFILE_ID --workspace ./workspace
rustapi automation history --workspace ./workspace
rustapi automation compare BEFORE_RUN_ID AFTER_RUN_ID --workspace ./workspace
rustapi automation watch --workspace ./workspace
```

Profiles live in `runs/profiles/`, datasets in `datasets/`, and result metadata in
`.local/runs/`. Zero iterations uses dataset size. Datasets are bounded to 8 MiB and
10000 rows. Globals and local environment overrides resolve as in normal runs.
History compares checks, status and added/removed requests; it does not compare
complete response bodies. The newest 200 generated records are retained per report
directory; ordinary run-history loading has a 128 MiB aggregate budget.

Scheduling is opt-in and requires the app scheduler or `automation watch` to stay
running. There is one scheduler per workspace, no overlapping scheduled runs, a
persisted next-due time, and intervals of 60 seconds to 30 days. Missed intervals
coalesce. This does not install an OS service or wake a sleeping computer. Failed
runs can POST a metadata-only alert to the URL in the named environment variable.
Alerts require HTTPS or loopback HTTP, do not follow redirects and have a 10-second
deadline; delivery failures are recorded in the run result.

## Performance testing

In **API tools → Performance testing**, use a saved run profile, set virtual users,
duration/ramp/pacing, request cap and optional error/p95 thresholds. Add stages for
changing load, or save/load the complete configuration.

```json
{
  "run": {
    "id": "33493155-1a26-4e22-b5be-4de542373d7c",
    "name": "Load workload",
    "collection": "collections/COLLECTION_ID.json"
  },
  "users": 4,
  "duration_seconds": 30,
  "ramp_seconds": 5,
  "pacing_ms": 100,
  "max_requests": 1000,
  "max_error_rate": 0.01,
  "max_p95_ms": 1000
}
```

```sh
rustapi load run load.json --workspace ./workspace
rustapi load compare before.json after.json
```

Each virtual user has an isolated HTTP/cookie/token session and repeats the saved
collection with its dataset. `__rustapi_vu` and `__rustapi_iteration` identify the
worker/iteration. Persistent OAuth caching is rejected for load runs; use independent
credentials or session-only token acquisition. Caps: 1–64 users, 1–3600 seconds,
1–100000 request executions. Script-directed loops count toward the same execution
cap; authentication challenges/token refresh can add HTTP exchanges within one
execution. Metrics describe the saved request executions.
Stages replace constant/ramp timing. Reports include throughput, errors, per-request
metrics and nearest-rank p50/p90/p95/p99 latency in `.local/load-runs/`. Comparisons
report configuration hashes so unlike workloads are visible. This is a single-host
load generator; machine/network limits affect measured throughput.

## GraphQL and live protocols

For GraphQL over HTTP, enable **Body → GraphQL document**. Edit query/mutation,
variables JSON and optional operation name; format the document or insert the
introspection query. Paste SDL/introspection output in the schema panel and select
**Inspect schema fields** for its type-field catalog. Syntax, operation selection
and variables are checked before sending; GraphQL errors fail runner checks.
The catalog does not provide full schema-aware IDE completion or execute subscriptions.

Use **Protocols** for live connections. Save/load JSON profiles and connect,
disconnect or send messages while connected. Received events remain in memory;
the desktop keeps the latest 1000 event previews. CLI events are newline-delimited JSON:

```sh
rustapi protocol run session.json --workspace ./workspace
```

Example SSE profile:

```json
{
  "protocol": "Sse",
  "request": {"url": "http://127.0.0.1:8080/events"},
  "duration_seconds": 30,
  "max_events": 100
}
```

| Protocol value | Supported behavior | Boundary |
| --- | --- | --- |
| `Sse` | Incremental HTTP events, multiline data, IDs/retry fields, normal HTTP authentication | No automatic reconnect/replay loop |
| `WebSocket` | ws/wss text/binary receive, live text sends, ping/pong, handshake cookies and Basic/Bearer/JWT/API-key auth | No custom CA/client PEM/proxy, Digest or SigV4 handshake helper |
| `SocketIo` | Engine.IO 4 WebSocket transport, namespaces, auth JSON, named JSON events, heartbeat | No polling fallback, binary events or acknowledgements |
| `Mqtt` | MQTT 3.1.1 publish/subscribe, QoS 0/1/2, retain, user/password, verified TLS | No MQTT 5, custom CA/client PEM or proxy |
| `Grpc` | Reflection list/describe, proto/protoset, unary and server-stream JSON, metadata, TLS/mTLS | Requires external `grpcurl`; no client/bidirectional streaming editor |

Install `grpcurl` separately on PATH or set `RUSTAPI_GRPCURL`. Its version is not
managed by the application. Use `grpc_action` (`invoke`, `list`, `describe`),
`grpc_method`, and optional `grpc_descriptor` (a `.proto` or descriptor-set path).
Endpoints use `grpc://host:port` or `grpcs://host:port`. Request `body` is protobuf
JSON and `headers` are metadata; supported HTTP auth resolves into metadata.

Sessions are bounded to 1–3600 seconds, 1–10000 events and 16 MiB aggregate event
payload. Oversized messages, unsupported settings and cancellation are explicit.
Profiles can contain plaintext configuration: use `{{vault:name}}` references
instead of saving passwords or tokens directly.

## Self-hosted collaboration and publishing

In **Self-hosted tools → Host a team server**, create an administrator token, save
configuration and start the server. Copy the newly issued token before closing;
only its hash is stored. Additional users have Admin, Editor or Viewer roles with
expiry. Configuration changes take effect after restarting the server.

```sh
rustapi team init team.json
rustapi team token team.json colleague Editor 30
rustapi team serve team.json ./team-data
```

The server binds to `127.0.0.1` (default port 9090; 0 chooses a port). Expose it
through a TLS reverse proxy if colleagues need remote access. The proxy must forward
the loopback upstream Host, Authorization and If-Match headers. Store team-data on
the server, separately from client workspaces. Configuration holds token hashes;
the init/token command outputs the bearer token once. Put a client token in the
desktop encrypted vault under `team_token`, or provide `RUSTAPI_TEAM_TOKEN` to CLI:

```sh
rustapi team catalog https://api-team.example.test
rustapi team push https://api-team.example.test collection.json new
rustapi team pull https://api-team.example.test COLLECTION_ID pulled.json
```

`new` creates a collection; subsequent pushes require the revision returned by the
previous push/pull. Viewer can read, Editor/Admin can write. Conflicting revisions
return 409 and require pull/review; there is no force overwrite. The desktop tracks
the last pulled/pushed revision and imports a remote collection as a separate local
copy. Known literal credentials are redacted on push, while vault references remain.
Catalog metadata is local to that server. This is collection synchronization, not
simultaneous editing, presence or automatic merges.

Writes append a metadata-only audit intent before committing a revision. The audit
file is bounded to 8 MiB and further writes fail until it is archived by the server
operator; revision IDs distinguish committed writes. A process lock protects one
server data directory. No external database or login service is required.

The authenticated `/v1/docs/COLLECTION_ID` endpoint generates offline HTML from a
stored collection. **Publish documentation without authentication** (or config
`public_documentation: true`) deliberately makes this endpoint public. Catalog and
collection APIs remain authenticated. Nothing is deployed to a remote host by
starting the local server.

## Visual flows

**Visual flows** builds a graph from a saved collection. Drag nodes, select a request,
set the success/failure destinations, choose the starting node and save. Each node
uses the normal scripts, assertions, extraction and authentication pipeline. Explicit
checks determine success; absent checks require HTTP 2xx. Extracted variables carry
forward. Failure edges allow recovery steps such as login-and-retry. A terminal
success can complete a recovered flow successfully.

```sh
rustapi flow save flow.json --workspace ./workspace
rustapi flow run flow.json --workspace ./workspace
```

Flows live in `flows/`, results in `.local/flow-runs/`. Loops have explicit step and
duration caps (up to 1000 steps/3600 seconds), with cancellation. This is a request
graph, not an arbitrary code/process orchestration system.

## AI drafting and MCP

**Self-hosted tools → AI request drafting** connects to an explicitly configured
Ollama-compatible endpoint. List models, choose one, enter a prompt and optionally
include a redacted collection as reference data. The selected endpoint receives
that content. Results open as unsent drafts for review; generated scripts, auth
helpers, transport overrides, uploads and examples are discarded. A working Ollama
server/model must be installed separately. CLI equivalents:

```sh
rustapi ai models http://127.0.0.1:11434
rustapi ai draft http://127.0.0.1:11434 MODEL prompt.txt draft.json
```

For MCP clients, configure a stdio command:

```json
{"command":"rustapi","args":["mcp","serve","--workspace","/absolute/workspace"]}
```

Tools expose the local catalog and redacted saved request descriptions. Add
`--allow-execution` only when the MCP client should run saved profiles. Workspace
policy must also permit execution. Stdio supports discovery/per-request metadata
for MCP 2026-07-28 and legacy initialize handshakes; it accepts cancellation while
requests run, with at most eight active calls. It provides tools, not MCP resource,
prompt, remote HTTP or arbitrary command APIs. Protocol details follow the
[official MCP specification](https://modelcontextprotocol.io/specification/2026-07-28).

## External secret managers and governance

Configure `secret-providers.json` in the workspace. Provider credentials come from
process environment variables, not the tracked configuration. For Vault KV v2:

```json
{
  "providers": [{"name":"vault","kind":"VaultKv2","endpoint":"https://vault.example.test","token_env":"VAULT_TOKEN","mount":"secret"}],
  "bindings": [{"alias":"api-token","provider":"vault","secret_id":"services/api","pointer":"/token"}]
}
```

For AWS Secrets Manager use kind `AwsSecretsManager`, a `region`, optional custom
`endpoint`, and environment `AWS_ACCESS_KEY_ID`, `AWS_SECRET_ACCESS_KEY`, optional
`AWS_SESSION_TOKEN`. A binding's `secret_id` names the secret and `pointer` selects
its JSON field (empty pointer selects a whole string value). AWS requests use SigV4;
this implementation uses explicit environment credentials, not an SDK credential
chain/role-assumption flow.

Refresh explicitly in **External secrets and MCP tools**, or use
`rustapi secrets check --workspace DIR` to validate aliases without printing values.
CLI runs resolve configured providers automatically. Values resolve as
`{{vault:ext/api-token}}`, stay in memory and are separate from persistent local
vault entries. Locking clears session caches. Endpoints require HTTPS or loopback
HTTP, have 5-second deadlines and do not follow redirects.

**Workspace execution policy** edits `workspace-policy.json`: require encrypted
transports, disable certificate overrides/scripts/AI/external providers/MCP execution,
and restrict destination hosts (exact names or `*.example.test`). Saving reloads
the service policy; malformed files fail closed.

Team client connections and failure-webhook delivery also check the workspace host
and transport restrictions before connecting.

CLI:

```sh
rustapi policy template workspace-policy.json
rustapi policy validate --workspace ./workspace
```

Defaults preserve unrestricted account-free local use. These are application
execution controls; someone who can edit the workspace can change its policy.
Governance covers server roles, expiring tokens, revision checks, audit and workspace
policy. SSO/SAML/SCIM, organization administration and centralized immutable audit
retention are not implemented.
