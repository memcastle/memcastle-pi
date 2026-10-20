# memcastle-pi

[![CI][ci-badge]][ci-workflow] [![Codecov][codecov-badge]][codecov-project]

[ci-badge]: https://github.com/memcastle/memcastle-pi/actions/workflows/ci.yaml/badge.svg?branch=main
[ci-workflow]: https://github.com/memcastle/memcastle-pi/actions/workflows/ci.yaml
[codecov-badge]: https://codecov.io/gh/memcastle/memcastle-pi/graph/badge.svg
[codecov-project]: https://codecov.io/gh/memcastle/memcastle-pi

Pi coding-agent history and lifecycle integration.

The `pi` source reads Pi's JSONL session history; the `pi` agent integration provides wake-up, recall and checkpoints
over MCP. They share an ID but remain separately activated, configured and versioned.
See [source behavior](docs/source.md) and [integration behavior and configuration](docs/integration.md).

## Install

When a release is available in the [MemCastle plugin catalogue](https://memcastle.github.io/plugins.json),
install it by ID with a plugin-capable Core binary:

```sh
memcastle plugin install pi
memcastle source enable pi
memcastle integration install pi
```

Installation makes the modules available; source activation and agent registration are separate steps.
Core retrieves and checks the published archive. If no release is available yet, build the project below and
install the resulting `dist/pi-VERSION.tar.gz` archive instead.

### Moving an existing installation

To migrate a palace already mined by the bundled `pi` source, preview installation for the exact permission
digest and pass `--adopt-source pi`. Remove a separately installed legacy package first, if present.
This transfers the existing source name without clearing its miner, cursor or drawers.
Enable the source explicitly and run `memcastle integration install pi` to register the new extension.
No independent Pi history migration or changes to the agent's own settings are required.

## Build and test locally

Run `mise install` for Rust/`wasm32-wasip2` and Bun, and use a plugin-capable MemCastle Core binary.
Build `sources/pi/source.wasm` and `integrations/pi/dist/` before packaging; the CI workflow contains the
reproducible build steps. Commit the source `Cargo.lock` and Bun lockfile.

```sh
memcastle plugin inspect --json
memcastle plugin package
memcastle plugin test dist/pi-VERSION.tar.gz
node scripts/smoke_adoption.mjs
```

Replace `VERSION` with the archive's version in `dist/`. Packaging leaves `plugin.toml` immutable and generates
`manifest.toml` with versions and checksums inside the archive, alongside an external `.sha256sum`.
The child-template revisions for `sources/pi/` and `integrations/pi/` are recorded in `.config/git.tpl.toml`;
neither module requires a sibling Core checkout. Set `MEMCASTLE_BIN` to use a released or local Core binary.

## CI and releases

CI tests both modules and the packaged artifact against Core `development` and the latest stable `0.5` patch.
Core's shared validation checks source fixtures, installation and activation, while Pi's adoption and
multi-workspace miner smokes run on every matrix leg. The Core setup action verifies its binary against
`SHA256SUMS` and reports its source SHA. Core 0.4.1 predates plugin installation and cannot join the matrix.

gh-ship and git-cliff open a reviewed `release/next` PR for releasable commits. After it merges, the release
workflow validates and publishes the `pi/vVERSION` archive. Configure `APP_CLIENT_ID` and `APP_PRIVATE_KEY` in the
`release` environment for an installed GitHub App.
The publish workflow uses this credential to check the draft release and upload its validated archive.

Native Cargo/Bun coverage and test results upload to Codecov. The fixed floors are 82% project, 80% patch,
41% for `source-pi` and 99% for `integration-pi`.

## Contributing

See [CONTRIBUTING.md](CONTRIBUTING.md) for the full project checks and [AGENTS.md](AGENTS.md) for agent guidance.
