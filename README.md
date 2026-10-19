# memcastle-pi

Pi coding-agent history and lifecycle integration.

This provider plugin owns the versioned package, while each module has its own lifecycle.
The `pi` source reads Pi's JSONL session history; the `pi` integration provides wake-up, recall and checkpoints over MCP.
They intentionally share an ID but remain separately activated, configured and versioned.
See [source behavior](docs/source.md) and [integration behavior and configuration](docs/integration.md).
Installing the plugin makes its modules available; enable sources and install integrations separately.
Sources live in `sources/<id>/` and integrations in `integrations/<id>/`; typed modules may share an ID without
sharing their implementation files or activation state.

```sh
mise install
memcastle plugin inspect --json
# Build sources/pi for wasm32-wasip2 and run bun run build in integrations/pi first.
memcastle plugin package
memcastle plugin test dist/pi-VERSION.tar.gz
node scripts/smoke_adoption.mjs
```

The archive in `dist/` is independently releasable from MemCastle Core.
The root `plugin.toml` is immutable; Core generates `manifest.toml` only inside the SCM-versioned archive, with
release versions and artifact checksums.
Commit each source module's generated `Cargo.lock` before publishing a release.
The source module carries the versioned public WIT contract and its own fixtures; the agent integration uses MCP/HTTP.
No module needs a private Core checkout or a sibling provider to build or test.
The smoke check starts an isolated daemon and uses a temporary agent configuration to verify the archive install path.
The adoption check separately mines a session with a legacy package, removes that package, and verifies its miner,
cursor and mined data survive the plugin handoff without re-reading the session.
Provider CI builds the archive locally, then calls Core's shared artifact-validation workflow to test the packaged
source and install/activate the plugin against its
permanent `development` prerelease, recording the exact Core SHA in the job summary.
`core_versions` can add plugin-capable stable release trees to the same static validation matrix;
Core 0.4.1 is not eligible because it predates plugin installation.
Pi's adoption smoke runs on every matrix leg, and Core CI separately checks the generated plugin contract.
Pi's release workflow validates the reviewed `pi/vVERSION` tag on `main`, then rebuilds and checks
the artifact before attaching it to gh-ship's draft release. gh-ship and git-cliff open a reviewed `release/next`
PR for releasable commits. Configure `APP_CLIENT_ID` and `APP_PRIVATE_KEY` in the `release` environment for an
installed GitHub App. Native Cargo/Bun coverage and test results upload to Codecov.
Codecov uses fixed 82% project, 80% patch, 41% `source-pi` and 99% `integration-pi` component floors.
The parent `memcastle-plugin.tpl` scaffold composes the Pi source in `sources/pi/` and the Pi integration in
`integrations/pi/` from the child revisions recorded in `.config/git.tpl.toml`. Their common `pi` ID does not share
an implementation or activation state. The Pi source and integration implementations were migrated from Core.
Neither module requires a sibling Core checkout, and `MEMCASTLE_BIN` selects a released or local Core.

To migrate a palace already mined by the bundled `pi` source, install this archive with the preview's exact consent
and `--adopt-source pi`; remove a separately installed legacy package first, if one is present.
This transfers the existing source name without clearing its miner, cursor or drawers.
Enable the source explicitly and run `memcastle integration install pi` to register the new bundled extension.
No independent Pi history migration or changes to the agent's own settings are required.
