# Versioning and releases

Townhall uses [semantic versioning](https://semver.org): `MAJOR.MINOR.PATCH`, with an optional
pre-release suffix and build metadata (see [software versioning](https://en.wikipedia.org/wiki/Software_versioning)).

## What the numbers mean

While the major version is `0`, the API is not stable:

- `MINOR` is for features and for breaking changes.
- `PATCH` is for fixes only.

From `1.0.0` on, `MAJOR` is for breaking changes, `MINOR` for compatible features and `PATCH` for fixes.

## Stages

Pre-release stages are written as a suffix. The suffix sorts before the plain version, so
`1.0.0-rc.1` is older than `1.0.0`.

| Stage | Example | Meaning |
|-------|---------|---------|
| alpha | `0.2.0-alpha.1` | where we are now. Features are still added and removed |
| beta | `0.3.0-beta.1` | feature set frozen, only fixes |
| rc | `1.0.0-rc.1` | release candidate, nothing planned except blocking fixes |
| stable | `1.0.0` | the daemon API and the config are stable |

Releases with a suffix are published as GitHub pre-releases. The Homebrew cask follows them too,
because everything is alpha for now.

## Dev builds: version and sha

A build from a git checkout reports its commit as build metadata, for example
`0.2.0-alpha.1+d812d0d`. You see it in:

- `townhall-daemon --version`
- `GET /api/info`, field `version`
- the `daemon.start` and `app.start` log events and the MCP `serverInfo`

`crates/townhall-core/build.rs` reads `git rev-parse --short HEAD`. Without git, or when
`TOWNHALL_RELEASE` is set (the release workflow sets it), the sha is left out and the version is
the plain `0.2.0-alpha.1`. No local setup is needed: `cargo build` and `npm run dev` work as before.

## Where the version lives

One place: `version` under `[workspace.package]` in the root `Cargo.toml`. Every crate has
`version.workspace = true`, and `src-tauri/tauri.conf.json` has no `version`, so Tauri uses the
Cargo one. `package.json` and `web/package.json` are private and stay at `0.0.0`.

## Releasing

1. Open a PR that changes the workspace `version` in the root `Cargo.toml` (and the `townhall-*`
   entries in `Cargo.lock`, which `cargo build` updates). Merge it.
2. Write the release notes in `notes.md`.
3. Tag the merge commit and push the tag:

   ```sh
   git tag -a v0.2.0-alpha.1 -F notes.md
   git push origin v0.2.0-alpha.1
   ```

The tag must be annotated, because its message becomes the release notes.

The `release` workflow then:

1. fails unless the tag equals `v` plus the workspace version.
2. runs the tests, builds a universal (Apple Silicon and Intel) `Townhall.app` with the Tauri CLI,
   signs it ad hoc and zips it as `Townhall-macos.zip`. The daemon runs inside the app, so there
   is nothing else to ship.
3. publishes the GitHub release with the zip and `SHA256SUMS`, notes from the tag, marked
   pre-release when the version has a suffix.
4. renders `.github/homebrew/townhall.rb` into `Casks/townhall.rb` in
   [WilliamThogersen/homebrew-tap](https://github.com/WilliamThogersen/homebrew-tap) and pushes it
   with the `HOMEBREW_TAP_DEPLOY_KEY` secret (a write deploy key on the tap).

The `brew-check` workflow then installs the cask on a clean Mac and checks that the installed
version is the latest release. It also runs every Monday and on demand.

## Installing

```sh
brew install --cask WilliamThogersen/tap/townhall
```

The app is signed ad hoc, not notarized, so macOS Gatekeeper may block a downloaded copy. The cask
removes the quarantine flag on install. If macOS still refuses to open it:

```sh
xattr -dr com.apple.quarantine /Applications/Townhall.app
```

## Local dev

Nothing changes. `cargo build`, `cargo test` and `npm run dev` need no extra tools or variables.
