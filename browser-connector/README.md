# RustAPI Cookie Connector

In Chromium's extensions manager, enable developer mode and load this directory
as an unpacked extension. In RustAPI, open **Cookies → Enable browser connection**
and copy the private pairing URL into the extension popup. Open your API's tab,
then click **Capture into RustAPI** or **Merge into browser**. Each operation asks
for permission to access that host. Pairing lives in browser session memory.

The bridge binds only to 127.0.0.1 on a random port. It checks Host, extension
Origin, and a random pairing token. It stops when disabled, the vault is locked,
the workspace changes, or the app closes. Cookie values are hidden in the app;
the bridge and extension do not log them. Local jars can be encrypted with the
workspace vault password. Cookies without an expiry are retained in persistent
jars until explicitly deleted.

Sync is an explicit merge, not continuous monitoring. Deletions are local;
partitioned cookies are skipped because RustAPI does not model browser partitions.
The connector does not read browser databases or bypass browser permission prompts.
The manifest targets Chromium MV3. Firefox distribution/signing is not included.
