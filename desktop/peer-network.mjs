import { hkdfSync } from 'node:crypto'
import { createServer as createTcpServer, connect as connectTcp, isIP } from 'node:net'
import { createServer as createTlsServer, connect as connectTls } from 'node:tls'

const TLS_OPTIONS = { minVersion: 'TLSv1.3', ciphers: 'TLS_AES_128_GCM_SHA256' }
const CONNECT_TIMEOUT_MS = 8_000

export function createPeerInvite({ host, port, revision, secret }) {
  validateCapability({ host, port, revision, secret })
  const address = isIP(host) === 6 ? `[${host}]` : host
  return `solomondarker://${address}:${port}/v1/${revision}#${secret}`
}

export function parsePeerInvite(value, currentRevision) {
  if (typeof value !== 'string' || value.length > 1024) throw new Error('Invalid multiplayer invite.')
  let url
  try { url = new URL(value.trim()) } catch { throw new Error('Invalid multiplayer invite.') }
  if (url.protocol !== 'solomondarker:' || url.username || url.password || url.search
    || !url.pathname.startsWith('/v1/')) throw new Error('Invalid multiplayer invite.')
  const invite = {
    host: url.hostname.replace(/^\[|\]$/g, ''),
    port: Number(url.port),
    revision: url.pathname.slice(4),
    secret: url.hash.slice(1),
  }
  validateCapability(invite)
  if (invite.revision !== currentRevision) {
    throw new Error('Both players need the same version of Solomon Darker. Download the host’s release or update both apps.')
  }
  return invite
}

function validateCapability({ host, port, revision, secret }) {
  if (typeof host !== 'string' || host.length > 253
    || (!isIP(host) && !/^[a-z\d](?:[a-z\d.-]*[a-z\d])?$/i.test(host))) {
    throw new Error('Enter a host IP address or hostname, without a port or URL.')
  }
  if (!Number.isInteger(port) || port < 1 || port > 65535) throw new Error('The peer port must be between 1 and 65535.')
  if (!/^(?:[a-f\d]{40}|development)$/.test(revision)) throw new Error('Invalid invite version.')
  if (typeof secret !== 'string' || !/^[\w-]{43}$/.test(secret)
    || Buffer.from(secret, 'base64url').toString('base64url') !== secret) throw new Error('Invalid invite key.')
}

function transportKey(secret) {
  return Buffer.from(hkdfSync('sha256', Buffer.from(secret, 'base64url'), '', 'solomon-darker-peer-v1', 32))
}

/** The authority stays on loopback. Only this explicitly requested TLS listener is public. */
export async function startPeerHost({ authorityPort, port = 27843, secret, revision }) {
  validateCapability({ host: 'localhost', port: port || 1, secret, revision })
  const key = transportKey(secret)
  const sockets = new Set()
  const server = createTlsServer({
    ...TLS_OPTIONS,
    handshakeTimeout: CONNECT_TIMEOUT_MS,
    pskCallback: (_socket, identity) => identity === `sdr-v1:${revision}` ? key : null,
  }, socket => {
    trackSocket(sockets, socket)
    const authority = connectTcp(authorityPort, '127.0.0.1')
    trackSocket(sockets, authority)
    pipePair(socket, authority)
  })
  server.maxConnections = 32
  server.on('connection', socket => trackSocket(sockets, socket))
  // Invalid keys and incomplete TLS handshakes belong to the connecting socket.
  server.on('tlsClientError', (_error, socket) => socket.destroy())
  await listen(server, port, '0.0.0.0')
  return listener(server, sockets)
}

/** Chromium keeps the shared WebSocket transport; Node owns TLS authentication. */
export async function startPeerJoin({ invite: value, revision }) {
  const invite = parsePeerInvite(value, revision)
  const probe = await connectPeer(invite)
  probe.end()
  const sockets = new Set()
  const server = createTcpServer(local => {
    trackSocket(sockets, local)
    local.pause()
    const peer = peerSocket(invite)
    trackSocket(sockets, peer)
    local.once('close', () => peer.destroy())
    peer.once('error', () => local.destroy())
    peer.once('close', () => local.destroy())
    peer.once('secureConnect', () => {
      peer.setTimeout(0)
      if (local.destroyed) peer.destroy()
      else pipePair(local, peer)
    })
  })
  server.maxConnections = 16
  await listen(server, 0, '127.0.0.1')
  return { ...listener(server, sockets), credential: invite.secret }
}

function peerSocket(invite) {
  const socket = connectTls({
    ...TLS_OPTIONS,
    host: invite.host,
    port: invite.port,
    pskCallback: () => ({ identity: `sdr-v1:${invite.revision}`, psk: transportKey(invite.secret) }),
    // TLS-PSK authenticates the random invite key instead of an X.509 name.
    checkServerIdentity: () => undefined,
  })
  socket.setTimeout(CONNECT_TIMEOUT_MS, () => socket.destroy(new Error('Peer connection timed out.')))
  return socket
}

function connectPeer(invite) {
  return new Promise((resolve, reject) => {
    const socket = peerSocket(invite)
    socket.once('error', cause => reject(new Error('Could not connect to the host. Check the invite, firewall and forwarded TCP port.', { cause })))
    socket.once('secureConnect', () => {
      socket.setTimeout(0)
      resolve(socket)
    })
  })
}

function pipePair(first, second) {
  first.once('error', () => second.destroy())
  second.once('error', () => first.destroy())
  first.once('close', () => second.destroy())
  second.once('close', () => first.destroy())
  first.pipe(second).pipe(first)
}

function trackSocket(sockets, socket) {
  sockets.add(socket)
  socket.once('close', () => sockets.delete(socket))
  socket.on('error', () => socket.destroy())
}

function listen(server, port, host) {
  return new Promise((resolve, reject) => {
    server.once('error', reject)
    server.listen(port, host, () => {
      server.off('error', reject)
      resolve()
    })
  })
}

function listener(server, sockets) {
  let closing
  return {
    port: server.address().port,
    close() {
      closing ??= new Promise((resolve, reject) => {
        server.close(error => error ? reject(error) : resolve())
        for (const socket of sockets) socket.destroy()
      })
      return closing
    },
  }
}
