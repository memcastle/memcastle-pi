// Real Pi archive bytes must keep a mined cursor and drawers across plugin lifecycle transitions.
import assert from "node:assert/strict"
import { copyFileSync, mkdirSync, readFileSync, utimesSync } from "node:fs"
import { join } from "node:path"
import { ROOT, isolated, mine, pluginArchive } from "./smoke_common.mjs"

await isolated("memcastle-pi-lifecycle-", null, async ({ root, request }) => {
  const archive = readFileSync(pluginArchive())
  const preview = await request("/api/plugins/preview", { method: "POST", data: archive })
  const consents = Object.fromEntries(preview.source_consents.map(({ source, digest }) => [source, digest]))
  const install = () => request("/api/plugins", { method: "POST", data: archive,
    headers: { "X-MemCastle-Consents": JSON.stringify(consents) } })
  await install()
  await request("/api/source-packages/pi/enable", { method: "POST" })

  const sessions = join(root, "sessions")
  const session = join(sessions, "--home-me-project--")
  mkdirSync(session, { recursive: true })
  const name = "2026-07-14T14-27-12-546Z_019f6106.jsonl"
  const target = join(session, name)
  copyFileSync(join(ROOT, "sources/pi/fixtures/pi-sessions/tree/--home-me-project--", name), target)
  utimesSync(target, 1000, 1000)
  const definition = { source: "pi", locator: sessions, wing: "work", room: "sessions" }
  await request("/api/miners/pi-history", { method: "PUT", json: definition })
  const run = async () => {
    const job = await mine(request, "pi", sessions)
    assert.equal(job.status, "completed", JSON.stringify(job))
    return job.result
  }
  assert.equal((await run()).created, 1)
  const source = () => request("/api/sources").then(({ sources }) => sources.find((item) => item.source === "pi"))
  const drawers = () => request("/api/search?q=rotation")
  const before = await source()
  assert.ok(before.cursor)
  assert.equal(before.documents, 1)
  assert.equal((await drawers()).length, 1)

  await install()
  assert.equal((await request("/api/source-packages/pi")).state, "enabled")
  assert.deepEqual([(await source()).cursor, (await source()).documents], [before.cursor, before.documents])
  assert.equal((await request("/api/miners/pi-history")).source, "pi")
  assert.equal((await run()).documents, 0, "reinstall rewound the source cursor")
  await assert.rejects(request("/api/plugins/pi", { method: "DELETE" }), { status: 409 })
  assert.equal((await request("/api/miners/pi-history")).source, "pi")

  await request("/api/miners/pi-history", { method: "DELETE" })
  await request("/api/source-packages/pi/disable", { method: "POST" })
  await request("/api/plugins/pi", { method: "DELETE" })
  assert.equal((await drawers()).length, 1, "uninstall deleted mined drawers")
  await install()
  const after = await source()
  assert.deepEqual([after.cursor, after.documents], [before.cursor, before.documents])
  assert.equal((await drawers()).length, 1)
  await request("/api/source-packages/pi/enable", { method: "POST" })
  await request("/api/miners/pi-history", { method: "PUT", json: definition })
  assert.equal((await run()).documents, 0, "reinstall after uninstall rewound the source cursor")
})
console.log("real Pi archive retained its miner boundary, cursor and mined drawers across reinstall and uninstall")
