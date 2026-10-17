# Pi integration

The Pi extension handles wake-up, recall and checkpoints over MCP. Install it separately with
`memcastle integration install pi` after installing the containing provider plugin.
See the [integration guide](../../docs/integration.md) for configuration and behavior.

From this directory, run `bun install --frozen-lockfile`, `bun run format:check`, `bun run lint`,
`bun test --timeout 60000` and `bun run build`. The build produces `dist/extension.js` and `dist/package.json`.
The independent session-history source lives in [`sources/pi/`](../../sources/pi/).
