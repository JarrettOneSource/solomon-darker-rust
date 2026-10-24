import assert from 'node:assert/strict'
import fs from 'node:fs'
import { mkdir, mkdtemp, open, rm, writeFile } from 'node:fs/promises'
import { get } from 'node:http'
import { syncBuiltinESMExports } from 'node:module'
import { tmpdir } from 'node:os'
import { join } from 'node:path'
import test from 'node:test'
import { setTimeout as delay } from 'node:timers/promises'

import { startStaticClientServer } from './static-client-server.mjs'

test('desktop static server is loopback-only, SPA-aware, and hardened', async (context) => {
  const root = await mkdtemp(join(tmpdir(), 'solomon-desktop-static-'))
  context.after(() => rm(root, { force: true, recursive: true }))
  await mkdir(join(root, 'assets'))
  await writeFile(join(root, 'index.html'), '<!doctype html><title>Solomon Dark</title>')
  await writeFile(join(root, 'assets', 'game.wasm'), new Uint8Array([0, 97, 115, 109, 1, 0, 0, 0]))
  await writeFile(join(root, 'assets', 'game.js'), 'export const ready = true')
  const server = await startStaticClientServer({ root })
  context.after(() => server.close())

  assert.match(server.origin, /^http:\/\/127\.0\.0\.1:\d+$/)
  const game = await fetch(`${server.origin}/game`)
  assert.equal(game.status, 200)
  assert.match(await game.text(), /Solomon Dark/)
  const contentSecurityPolicy = game.headers.get('content-security-policy') ?? ''
  assert.match(contentSecurityPolicy, /object-src 'none'/)
  assert.match(contentSecurityPolicy, /font-src 'self' data:/)
  assert.match(contentSecurityPolicy, /img-src 'self' data: blob:/)
  assert.match(contentSecurityPolicy, /script-src 'self' 'wasm-unsafe-eval'/)
  assert.doesNotMatch(contentSecurityPolicy, /(?:^|[\s;])'unsafe-eval'/)
  assert.equal(game.headers.get('cache-control'), 'no-store')

  const asset = await fetch(`${server.origin}/assets/game.js`)
  assert.equal(asset.status, 200)
  assert.equal(asset.headers.get('content-type'), 'text/javascript; charset=utf-8')
  assert.match(asset.headers.get('cache-control') ?? '', /immutable/)
  assert.equal(await asset.text(), 'export const ready = true')
  const wasm = await fetch(`${server.origin}/assets/game.wasm`)
  assert.equal(wasm.headers.get('content-type'), 'application/wasm')
  await WebAssembly.compileStreaming(wasm)

  const launcher = await fetch(`${server.origin}/launcher`)
  assert.equal(launcher.headers.get('content-type'), 'text/html; charset=utf-8')
  assert.equal(launcher.headers.get('cache-control'), 'no-store')
  assert.match(await launcher.text(), /Solomon Dark/)

  const missingAsset = await fetch(`${server.origin}/assets/missing.js`)
  assert.equal(missingAsset.status, 404)
  const health = await fetch(`${server.origin}/__desktop/health`)
  assert.deepEqual(await health.json(), { status: 'ok' })
  const rejectedPost = await fetch(`${server.origin}/__desktop/health`, { method: 'POST' })
  assert.equal(rejectedPost.status, 405)
})

test('desktop static server rejects non-loopback binding', async () => {
  await assert.rejects(
    startStaticClientServer({ root: '.', host: '0.0.0.0' }),
    /must bind to loopback/,
  )
})

test('desktop static server closes a backpressured file when its client disconnects', async (context) => {
  const { root, server, sources, requestAsset } = await createStreamingFixture(context)
  const assetBytes = await writeLargeAsset(root, 'large.mp3')
  const response = await requestAsset('/large.mp3')
  response.pause()
  const source = sources[0]
  assert.ok(source)
  let previousBytes = -1
  await waitFor(() => {
    const unchanged = source.bytesRead === previousBytes
    previousBytes = source.bytesRead
    return source.isPaused() && source.bytesRead > 0 && unchanged
  }, 'the file must be backpressured before disconnecting')
  assert.ok(source.bytesRead < assetBytes, 'the file must not have reached EOF')
  assert.equal(source.closed, false)

  response.destroy()
  await waitFor(() => source.closed, 'the disconnected response must close its file')
  assert.equal(source.destroyed, true)
  assert.equal(source.fd, null)
  const health = await fetch(`${server.origin}/__desktop/health`)
  assert.deepEqual(await health.json(), { status: 'ok' })
})

test('desktop shutdown closes in-flight assets even when the renderer stops reading', async (context) => {
  const { root, server, sources, requestAsset } = await createStreamingFixture(context)
  await writeLargeAsset(root, 'paused.mp3')
  const response = await requestAsset('/paused.mp3')
  response.pause()
  const closing = server.close()
  try {
    await Promise.race([
      closing,
      delay(1500).then(() => { throw new Error('shutdown is waiting for a paused asset response') }),
    ])
    await waitFor(() => sources[0].closed, 'shutdown must also close the asset file')
  } finally {
    response.destroy()
    await closing
  }
})

test('desktop static server closes files after successful streaming', async (context) => {
  const { root, server, sources } = await createStreamingFixture(context)
  const payload = Buffer.alloc(128 * 1024, 0x61)
  await writeFile(join(root, 'normal.mp3'), payload)

  const response = await fetch(`${server.origin}/normal.mp3`)
  assert.equal(response.status, 200)
  assert.equal(response.headers.get('content-type'), 'audio/mpeg')
  assert.equal(response.headers.get('content-length'), String(payload.byteLength))
  assert.equal(response.headers.get('transfer-encoding'), null)
  assert.deepEqual(Buffer.from(await response.arrayBuffer()), payload)
  assert.equal(sources.length, 1)
  await waitFor(() => sources[0].closed, 'the completed response must close its file')
  assert.equal(sources[0].fd, null)

  const head = await fetch(`${server.origin}/normal.mp3`, { method: 'HEAD' })
  assert.equal(head.status, 200)
  assert.equal(head.headers.get('content-length'), String(payload.byteLength))
  assert.equal(await head.text(), '')
  assert.equal(sources.length, 1, 'HEAD must not open a file stream')
})

test('desktop static server closes a failed source and terminates the partial response', async (context) => {
  const { root, server, sources, requestAsset } = await createStreamingFixture(context)
  const assetBytes = await writeLargeAsset(root, 'broken.mp3')
  const failure = new Error('simulated static asset read failure')
  failure.code = 'EIO'
  const errors = []
  const writeStderr = process.stderr.write.bind(process.stderr)
  context.mock.method(process.stderr, 'write', (chunk, ...args) => {
    if (String(chunk).includes(failure.message)) {
      errors.push(String(chunk))
      return true
    }
    return writeStderr(chunk, ...args)
  })

  const response = await requestAsset('/broken.mp3')
  assert.equal(response.statusCode, 200)
  assert.ok(sources[0].bytesRead < assetBytes, 'the file must not have reached EOF')
  sources[0].destroy(failure)
  response.resume()
  await waitFor(() => response.closed, 'the failed source must close its response')

  assert.equal(response.complete, false)
  await waitFor(() => sources[0].closed, 'the failed file must close')
  assert.equal(sources[0].fd, null)
  assert.deepEqual(errors, [`Desktop client response failed: ${String(failure)}\n`])
  const health = await fetch(`${server.origin}/__desktop/health`)
  assert.deepEqual(await health.json(), { status: 'ok' })
})

async function createStreamingFixture(context) {
  const root = await mkdtemp(join(tmpdir(), 'solomon-desktop-stream-'))
  const resources = []
  const sources = []
  let server
  context.after(async () => {
    for (const resource of resources) resource.destroy()
    context.mock.restoreAll()
    syncBuiltinESMExports()
    await server?.close()
    await rm(root, { force: true, recursive: true })
  })
  await writeFile(join(root, 'index.html'), '<!doctype html><title>Solomon Dark</title>')
  server = await startStaticClientServer({ root })
  const createReadStream = fs.createReadStream
  context.mock.method(fs, 'createReadStream', (...args) => {
    const source = createReadStream(...args)
    resources.push(source)
    sources.push(source)
    return source
  })
  syncBuiltinESMExports()
  return {
    root,
    server,
    sources,
    requestAsset(path) {
      return new Promise((resolveResponse, reject) => {
        const request = get(`${server.origin}${path}`, (response) => {
          resources.push(response)
          response.on('error', () => {})
          resolveResponse(response)
        })
        resources.push(request)
        request.on('error', reject)
      })
    },
  }
}

async function writeLargeAsset(root, name) {
  const bytes = 32 * 1024 * 1024
  const file = await open(join(root, name), 'w')
  try {
    await file.truncate(bytes)
  } finally {
    await file.close()
  }
  return bytes
}

async function waitFor(predicate, message) {
  for (let attempt = 0; attempt < 100; attempt += 1) {
    if (predicate()) return
    await delay(10)
  }
  assert.fail(message)
}
