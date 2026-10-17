// A mined Pi session and its cursor survive explicit plugin adoption.
import assert from "node:assert/strict"
import { copyFileSync, mkdirSync, readFileSync, utimesSync } from "node:fs"
import { join } from "node:path"
import { ROOT, cli, isolated, mine } from "./smoke_common.mjs"

await isolated("memcastle-pi-adopt-", null, async ({ root, env, request }) => {
  const plugin = readFileSync(join(ROOT, "dist/pi-0.1.0.tar.gz"))
  cli(["source", "package", join(ROOT, "modules/pi"), "--no-build"], env)
  const legacy = readFileSync(join(ROOT, "modules/pi/dist/pi-0.1.0.tar.gz"))
  let digest
  try { await request("/api/source-packages", { method: "POST", data: legacy }) }
  catch (error) {
    assert.equal(error.body.code, "memcastle::source::consent_required")
    digest = error.body.help.split("--consent ")[1].split("`")[0]
  }
  assert.ok(digest, "legacy install must request source consent")
  const installed = await request(`/api/source-packages?consent=${encodeURIComponent(digest)}&enable=true`, {
    method: "POST", data: legacy,
  })
  assert.equal(installed.source.name, "pi")
  const sessions = join(root, "sessions")
  const session = join(sessions, "--home-me-project--")
  mkdirSync(session, { recursive: true })
  const name = "2026-07-14T14-27-12-546Z_019f6106.jsonl"
  const destination = join(session, name)
  copyFileSync(join(ROOT, "modules/pi/fixtures/pi-sessions/tree/--home-me-project--", name), destination)
  utimesSync(destination, 1000, 1000)
  await request("/api/miners/pi", { method: "PUT", json: { source: "pi", locator: sessions } })
  const run = async () => {
    const job = await mine(request, "pi", sessions)
    assert.equal(job.status, "completed", JSON.stringify(job))
    return job.result
  }
  assert.equal((await run()).created, 1)
  const before = (await request("/api/sources")).sources.find((item) => item.source === "pi")
  await request("/api/source-packages/pi", { method: "DELETE" })
  const preview = await request("/api/plugins/preview", { method: "POST", data: plugin })
  assert.deepEqual(preview.adoptions, ["pi"])
  const consents = Object.fromEntries(preview.source_consents.map(({ source, digest }) => [source, digest]))
  await assert.rejects(request("/api/plugins", { method: "POST", data: plugin,
    headers: { "X-MemCastle-Consents": JSON.stringify(consents) } }), { status: 409 })
  await request("/api/plugins", { method: "POST", data: plugin, headers: {
    "X-MemCastle-Consents": JSON.stringify(consents), "X-MemCastle-Adopt-Sources": '["pi"]',
  } })
  const after = (await request("/api/sources")).sources.find((item) => item.source === "pi")
  assert.deepEqual([after.cursor, after.documents], [before.cursor, before.documents])
  assert.equal((await request("/api/miners/pi")).source, "pi")
  assert.equal((await request("/api/source-packages/pi")).state, "installed")
  await request("/api/source-packages/pi/enable", { method: "POST" })
  assert.equal((await run()).documents, 0, "adoption rewound the Pi cursor")
})
console.log("legacy Pi miner, cursor and mined data survived explicit plugin adoption")
