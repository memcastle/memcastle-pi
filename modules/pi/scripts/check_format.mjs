// Require portable whitespace in the authored entry point and its lifecycle tests.
import { readFileSync } from "node:fs"
import { dirname, join } from "node:path"
import { fileURLToPath } from "node:url"

const root = dirname(dirname(fileURLToPath(import.meta.url)))
for (const file of ["src/extension.ts", "test/extension.test.ts"]) {
  const data = readFileSync(join(root, file), "utf8")
  if (data.includes("\r") || !data.endsWith("\n") || data.split("\n").some((line) => /[\t ]+$/.test(line))) {
    throw new Error(`${file}: use LF, remove trailing whitespace and add a final newline`)
  }
}
