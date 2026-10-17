# Contributing to memcastle-pi

Install Rust with the `wasm32-wasip2` target and Bun (`mise install`).
Changes to Pi's JSONL reader and cursor live in `sources/pi/src/lib.rs`; changes to the live Pi extension live in
`integrations/pi/src/*.ts`.
Keep the source and integration IDs `pi`, and preserve source option identity (`dir` is part of the scoped cursor;
`since` only narrows a run).
Do not change the source contract, filesystem grants or Core version range without reviewing their compatibility.

Run these checks with `MEMCASTLE_BIN` pointing to a plugin-capable Core binary (the permanent `main` prerelease or a
locally built Core):

```sh
cargo test --manifest-path sources/pi/Cargo.toml --locked
# Build the source component for wasm32-wasip2 and copy it to sources/pi/source.wasm first.
(cd integrations/pi && bun install --frozen-lockfile && bun run build && bun test --timeout 60000)
mise run package
"$MEMCASTLE_BIN" plugin test dist/pi-0.1.0.tar.gz
"$MEMCASTLE_BIN" source test sources/pi --no-build
node scripts/smoke_adoption.mjs
```

The archive is `dist/pi-<manifest.toml version>.tar.gz`; Core regenerates reviewed module and artifact checksums in
`manifest.toml` while keeping `plugin.toml` as an immutable descriptor.
Commit `manifest.toml`, the source's `Cargo.lock`, the Bun lockfile and the change together.
Pi's own packaging job checks those hashes, locks, format, lint, native and integration tests;
Core's shared validation checks source conformance, installation and activation on every selected Core version.
CI runs once per pull request and on every push to `main`, including merges.
The matrix always tests `main` and records its exact SHA; add only plugin-capable stable versions to `core-versions`.
Pi's adoption smoke also runs on every leg.

git-cliff computes the next `manifest.toml` version and gh-ship opens a reviewed `release/next` PR.
Merging that PR publishes the validated `pi/v<version>` archive and `.sha256sum`.
Add an entry to Core's `plugins.json` only when the archive has actually been published; for a direct GitHub repository,
Core can discover the plugin's versioned release without a static entry.
