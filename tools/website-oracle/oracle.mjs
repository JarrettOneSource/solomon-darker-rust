// Shared ground for the oracle scripts: where the Website checkout is, how to import one of
// its kernels unmodified, and how a fixture is written.
//
// Node strips the Website's types at import time (`--experimental-strip-types`), so a kernel
// runs exactly as the Website runs it. Only kernels whose imports are pure TypeScript load
// this way; a module that imports an image or a Pixi class is not an oracle.
import { mkdirSync, readFileSync, writeFileSync } from 'node:fs'
import { createHash } from 'node:crypto'
import { basename, dirname, resolve } from 'node:path'
import { fileURLToPath, pathToFileURL } from 'node:url'
import { deflateSync, inflateSync } from 'node:zlib'

const here = dirname(fileURLToPath(import.meta.url))
export const root = resolve(here, '..', '..')
export const websiteDir = process.env.SOLOMON_WEBSITE_DIR ?? resolve(root, '..', 'Website')
export const gameDir = resolve(websiteDir, 'frontend', 'src', 'game')
export const fixturesDir = resolve(root, 'crates', 'sim', 'tests', 'fixtures', 'website')

/** Imports a Website kernel by its path under `frontend/src/game`. */
export const loadKernel = (name) => import(pathToFileURL(resolve(gameDir, name)).href)

/** Reads an owned fixture, including the complete AI recording's lossless container. */
export function readFixture(name) {
  const stored = readFileSync(resolve(fixturesDir, name))
  const bytes = name.endsWith('.zlib') ? inflateSync(stored) : stored
  return JSON.parse(bytes.toString('utf8'))
}

/** Writes one fixture, provenance first, so a reader sees what produced it before the data. */
export function writeFixture(name, generator, source, value) {
  mkdirSync(fixturesDir, { recursive: true })
  const bytes = Buffer.from(`${JSON.stringify({ source, generator, ...value })}\n`)
  if (name.endsWith('.zlib')) {
    writeFileSync(resolve(fixturesDir, name), deflateSync(bytes, { level: 9 }))
    // This receipt identifies the JSON bytes consumed by the replay, independently
    // of zlib's compressed representation or the generator's current runtime.
    const jsonName = name.slice(0, -'.zlib'.length)
    const sha256 = createHash('sha256').update(bytes).digest('hex')
    writeFileSync(resolve(fixturesDir, `${jsonName}.sha256`), `${sha256}  ${basename(jsonName)}\n`)
  } else {
    writeFileSync(resolve(fixturesDir, name), bytes)
  }
  console.log(`wrote ${name}`)
}

export const point = ({ x, y }) => [x, y]
