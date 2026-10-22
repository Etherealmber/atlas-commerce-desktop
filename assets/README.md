# Release signing assets

This directory holds the public key used to verify update checksums. It is
created by the release process, not committed as a placeholder.

## `update-public-key.hex`

When update signature verification is enabled, the release workflow expects:

- the Ed25519 **public** key, hex-encoded, at `assets/update-public-key.hex`;
- the matching private key in the `ATLAS_UPDATE_SIGNING_KEY` repository
  secret;
- the same public key compiled into the binary through the
  `ATLAS_UPDATE_PUBLIC_KEY` environment variable
  (`crates/ui/src/update.rs` reads it with `option_env!`).

Without these, releases still ship a `checksums.txt` and the updater verifies
every download against the GitHub-served SHA-256 checksums; only the Ed25519
signature step is skipped.

Generate a keypair with the toolchain documented by
[fastframe-update](https://github.com/crmne/fastframe), commit the public
key only, and set the repository variable `UPDATE_SIGNING_ENABLED=true` so
the signing job in `.github/workflows/release.yml` runs.
