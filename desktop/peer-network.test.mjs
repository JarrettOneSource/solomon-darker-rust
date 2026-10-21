import assert from 'node:assert/strict'
import { randomBytes } from 'node:crypto'
import { createServer, connect } from 'node:net'
import { once } from 'node:events'
import test from 'node:test'

import { createPeerInvite, parsePeerInvite, startPeerHost, startPeerJoin } from './peer-network.mjs'

const revision = 'a'.repeat(40)

test('an invite round-trips a direct address and rejects invalid capabilities or different builds', () => {
  const secret = randomBytes(32).toString('base64url')
  const invite = createPeerInvite({ host: '192.168.1.12', port: 27843, revision, secret })
  assert.deepEqual(parsePeerInvite(invite, revision), { host: '192.168.1.12', port: 27843, revision, secret })
  assert.throws(() => parsePeerInvite(invite, 'b'.repeat(40)), /same version/)
  assert.throws(() => parsePeerInvite('not an invite', revision), /invite/i)
  for (const host of ['', 'user@host', 'host/path', 'host?query', 'host#fragment']) {
    assert.throws(() => createPeerInvite({ host, port: 27843, revision, secret }), /address/i)
  }
  for (const port of [0, 65536, 2.5, '27843']) {
    assert.throws(() => createPeerInvite({ host: 'localhost', port, revision, secret }), /port/i)
  }
  assert.throws(() => createPeerInvite({ host: 'localhost', port: 27843, revision, secret: 'password' }), /key/i)
})

test('two desktops exchange a large ordered stream through authenticated TLS and close active connections', async (t) => {
  const authority = createServer(socket => socket.pipe(socket))
  authority.listen(0, '127.0.0.1')
  await once(authority, 'listening')
  t.after(() => new Promise(resolve => authority.close(resolve)))
  const secret = randomBytes(32).toString('base64url')
  const host = await startPeerHost({ authorityPort: authority.address().port, port: 0, secret, revision })
  t.after(() => host.close())
  const invite = createPeerInvite({ host: '127.0.0.1', port: host.port, secret, revision })
  const guest = await startPeerJoin({ invite, revision })
  t.after(() => guest.close())
  const client = connect(guest.port, '127.0.0.1')
  t.after(() => client.destroy())
  await once(client, 'connect')
  const payload = randomBytes(2 * 1024 * 1024)
  const chunks = []
  let received = 0
  const echoed = new Promise(resolve => client.on('data', chunk => {
    chunks.push(chunk)
    received += chunk.length
    if (received === payload.length) resolve(Buffer.concat(chunks))
  }))
  client.write(payload)
  assert.deepEqual(await echoed, payload)
  const closed = once(client, 'close')
  await host.close()
  await closed
})

test('a wrong invite key fails before exposing a local join endpoint', async (t) => {
  const host = await startPeerHost({ authorityPort: 1, port: 0, revision, secret: randomBytes(32).toString('base64url') })
  t.after(() => host.close())
  const invite = createPeerInvite({ host: '127.0.0.1', port: host.port, revision, secret: randomBytes(32).toString('base64url') })
  await assert.rejects(startPeerJoin({ invite, revision }), /connect|invite|TLS/i)
})
