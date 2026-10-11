# pi

Pi coding-agent history and lifecycle integration.

This provider plugin owns the versioned package, while each module has its own lifecycle.
Installing the plugin makes its modules available; enable sources and install integrations separately.

```sh
python3 scripts/package.py
python3 scripts/check_modules.py
python3 -m unittest discover -s tests
MEMCASTLE_BIN=/path/to/memcastle python3 scripts/smoke_install.py
```

The archive in `dist/` is independently releasable from MemCastle Core.
The root `plugin.toml` declares stable module identities; the package script fills artifact hashes after building.
Commit each source module's generated `Cargo.lock` before publishing a release.
The source module carries the versioned public WIT contract and its own fixtures; the agent integration uses MCP/HTTP.
No module needs a private Core checkout or a sibling provider to build or test.
The smoke check starts an isolated daemon and uses a temporary agent configuration to verify the archive install path.
Provider CI tests sources against the latest stable Core and installs the archive when that release supports plugins;
Core CI separately tests a freshly composed project against its current install contract.
The release workflow accepts only a `pi/v<plugin.toml version>` tag on `main`, then rebuilds the exact
reviewed artifact, runs manifest tests and source conformance against a stable Core release, and refuses an existing release.
Keep IDs stable when moving this project from `plugins/pi/` to its own repository.
