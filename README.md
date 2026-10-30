<p align="center">
  <img src="assets/icons/rustapi-icon-256.png" width="128" height="128" alt="RustAPI Logo">
</p>

# RustAPI

A local-first REST client built in Rust and egui. Create Git-friendly workspaces,
import request collections, send requests, test responses, and run the same collections
from the desktop or a headless CLI. No account or cloud service is required.

[**Website & Downloads**](https://wulfnb.github.io/rustapi/) · [**GitHub Releases**](https://github.com/wulfnb/rustapi/releases) · [**Feature Roadmap**](docs/feature-gap-analysis.md)

## Download

[Website](https://wulfnb.github.io/rustapi/) · [All releases](https://github.com/wulfnb/rustapi/releases) · [SHA256 checksums](https://github.com/wulfnb/rustapi/releases/latest/download/SHA256SUMS.txt)

| Platform | Download Preview |
| --- | --- |
| macOS · Apple Silicon (ARM64) | [Download DMG](https://github.com/wulfnb/rustapi/releases/latest/download/RustAPI-macos-arm64.dmg) · [Download ZIP](https://github.com/wulfnb/rustapi/releases/latest/download/RustAPI-macos-arm64.zip) |
| macOS · Intel (x86_64) | [Download DMG](https://github.com/wulfnb/rustapi/releases/latest/download/RustAPI-macos-x86_64.dmg) · [Download ZIP](https://github.com/wulfnb/rustapi/releases/latest/download/RustAPI-macos-x86_64.zip) |
| Windows · x64 | [Download Setup EXE](https://github.com/wulfnb/rustapi/releases/latest/download/RustAPI-windows-x86_64-Setup.exe) · [Download ZIP](https://github.com/wulfnb/rustapi/releases/latest/download/RustAPI-windows-x86_64.zip) |
| Linux · x86_64 | [Download tar.gz](https://github.com/wulfnb/rustapi/releases/latest/download/RustAPI-linux-x86_64.tar.gz) |

On macOS, open the DMG and drag **RustAPI.app** into **Applications**, then launch the installed app from Applications. The release is ad-hoc signed, so macOS may require you to approve that specific app in **System Settings → Privacy & Security → Open Anyway** after trying to open it.

### Docker Quickstart (Browser Desktop)

Run the full RustAPI desktop UI inside Docker, accessed via loopback HTTPS and noVNC in your browser:

```sh
docker run -d --name rustapi-client \
  -p 127.0.0.1:7443:7443 \
  -e API_USER=tester \
  -e API_PASSWORD='choose-a-strong-password' \
  -v rustapi-data:/config \
  ghcr.io/wulfnb/rustapi:latest
```

Open `https://127.0.0.1:7443` in your browser and log in with your credentials.

This release targets the everyday REST workflow. It implements a documented subset
of collection scripting APIs; imported scripts must use the supported subset.
See [migration and compatibility](docs/import-compatibility.md).

See the [feature gap analysis](docs/feature-gap-analysis.md) and
[prioritized implementation plan](docs/implementation-plan.md) for the
2026-10-29 review of current capabilities, remaining gaps and planned work.

## Features

- Collections and nested folders with inherited variables/authentication, rename,
  duplication, move, and ordering controls.
- Multiple request tabs with stable identities, Save As, dirty indicators, close
  protection, cancellation, and recoverable local drafts.
- GET, POST, PUT, PATCH, DELETE, HEAD and OPTIONS; query parameters, headers, JSON,
  text, URL-encoded forms, multipart file uploads and binary files.
- Collection/folder, environment, iteration and local variables with strict
  unresolved-variable diagnostics and `{{vault:name}}` secret references.
- Basic, bearer and API-key auth; OAuth 2 client credentials and authorization code
  with PKCE, loopback state validation, token caching and refresh.
- TLS verification by default, per-request development override, custom CA and
  client-certificate PEM files, proxies, redirects, timeouts and response limits.
- Workspace session cookies with inspect, set and clear controls.
- Response search/copy, bounded text preview, raw binary download, total transfer
  duration, assertions, JSON extraction, and pre/post scripts.
- Collection/folder runs with CSV/JSON data, cancellation, live results and JSON/JUnit
  reports. The GUI and CLI share the same execution pipeline.
- Collection v2.1/environment JSON and cURL import. Native collection export
  redacts known sensitive fields by default; including credentials is an explicit action.
- cURL, Python requests, JavaScript Fetch and Rust reqwest code generation.

## Build and run

Install Rustup; `rust-toolchain.toml` pins Rust 1.98.0. On Linux, install the build
packages listed in `docker/build-packages.txt`.

```sh
git clone https://github.com/wulfnb/rustapi.git
cd rustapi
cargo run --locked -- --workspace /path/to/api-project
```

Use **Open / create workspace** to select a project folder in the desktop. Right-click
a collection or folder to add requests, edit variables/auth/scripts, move, rename or
export. New requests and history entries can be saved into a selected collection.
Request editing has Params, Headers, Body, Auth, Tests, Scripts and Settings tabs.
Use JSON Pointer paths such as `/user/id` for JSON assertions and extraction.
Cmd/Ctrl+O opens JSON import; Cmd/Ctrl+Shift+E exports the active request's collection
(or the first collection when no request is active).

Workspaces contain:

```text
api-project/
  rustapi.json          # workspace schema/version
  collections/          # ID-named JSON files, including nested folders
  environments/         # ID-named environment JSON
  .gitignore            # excludes local state and secrets
  .local/vault.json     # encrypted local secrets, never a plaintext key
  .local/migrations/    # backups of retired legacy environment files
  drafts.json           # recoverable local request tabs
  state.json
  history.json          # redacted history; bodies omitted
```

Collection and environment names can be changed without changing their identity.
Old native collection JSON is readable; newly written collections carry schema
version 1. Unknown future schemas and malformed files produce visible errors.
Uploads and certificate paths can be relative to the workspace root. Imported files
are copied into managed storage; editing a copy does not modify its source file.

Without `--workspace`, data uses the normal OS application data directory under
`rustapi`. `RUSTAPI_DATA_DIR` overrides that location. Local drafts can contain data
that you type into the request editor; use vault references for secrets and keep
local state out of version control. Responses are not serialized into drafts.

## Import collections and environments

Export a **collection v2.1 JSON** and its environment JSON. Use **Import** in
the desktop, or:

```sh
rustapi import collection.v2.1.json --workspace /path/to/api-project
rustapi import development.environment.json --workspace /path/to/api-project
rustapi import --curl 'curl -H "Accept: application/json" https://example.com/users' --workspace /path/to/api-project
```

Import validates the entire document before saving it. Scripts are preserved, with
compatibility warnings. Unsupported body modes stop import; unsupported script APIs
fail when executed rather than silently passing. Review local file paths and OAuth
configuration after migration.

Known omissions, such as saved examples, structured descriptions, path variables
and additional multipart files, are listed before saving. The desktop asks you to review and accept
them. CLI imports and runs exit with code **2** by default when these omissions are
found. Use `--allow-lossy-import` only after reviewing the report:

```sh
rustapi import collection.v2.1.json --workspace /path/to/api-project --import-report
rustapi import collection.v2.1.json --workspace /path/to/api-project --allow-lossy-import
```

`--import-report` emits JSON diagnostics on stdout instead of saved paths; it is not
a dry run. A supported import without known omissions is saved normally. See
[import review and its limits](docs/import-compatibility.md#known-import-omissions).

Plain-text request/folder descriptions and descriptions on headers, query parameters, form fields and environment
variables are preserved during import and save/reload. These descriptions alone do
not require omission review.

## Tests and collection runs

In **Tests**, add status/header/JSON/body/time assertions and JSON-to-variable
extractors. For example, extract `/access_token` into `token`, then use `{{token}}`
in a later request's bearer authentication. Run a saved collection from **Runner**.

A supported post-response script looks like:

```javascript
pm.test("status is 200", () => {
  pm.response.to.have.status(200);
});
pm.environment.set("token", pm.response.json().access_token);
```

Headless runs do not initialize the GUI:

```sh
rustapi run collections/COLLECTION-ID.json \
  --workspace /path/to/api-project \
  --environment environments/ENVIRONMENT-ID.json \
  --data users.csv --stop-on-failure \
  --report-json results.json --report-junit results.xml
```

Collection/environment/data paths are relative to the shell's working directory;
upload and certificate paths are relative to their workspace/collection source.
`--folder ID` selects a folder, `--iterations N` overrides the number of data rows,
and `--delay MS` adds a cancellable delay between requests. Ctrl-C cancels a run.
Exit codes: **0** pass, **1** assertion/script/transport failure or cancellation,
**2** invalid arguments/configuration/import. HTTP error statuses can be tested
explicitly with assertions. Reports omit request/response bodies and transport details.

## Secrets and OAuth

Open **Vault**, choose a password of at least 12 characters, and save named values.
Reference them as `{{vault:api_token}}`. The encrypted store uses Argon2 key derivation
and AES-256-GCM; the password/key is not saved. Losing the password means losing access
to that vault. Unlocking applies to the current application session.

For CLI automation, provide `RUSTAPI_VAULT_PASSWORD` through your CI secret mechanism.
`rustapi vault set NAME --workspace DIR` reads the secret value from stdin;
`rustapi vault delete NAME --workspace DIR` removes it. Neither takes secrets as
command-line arguments. The vault file is under `.local/`, excluded from Git.

For OAuth PKCE, select **Authorization code + PKCE** in Auth, configure your provider,
and choose **Authorize in browser**. Register an `http://127.0.0.1:PORT/callback`
redirect with the provider; configure a fixed port in the redirect field when the
provider requires one. The callback validates state and expires after two minutes.
OAuth tokens are kept in memory and refreshed when supported by the provider.
Client certificate settings expect a PEM containing both the certificate and key.

## Docker desktop

```sh
docker compose up --build -d
```

Open http://localhost:6080/vnc.html?autoconnect=true&resize=scale . The native Linux app
runs on a virtual display with noVNC. The viewer has no authentication; Compose binds
it to localhost. Use a trusted tunnel for remote access. Data persists in the
`rustapi-data` volume. Host APIs are reached using `host.docker.internal`.

## Validation

```sh
cargo fmt --all --check
cargo clippy --locked --all-targets -- -D warnings
cargo test --locked --all-targets
cargo test --locked --doc
cargo build --locked --examples
python -m pip install requests==2.32.5
python scripts/codegen-smoke.py target/debug
python scripts/desktop-smoke.py target/debug/rustapi
```

Code-generation validation also needs Node 20+, curl and bash (POSIX systems).
On headless Linux, run desktop smoke under Xvfb with `LIBGL_ALWAYS_SOFTWARE=1`.
Tests use temporary profiles and loopback servers. CI validates native platforms,
Docker, packaging and coverage; see [CI/CD](docs/ci-cd.md) and
[validation evidence](docs/validation.md).

## License

GPL-3.0-or-later. See [LICENSE](LICENSE), [NOTICE](NOTICE), and
[contribution instructions](CONTRIBUTING.md). Dependencies retain their own licenses.
