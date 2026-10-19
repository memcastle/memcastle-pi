# Pi history source

The conversation history of the [Pi](https://github.com/badlogic/pi-mono) coding agent, as a MemCastle mining source:
a WebAssembly component built from `sources/pi/src/lib.rs`, one document per session file.

It is acquisition only.
MemCastle owns normalisation into drawers, chunking, deduplication, provenance, the cursor, idempotency and the durable
job, and the live Pi integration (in `integrations/pi/`) is separate: it talks to the daemon over MCP and only decides when
to ask for mining.

## Use it

```sh
memcastle source test sources/pi                   # build, then run fixtures/ in the source sandbox
archive="$(memcastle plugin package --json | jq -r .archive)" # includes the integration
memcastle plugin install "$archive" --consent pi=SHA256_FROM_PREVIEW
memcastle source enable pi
memcastle mine pi                                 # ~/.pi/agent/sessions
memcastle mine pi /backups/pi/sessions             # or: memcastle mine pi since=2026-09 dir=/work/app
```

The build needs the `wasm32-wasip2` target (`rustup target add wasm32-wasip2`).
[Mining sources](https://memcastle.github.io/mining-sources/#pi) describes what is filed and what is left out.

## One source configuration, multiple workspace miners

With a Core version containing [reusable source configurations and miners](https://github.com/memcastle/memcastle/pull/329),
enable the Pi source once, then define a shared source configuration and separate miners in the daemon's global
`~/.config/memcastle/config.toml`:

```toml
[[source_configs]]
id = "pi-history"
source = "pi"

[source_configs.options]
since = "2026-07"                 # shared source-native default; optional

[[miners]]
name = "pi-app"
source = "pi"
source_config = "pi-history"
wing = "app"
room = "sessions"
max_documents = 300

[miners.options]
dir = "/home/me/work/app"

[miners.schedule]
every = "1d"
at = "03:30"                     # UTC; disabled until explicitly enabled

[[miners]]
name = "pi-notes"
source = "pi"
source_config = "pi-history"
wing = "personal"
room = "notes"
max_documents = 100

[miners.options]
dir = "/home/me/work/notes"
```

```sh
memcastle miner run pi-app
memcastle miner run pi-notes
memcastle miner schedule-enable pi-app  # opt in to unattended runs of this miner only
```

Omit `locator` as above to use `~/.pi/agent/sessions`, or set the **same absolute sessions-folder path** on each
miner to use a backup. Pi reads local files and needs no credential. Source configuration options apply to both miners;
miner `options` override matching defaults without editing the shared configuration. Keep `dir` in each miner to select
sessions by the **session header's working directory**, not by Pi's slugged sessions subfolder. `dir` accepts a full path
or a pattern such as `/home/me/work/*` (`*` includes `/`). An unreadable header, missing or relative `cwd` cannot match
a scoped miner; an unfiltered miner can collect such sessions. Overlapping filters deliberately allow both miners to
mine a matching session into their respective destinations.

Each miner owns its destination, limit and schedule. `since` only narrows runs; normalized `dir` is part of the source's
cursor scope. Core keeps different source configurations and Wing/Room or metadata mappings in independent ingestion
identities, so a run or destination change for one miner does not advance or rewrite another miner's records. A miner's
`room` overrides the session's suggested room (normally the `cwd` basename). Core also supports Wing/Room metadata
mapping when a source supplies a suitable name; Pi's full `cwd` is a path, not a valid Wing name. Live Pi checkpoints
are separate from these history miners and follow the integration's project context instead.

## Capabilities and permissions

| Declared | Value | Why |
|---|---|---|
| `incremental` | yes | A modification-time watermark finds the sessions that grew; only their new tail is filed again. |
| `retains_raw` | yes | Pi's session files are rotated and deleted by the user, so the transcript is kept once read. |
| `needs_credentials` | no | It reads local files. |
| `filesystem.read` | `locator`, `~/.pi/agent/sessions` | The folder being mined, and Pi's own, the default. Read-only. |
| `env` | `HOME` | Only to name `~/.pi/agent/sessions` when no locator is given. |
| `network`, `process` | none | Nothing else is reachable. |

The source lists only `*.jsonl` files directly in the sessions folder or one folder down, never follows a symlink, and so
never opens Pi's credentials file (`auth.json`).
It follows Pi's session format version 3 and skips a line or an entry it does not understand instead of failing.

## Identity and provenance

| Field | Value |
|---|---|
| `external_id` | The path under the sessions folder, such as `--home-me-project--/2026-07-14T14-27-12-546Z_019f6106.jsonl`. |
| `revision` | A hash of the whole file, so it changes exactly when the session does. |
| `occurred_at` | The session header's `timestamp`, when it is RFC 3339. |
| metadata | `path`, and the header's `id` (session id), `cwd` and `version`. |
| room, name, tags | The working directory's name, the file's name, `transcript` and `pi`. |

## Tests

`sources/pi/fixtures/pi-sessions/` is the conformance case `memcastle source test` runs: three sessions in two working directories
and the usual noise.
`memcastle plugin test "$archive"` installs the archive in a disposable palace;
`node scripts/smoke_adoption.mjs` mines a fixture and checks the legacy cursor handoff through a real daemon.
`node scripts/smoke_workspace_miners.mjs` exercises shared Pi configuration, independently scoped miners and
destination-specific idempotency against a Core binary containing #329.
