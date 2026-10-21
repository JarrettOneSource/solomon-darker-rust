import { join, resolve } from 'node:path'
import { startRuntimeService } from './runtime-service.mjs'

// Source desktop/local-game-host.mjs: only the authoritative child changes.
// Credentials remain environment-only and no server-owned save file is introduced.
export async function startLocalGameHost({ applicationRoot, origin, credential, revision, onExit }) {
  const executable = resolve(process.env.SDR_DESKTOP_GAME_HOST || join(applicationRoot, 'authority', process.platform === 'win32' ? 'solomon-darker.exe' : 'solomon-darker'))
  const assets = resolve(process.env.SDR_DESKTOP_CLIENT_ROOT || join(applicationRoot, 'client'))
  const service = await startRuntimeService({
    applicationRoot, executable, arguments: ['--desktop', '--assets', assets], onExit,
    environment: {
      SDR_GAME_ALLOWED_ORIGINS: origin,
      SDR_GAME_BOOTSTRAP_CREDENTIAL: credential,
      SDR_GAME_HOST: '127.0.0.1',
      SDR_GAME_PORT: '0',
      SDR_GAME_REVISION: revision,
    },
    validate(message) {
      const url = typeof message.url === 'string' ? new URL(message.url) : null
      if (url?.protocol !== 'ws:' || url.hostname !== '127.0.0.1' || url.pathname !== '/game' || url.search || url.hash || url.username || url.password || !url.port) {
        throw new Error('Local game host emitted invalid readiness data')
      }
      return { kind: 'localhost', sessionKind: 'standalone', url: message.url, credential }
    },
  })
  return { endpoint: service.ready, pid: service.pid, close: service.close }
}
