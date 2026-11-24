# pi

Pi session-history source. The separate [Pi integration](../../integrations/pi/) handles agent lifecycle events.

This is a separately activatable MemCastle source module using the `memcastle:source@0.5.0` WIT world.
Requires Rust 1.100+, `wasm32-wasip3` / WASI 0.3, and upstream development Core containing `828f0f6`.
Build with `cargo build --locked --release --target wasm32-wasip3` after installing the target.
Run `cargo fmt --all -- --check` and `cargo clippy --locked --target wasm32-wasip3 -- -D warnings` locally.
From the plugin root, `mise run source:pi:build` stages the package-ready component.
Validate with `python3 scripts/validate_component.py source sources/pi/source.wasm` and wasm-tools 1.263.0.
Commit `Cargo.lock` after the initial `cargo generate-lockfile` so CI and releases build the same dependency graph.
Run `memcastle source test sources/pi --no-build` from the plugin root to check the Pi session fixtures against the real host.
See [source behavior](../../docs/source.md) for the mined session format and cursor.
The plugin packages both implementations; installation leaves the source disabled and does not register the extension in Pi.
