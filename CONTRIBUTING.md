# Contributing to memcastle-pi

Install Rust with the `wasm32-wasip2` target, Python 3.11+, npm and Bun (`mise install`).
Changes to Pi's JSONL reader and cursor live in `modules/pi/src/lib.rs`; changes to the live Pi extension live in
`modules/pi/src/*.ts`.
Keep the source and integration IDs `pi`, and preserve source option identity (`dir` is part of the scoped cursor;
`since` only narrows a run).
Do not change the source contract, filesystem grants or Core version range without reviewing their compatibility.

Run these checks with `MEMCASTLE_BIN` pointing to a plugin-capable Core binary (the permanent `main` prerelease or a
locally built Core):

```sh
mise run package
mise run test
python3 -m unittest discover -s tests
"$MEMCASTLE_BIN" source test modules/pi --no-build
python3 scripts/smoke_install.py
python3 scripts/smoke_adoption.py
```

The archive is `dist/pi-<plugin.version>.tar.gz`; `scripts/package.py` updates its two reviewed module hashes in
`plugin.toml`.
Commit `plugin.toml`, the source's `Cargo.lock`, the Bun lockfile and the change together.
The shared Core workflows check those hashes, locks, format, lint, native and integration tests, plus source
conformance, installation and activation on every explicitly selected Core version.
The matrix always tests `main` and records its exact SHA; add only plugin-capable stable versions to `core-versions`.
Pi's adoption smoke also runs on every leg.

For an independent release, advance `plugin.version` and whichever module versions changed, repackage and review the
archive and compatibility, then follow the repository's `pi/v<version>` release workflow on a reviewed `main` commit.
The workflow checks that the tag matches the manifest, rebuilds the same bytes, and refuses an existing release.
Add an entry to Core's `plugins.json` only when the archive has actually been published; for a direct GitHub repository,
Core can discover the plugin's versioned release without a static entry.
