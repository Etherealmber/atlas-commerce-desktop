# Upstream provenance

This standalone crate is copied from `rust/atlas-auth-core` in
[Etherealmber/atlas-commerce](https://github.com/Etherealmber/atlas-commerce)
at commit `3b0e8c63e10c6efdfd2f21cecaf1a1f98a81458a` (including PR #213).
Its upstream manifest declares `MIT OR Apache-2.0`. Source has been formatted for
this workspace. Keep it and the contract vectors synchronized with the Worker
protocol; do not update it without reviewing the matching backend contract.
The local `ServerOrigin::parse` also rejects empty userinfo and backslashes in
a configured origin, and `validate_challenge_url` rejects raw backslashes.
The local state machine also exposes phase-checked cancellation proof fields.
The local dependency on `p256` was upgraded to 0.14, and point encoding was
adapted, to use a `crypto-bigint` version containing the RUSTSEC-2026-0387 fix.
These changes are not yet part of the upstream copy and require contract tests.

The crate is a protocol building block only. The desktop does not yet wire it
into login, platform key storage, WebView navigation or authenticated requests.
