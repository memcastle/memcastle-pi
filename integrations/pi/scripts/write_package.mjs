// Preserve Pi's extension metadata alongside its bundled entry point.
import { readFileSync, writeFileSync } from "node:fs"
import { dirname, join } from "node:path"
import { fileURLToPath } from "node:url"
import { parse } from "smol-toml"

const root = dirname(dirname(fileURLToPath(import.meta.url)))
const { integration } = parse(readFileSync(join(root, "memcastle-integration.toml"), "utf8"))
writeFileSync(join(root, "dist/package.json"), `${JSON.stringify({
  name: "memcastle-pi",
  version: integration.version,
  private: true,
  type: "module",
  license: "MIT",
  pi: { extensions: ["./extension.js"] },
}, null, 2)}\n`)
