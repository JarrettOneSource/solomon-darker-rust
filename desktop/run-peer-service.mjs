import { startPeerHost, startPeerJoin } from './peer-network.mjs'

// Electron uses BoringSSL; the bundled Node runtime owns the verified TLS-PSK rail.
let peer
let stopping = false
async function stop() {
  stopping = true
  await peer?.close()
  if (process.connected) process.disconnect()
}
process.once('disconnect', () => { void stop() })
process.once('SIGINT', () => { void stop() })
process.once('SIGTERM', () => { void stop() })

try {
  const configuration = await new Promise(resolve => process.once('message', resolve))
  peer = configuration.mode === 'host'
    ? await startPeerHost(configuration)
    : await startPeerJoin(configuration)
  if (stopping) await stop()
  else process.stdout.write(`${JSON.stringify({ type: 'ready', port: peer.port, credential: peer.credential })}\n`)
} catch (error) {
  const message = error.code === 'EADDRINUSE'
    ? 'That TCP port is already in use. Choose another port.'
    : error.message
  process.stdout.write(`${JSON.stringify({ type: 'error', message })}\n`)
  await stop()
  process.exitCode = 1
}
