# pi

Pi session-history source and Pi agent lifecycle extension.

This is a separately activatable MemCastle source module using the `memcastle:source@0.4.0` WIT world.
Build with `cargo build --release --target wasm32-wasip2` after installing the target.
Run `cargo fmt --all -- --check` and `cargo clippy --locked --target wasm32-wasip2 -- -D warnings` locally.
Commit `Cargo.lock` after the initial `cargo generate-lockfile` so CI and releases build the same dependency graph.
Run `memcastle source test .` to check the Pi session fixtures against the real host.
Run `MEMCASTLE_BIN=/path/to/memcastle bun test` to exercise the extension against a disposable daemon.
The package builds both implementations; installation leaves the source disabled and does not register the extension in Pi.
