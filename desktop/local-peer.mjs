import { join } from 'node:path'
import { startRuntimeService } from './runtime-service.mjs'

export async function startLocalPeer({ applicationRoot, onExit, ...configuration }) {
  const service = await startRuntimeService({
    applicationRoot, onExit, configuration,
    entry: join(applicationRoot, 'run-peer-service.mjs'),
    validate(message) {
      if (!Number.isInteger(message.port) || message.port < 1 || message.port > 65535) {
        throw new Error('Peer service emitted invalid readiness data.')
      }
      if (configuration.mode === 'join' && typeof message.credential !== 'string') {
        throw new Error('Peer service omitted its admission credential.')
      }
      return { port: message.port, credential: message.credential }
    },
  })
  return { ...service.ready, pid: service.pid, close: service.close }
}
