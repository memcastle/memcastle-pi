# memcastle-pi

Pi coding-agent history and lifecycle integration.

This provider plugin owns the versioned package, while each module has its own lifecycle.
The `pi` source reads Pi's JSONL session history; the `pi` integration provides wake-up, recall and checkpoints over MCP.
They intentionally share an ID but remain separately activated, configured and versioned.
See [source behavior](docs/source.md) and [integration behavior and configuration](docs/integration.md).
Installing the plugin makes its modules available; enable sources and install integrations separately.

```sh
mise install
python3 scripts/package.py
python3 scripts/check_modules.py
python3 -m unittest discover -s tests
MEMCASTLE_BIN=/path/to/memcastle python3 scripts/smoke_install.py
MEMCASTLE_BIN=/path/to/memcastle python3 scripts/smoke_adoption.py
```

The archive in `dist/` is independently releasable from MemCastle Core.
The root `plugin.toml` declares stable typed module identities; the package script fills artifact hashes after building.
Commit each source module's generated `Cargo.lock` before publishing a release.
The source module carries the versioned public WIT contract and its own fixtures; the agent integration uses MCP/HTTP.
No module needs a private Core checkout or a sibling provider to build or test.
The smoke check starts an isolated daemon and uses a temporary agent configuration to verify the archive install path.
The adoption check separately mines a session with a legacy package, removes that package, and verifies its miner,
cursor and mined data survive the plugin handoff without re-reading the session.
Provider CI tests sources against the latest stable Core and installs the archive when that release supports plugins;
Core CI separately tests a freshly composed project against its current install contract.
The release workflow accepts only a `pi/v<plugin.toml version>` tag on `main`, then rebuilds the exact
reviewed artifact, runs manifest tests and source conformance against a stable Core release, and refuses an existing release.
The parent scaffold uses `memcastle-plugin.tpl` with the Pi source composed at the pinned child revision recorded in
`.config/git.tpl.toml`. GitTPL 0.14 rejects the same `pi` identifier in both composition lists, so the integration is
overlaid in `modules/pi/` from `memcastle-integration.tpl` at the parent template's pinned revision
`04cc8455f056485cb08298a4a0af82909d314fe9`; retain both typed module entries when updating the parent template.
The actual Pi implementation and manifest were migrated from Core over that scaffold.
Neither module requires a sibling Core checkout, and `MEMCASTLE_BIN` selects a released or local Core.

To migrate a palace already mined by the bundled `pi` source, install this archive with the preview's exact consent
and `--adopt-source pi`; remove a separately installed legacy package first, if one is present.
This transfers the existing source name without clearing its miner, cursor or drawers.
Enable the source explicitly and run `memcastle integration install pi` to register the new bundled extension.
No independent Pi history migration or changes to the agent's own settings are required.
