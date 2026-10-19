// One Pi source configuration feeds independently scoped and filed workspace miners.
import assert from "node:assert/strict"
import { copyFileSync, mkdirSync, readFileSync, utimesSync, writeFileSync } from "node:fs"
import { join } from "node:path"
import { ROOT, isolated, pluginArchive } from "./smoke_common.mjs"

await isolated("memcastle-pi-workspaces-", null, async ({ root, request }) => {
  const plugin = readFileSync(pluginArchive())
  const preview = await request("/api/plugins/preview", { method: "POST", data: plugin })
  const consents = Object.fromEntries(preview.source_consents.map(({ source, digest }) => [source, digest]))
  await request("/api/plugins", { method: "POST", data: plugin,
    headers: { "X-MemCastle-Consents": JSON.stringify(consents) } })
  await request("/api/source-packages/pi/enable", { method: "POST" })

  const sessions = join(root, "sessions")
  const fixture = join(ROOT, "sources/pi/fixtures/pi-sessions/tree")
  const put = (folder, name, day) => {
    const dir = join(sessions, folder)
    mkdirSync(dir, { recursive: true })
    const target = join(dir, name)
    copyFileSync(join(fixture, folder, name), target)
    const time = new Date(`2026-07-${day}T00:00:00Z`)
    utimesSync(target, time, time)
  }
  put("--home-me-project--", "2026-07-14T14-27-12-546Z_019f6106.jsonl", "20")
  put("--home-me-project--", "2026-07-15T09-00-00-000Z_019f7000.jsonl", "21")
  put("--home-me-notes--", "2026-07-16T10-00-00-000Z_019f8000.jsonl", "22")
  // A slug alone cannot establish the workspace of this otherwise valid transcript.
  writeFileSync(join(sessions, "--home-me-project--", "unknown.jsonl"),
    '{"type":"session","id":"unknown"}\n{"type":"message","message":{"role":"user","content":"Unassigned"}}\n')

  const config = await request("/api/source-configs/pi-history", { method: "PUT", json: {
    source: "pi", options: { since: "2026-07" },
  } })
  assert.equal(config.source, "pi")
  const define = async (name, dir, wing, room, extra = {}) => {
    const change = await request(`/api/miners/${name}`, { method: "PUT", json: {
      source: "pi", source_config: "pi-history", locator: sessions, wing, room,
      options: { dir }, ...extra,
    } })
    assert.equal(change.miner.state, "ready")
    return change.miner
  }
  await define("pi-project", "/home/me/project", "work", "project-sessions", {
    max_documents: 1, schedule: { every: "1d", enabled: false },
  })
  await define("pi-notes", "/home/me/notes", "personal", "notes-sessions")
  const run = async (name) => {
    const queued = await request(`/api/miners/${name}/run`, { method: "POST" })
    for (let attempt = 0; attempt < 300; attempt++) {
      const job = await request(`/api/jobs/${queued.id}`)
      if (["completed", "failed", "cancelled"].includes(job.status)) {
        assert.equal(job.status, "completed", JSON.stringify(job))
        return job.result
      }
      await new Promise((done) => setTimeout(done, 100))
    }
    throw new Error(`${name} did not finish within 30 seconds`)
  }
  const drawers = (wing, room) => request(`/api/wings/${wing}/rooms/${room}/drawers`)

  assert.equal((await run("pi-project")).created, 1)
  assert.equal((await run("pi-notes")).created, 1)
  assert.equal((await run("pi-project")).created, 1, "project cursor must advance past only its first session")
  assert.equal((await run("pi-project")).documents, 0)
  assert.equal((await run("pi-notes")).documents, 0)
  const original = await drawers("work", "project-sessions")
  const notes = await drawers("personal", "notes-sessions")
  assert.equal(original.length, 2)
  assert.equal(notes.length, 1)
  assert.equal((await request("/api/miners/pi-notes")).options.dir, "/home/me/notes")

  // Overlapping selection is deliberate: both miners may file a session into their own destination.
  await define("pi-workspaces", "/home/me/*", "shared", "workspace-sessions")
  assert.equal((await run("pi-workspaces")).created, 3)
  assert.equal((await drawers("shared", "workspace-sessions")).length, 3)
  assert.equal((await run("pi-workspaces")).documents, 0)
  assert.equal((await run("pi-notes")).documents, 0)

  // Changing one miner's destination gives it new ingestion identity without mutating any other definition.
  const change = await request("/api/miners/pi-project", { method: "PUT", json: { room: "project-archive" } })
  assert.equal(change.miner.room, "project-archive")
  assert.equal((await request("/api/miners/pi-notes")).room, "notes-sessions")
  assert.equal((await request("/api/miners/pi-notes")).options.dir, "/home/me/notes")
  assert.equal((await request("/api/source-configs/pi-history")).options.since, "2026-07")
  assert.equal((await run("pi-project")).created, 1)
  assert.equal((await run("pi-project")).created, 1)
  assert.equal((await run("pi-project")).documents, 0)
  assert.equal((await drawers("work", "project-archive")).length, 2)
  assert.equal((await drawers("work", "project-sessions")).length, 2)
  assert.equal((await drawers("personal", "notes-sessions")).length, 1)
  assert.equal((await run("pi-notes")).documents, 0)
})
console.log("shared Pi configuration, independent workspace miners and destination-scoped ingestion verified")
