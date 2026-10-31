# Authentication and local sessions

The Auth editor supports Basic, Bearer, API key, OAuth, JWT signing, AWS SigV4 and
Digest. Collection/folder inheritance, variables and `{{vault:name}}` references
work with these helpers. An enabled explicit Authorization header takes precedence.

JWT supports HS256/384/512, RS256/384/512, PS256/384/512, ES256/384 and EdDSA using
jsonwebtoken's AWS-LC backend. HMAC keys require at least 32/48/64 bytes;
asymmetric algorithms require a matching private PEM key. Claims must be a JSON
object. Set temporal claims explicitly, using dynamic variables where appropriate.
The optional key ID is included in the protected header. Unsigned JWTs are rejected.

AWS signing uses the AWS Rust SDK signer on the final encoded URL, headers and
body. Configure access key, secret, region, service and optional session token.
S3 uses single URI encoding, preserves object-key slashes and adds the payload
checksum. Digest negotiates MD5, SHA-256 or SHA-512-256, including sess/auth-int.
Signed/replayed bodies are bounded to 64 MiB. Helper redirects stay on the same
origin, are signed again, and share one deadline with challenge negotiation and
response reading. Cross-origin redirects are returned for inspection.

JWT code samples contain a token signed at generation time. Digest/AWS dynamic
helpers are currently available only in cURL samples. AWS samples require redirects
disabled, preventing explicit session-token headers from leaking on redirects.
Other targets give a diagnostic until explicit Bearer/header auth is selected.
Native storage and portable v2.1 exports retain the supported helper settings.

Enable **Keep tokens in the encrypted local vault** in OAuth auth settings and
unlock **Local encrypted secrets** before sending. Access/refresh tokens and expiry
are encrypted in `.local/oauth-tokens.json`; the password is held only in memory.
The vault lists token metadata without values, and **Forget token** removes the
saved access/refresh pair. Locking clears unlocked tokens and cookie state.
Forgetting is local; it does not revoke the token at the provider.

CLI runs unlock saved tokens with `RUSTAPI_VAULT_PASSWORD`. Token inspection and
removal are available as `rustapi tokens list` and `rustapi tokens forget ID`, with
the usual `--workspace DIR`. Token values are not printed. Expired entries refresh
using their saved refresh token after restart. Refresh endpoint/client options are
part of the cache binding; one-time authorization code/verifier values are excluded.

Cookies are session-only until **Cookies → Keep jar in encrypted vault** is chosen.
Subsequent vault unlocks restore that workspace's encrypted `.local/cookies.json`.
The editor shows domain/path/security attributes, supports editing and deletion,
and masks cookie values. Persistence errors remain visible in the cookie window.
Session cookies in an opted-in jar survive restart. CLI runs use the same jar when
the vault password is supplied. See the [browser connector](../browser-connector/README.md)
for explicit browser capture and merge.
