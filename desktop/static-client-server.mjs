import { createReadStream } from 'node:fs'
import { stat } from 'node:fs/promises'
import { createServer } from 'node:http'
import { extname, resolve, sep } from 'node:path'
import { pipeline } from 'node:stream'

const CONTENT_TYPES = new Map([
  ['.css', 'text/css; charset=utf-8'],
  ['.html', 'text/html; charset=utf-8'],
  ['.ico', 'image/x-icon'],
  ['.jpeg', 'image/jpeg'],
  ['.jpg', 'image/jpeg'],
  ['.js', 'text/javascript; charset=utf-8'],
  ['.mjs', 'text/javascript; charset=utf-8'],
  ['.json', 'application/json; charset=utf-8'],
  ['.mp3', 'audio/mpeg'],
  ['.ogg', 'audio/ogg'],
  ['.wav', 'audio/wav'],
  ['.wasm', 'application/wasm'],
  ['.png', 'image/png'],
  ['.svg', 'image/svg+xml'],
  ['.webp', 'image/webp'],
  ['.woff', 'font/woff'],
  ['.woff2', 'font/woff2'],
])

export async function startStaticClientServer({ root, host = '127.0.0.1', port = 0 }) {
  if (host !== '127.0.0.1' && host !== '::1' && host !== 'localhost') {
    throw new Error('Desktop client server must bind to loopback')
  }
  const clientRoot = resolve(root)
  const indexPath = resolve(clientRoot, 'index.html')
  await requireFile(indexPath)
  let expectedHost = ''
  const server = createServer(async (request, response) => {
    try {
      if (expectedHost && request.headers.host !== expectedHost) {
        response.writeHead(421, securityHeaders())
        response.end()
        return
      }
      if (request.method !== 'GET' && request.method !== 'HEAD') {
        response.writeHead(405, { ...securityHeaders(), allow: 'GET, HEAD' })
        response.end()
        return
      }
      if (request.url === '/__desktop/health') {
        response.writeHead(200, {
          ...securityHeaders(),
          'cache-control': 'no-store',
          'content-type': 'application/json; charset=utf-8',
        })
        if (request.method === 'HEAD') response.end()
        else response.end(JSON.stringify({ status: 'ok' }))
        return
      }
      const path = safePathname(request.url)
      if (path === null) {
        response.writeHead(400, securityHeaders())
        response.end()
        return
      }
      const requested = resolve(clientRoot, `.${path}`)
      if (requested !== clientRoot && !requested.startsWith(`${clientRoot}${sep}`)) {
        response.writeHead(404, securityHeaders())
        response.end()
        return
      }
      const requestedInfo = await regularFileInfo(requested)
      if (!requestedInfo && extname(path)) {
        response.writeHead(404, securityHeaders())
        response.end()
        return
      }
      const file = requestedInfo ? requested : indexPath
      const fileInfo = requestedInfo ?? await stat(indexPath)
      const extension = extname(file).toLowerCase()
      response.writeHead(200, {
        ...securityHeaders(),
        'cache-control': file === indexPath || path.startsWith('/__desktop/') ? 'no-store' : 'public, max-age=31536000, immutable',
        'content-type': CONTENT_TYPES.get(extension) ?? 'application/octet-stream',
        'content-length': fileInfo.size,
      })
      if (request.method === 'HEAD') response.end()
      else pipeline(createReadStream(file), response, (error) => {
        if (error && error.code !== 'ERR_STREAM_PREMATURE_CLOSE') {
          process.stderr.write(`Desktop client response failed: ${String(error)}\n`)
        }
      })
    } catch (error) {
      response.writeHead(500, securityHeaders())
      response.end()
      process.stderr.write(`Desktop client response failed: ${String(error)}\n`)
    }
  })
  await new Promise((resolveListen, reject) => {
    server.once('error', reject)
    server.listen(port, host, () => {
      server.off('error', reject)
      resolveListen()
    })
  })
  const address = server.address()
  if (!address || typeof address === 'string') throw new Error('Desktop client server did not bind TCP')
  expectedHost = `${host.includes(':') ? `[${host}]` : host}:${address.port}`
  let closing
  return {
    origin: `http://${expectedHost}`,
    async close() {
      closing ??= new Promise((resolveClose, reject) => {
        server.close((error) => error ? reject(error) : resolveClose())
        server.closeAllConnections()
      })
      await closing
    },
  }
}

function securityHeaders() {
  return {
    'content-security-policy': "default-src 'self'; script-src 'self' 'wasm-unsafe-eval'; style-src 'self' 'unsafe-inline'; img-src 'self' data: blob:; font-src 'self' data:; media-src 'self'; connect-src 'self' ws://127.0.0.1:* wss:; object-src 'none'; base-uri 'none'; frame-ancestors 'none'",
    'cross-origin-opener-policy': 'same-origin',
    'referrer-policy': 'no-referrer',
    'x-content-type-options': 'nosniff',
    'x-frame-options': 'DENY',
  }
}

function safePathname(rawUrl = '/') {
  try {
    const rawPath = rawUrl.split('?', 1)[0]
    if (/%2f|%5c/i.test(rawPath)) return null
    const decoded = decodeURIComponent(rawPath)
    if (decoded.includes('\\') || decoded.split('/').includes('..')) return null
    return new URL(decoded, 'http://desktop.invalid').pathname
  } catch {
    return null
  }
}

async function regularFileInfo(path) {
  try {
    const info = await stat(path)
    return info.isFile() ? info : null
  } catch {
    return null
  }
}

async function requireFile(path) {
  if (!await regularFileInfo(path)) throw new Error(`Desktop client is missing ${path}`)
}
