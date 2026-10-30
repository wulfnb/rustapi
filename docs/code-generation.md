# Request code generation

Open the request's code panel, choose a target and copy the source into a local file.
Inherited authentication and scoped variables resolve before generation. Unresolved
variables, unsupported options and invalid headers produce a diagnostic instead of
an executable sample. Templates escape data as language literals, without evaluating
interpolation or expanding template markers inside request values.

| Target | Runtime / artifact | Request support and limits |
| --- | --- | --- |
| cURL | curl | Existing shell export with upload files. |
| Python | requests | Existing export; repeated header fields are rejected. |
| JavaScript | Fetch | Existing export; repeated fields and transport settings unavailable to Fetch are rejected. |
| Rust | reqwest | Existing standalone Rust export. |
| Raw HTTP | `.http`, HTTP/1.1 message | Text, JSON, URL-encoded and text-only multipart bodies; byte-counted Content-Length and CRLF framing. External upload files and chunked framing are rejected. TLS, proxies and timeouts belong to the sending tool. |
| Go | Standard library | Repeated headers, binary/multiple uploads, part MIME types, CA bundles, PEM identity, explicit TLS override and HTTP(S) proxy. |
| C# | .NET 8+ console project | Binary/multiple uploads, part MIME types, CA bundles, PEM identity, explicit TLS override and HTTP(S) proxy. Repeated header fields are rejected because the SDK combines them. |
| Java | Java 17+ standard library | Repeated headers, binary/multiple uploads, part MIME types, custom CA and HTTP proxy. Insecure TLS override, direct PEM identity, Host override, proxy credentials and HTTPS proxy transport are rejected. |
| Dart | Dart 3.11+ `dart:io` | Repeated headers, binary/multiple uploads, part MIME types, custom CA, PEM identity, explicit TLS override and HTTP proxy. HTTPS proxy transport is rejected. |

The additional SDK targets manage transport framing themselves and reject explicit
Content-Length, Transfer-Encoding, Trailer, Connection, Expect and Upgrade headers.
C#, Java and Dart require ASCII top-level header values; Go and raw HTTP preserve
opaque UTF-8 bytes. Multipart part values and files retain their original bytes.

Generated Go/C#/Java/Dart requests enforce response-size limits and one network
deadline across headers, redirects and body reads. Redirects retain credentials for
the same origin and remove explicit Authorization, Cookie and Proxy-Authorization
on origin changes. Client certificate and CA paths refer to local files at execution
time. Custom CA handling validates both trust and the destination hostname; C# also
requires the server-authentication certificate usage.

Samples are standalone requests. Workspace cookie state, vault contents and OAuth
browser/token acquisition are not embedded. Acquire a token and select explicit
Bearer authentication first. Go/C#/Java start with an empty cookie jar when cookies
are enabled; Dart does not replay response cookies. Uploads are buffered in memory
before the network deadline starts. Large uploads may need a streaming application.

## Executable validation

Build examples with `cargo build --locked --examples`. The existing
`scripts/codegen-smoke.py` executes 24 samples across the first four targets.
`python tests/codegen_targets.py target/debug` compiles and executes the added SDK
sources and sends raw HTTP messages to temporary loopback servers. It checks bodies,
multipart files/MIME, header repetition, custom CA, wrong-host rejection, mutual TLS,
HTTP proxy, body limits/timeouts and same/cross-origin redirects. Unsupported options
are checked as diagnostics. Nothing is installed into a system trust store.

The additional driver needs Go, .NET 8 SDK, Java 17+, Dart 3.11+ and OpenSSL. Optional
`RUSTAPI_GO`, `RUSTAPI_DOTNET`, `RUSTAPI_JAVA`, `RUSTAPI_JAVAC`, `RUSTAPI_DART` select
executables; `RUSTAPI_CODEGEN_CACHE` keeps isolated SDK caches. Source/projects and
generated client certificates are temporary. CI runs this driver on Linux; local
validation evidence is recorded in [implementation progress](implementation-progress.md).

The C# hostname check uses the platform-neutral
[MatchesHostname API](https://learn.microsoft.com/en-us/dotnet/api/system.security.cryptography.x509certificates.x509certificate2.matcheshostname?view=net-8.0)
alongside X509Chain trust validation, including exact IP SAN matching.
