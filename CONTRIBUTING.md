# Contributing to memcastle-pi

Install Rust with the `wasm32-wasip2` target and Bun (`mise install`).
Changes to Pi's JSONL reader and cursor live in `sources/pi/src/lib.rs`; changes to the live Pi extension
live in `integrations/pi/src/*.ts`. Keep the source and integration IDs `pi`, and preserve source option identity:
`dir` is part of the scoped cursor, while `since` only narrows a run. Review changes to the source contract,
filesystem grants and Core version range for compatibility before changing them.

## Development checks

Point `MEMCASTLE_BIN` to a plugin-capable Core binary (the `development` prerelease or a local build).
Build the source component to `sources/pi/source.wasm` before packaging.

```sh
cargo test --manifest-path sources/pi/Cargo.toml --locked
# Build the source component for wasm32-wasip2 and copy it to sources/pi/source.wasm first.
(cd integrations/pi && bun install --frozen-lockfile && bun run build && bun test --timeout 60000)
archive="$("$MEMCASTLE_BIN" plugin package --json | jq -r .archive)"
"$MEMCASTLE_BIN" plugin test "$archive"
"$MEMCASTLE_BIN" source test sources/pi --no-build
ARCHIVE="$archive" node scripts/smoke_adoption.mjs
ARCHIVE="$archive" node scripts/smoke_workspace_miners.mjs
```

Core generates `manifest.toml` inside the SCM-versioned archive while leaving `plugin.toml` unchanged.
Commit the source `Cargo.lock`, Bun lockfile and implementation together; do not commit generated archives.
Pi CI runs format, lint, native and integration tests before packaging. Core's shared validation checks source
conformance, installation and activation against `development` and `0.5`, recording each Core source SHA.
The adoption and multi-workspace miner smokes run on every matrix leg.

## Template updates and releases

Template answers and child revisions are recorded in `.config/git.tpl.toml`. To update, run
`git tpl update --defaults --json`, review `git tpl diff --json`, then apply with `git tpl merge --json`.
Preserve Pi's module IDs, cursor behavior and provider-specific docs when resolving conflicts.
Keep the coverage-reporting module lists aligned with native tests that produce nonempty LCOV.

git-cliff computes the next tag and gh-ship opens a reviewed `release/next` PR.
Merging that PR publishes the validated `pi-<version>.tar.gz` archive and `.sha256sum`
under the `<version>` tag.
The [README](README.md) describes installation, migration and release credentials.
