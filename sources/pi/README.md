# pi

Pi session-history source. The separate [Pi integration](../../integrations/pi/) handles agent lifecycle events.

This is a separately activatable MemCastle source module using the `memcastle:source@0.4.0` WIT world.
Build with `cargo build --release --target wasm32-wasip2` after installing the target.
Run `cargo fmt --all -- --check` and `cargo clippy --locked --target wasm32-wasip2 -- -D warnings` locally.
Commit `Cargo.lock` after the initial `cargo generate-lockfile` so CI and releases build the same dependency graph.
Run `memcastle source test sources/pi --no-build` from the plugin root to check the Pi session fixtures against the real host.
See [source behavior](../../docs/source.md) for the mined session format and cursor.
The plugin packages both implementations; installation leaves the source disabled and does not register the extension in Pi.
