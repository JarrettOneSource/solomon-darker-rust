import { createHash } from 'node:crypto'
import { homedir } from 'node:os'
import { dirname, join } from 'node:path'
import { fileURLToPath } from 'node:url'
import { mkdir, readFile, writeFile } from 'node:fs/promises'

import { readZipEntries } from './zip-reader.mjs'

const DEFAULT_SOURCES = fileURLToPath(new URL('./asset-sources.json', import.meta.url))

export async function runAssetCommand(args, io = console) {
  const [command, ...ids] = args
  if (command !== 'fetch') throw new Error('usage: sdmod assets fetch [source-id ...]')
  const catalog = await readAssetSources()
  const selected = ids.length === 0
    ? catalog.sources
    : ids.map(id => {
        const source = catalog.sources.find(candidate => candidate.id === id)
        if (!source) throw new Error(`unknown asset source: ${id}`)
        return source
      })
  const fetched = []
  for (const source of selected) fetched.push(await fetchAssetSource(source))
  io.log(JSON.stringify({ cache: assetCacheRoot(), sources: fetched }, null, 2))
  return fetched
}

export async function readAssetSources(path = DEFAULT_SOURCES) {
  const value = JSON.parse(await readFile(path, 'utf8'))
  if (!value || typeof value !== 'object' || !Array.isArray(value.sources)) {
    throw new Error('asset source catalog is invalid')
  }
  return value
}

export function assetCacheRoot() {
  return process.env.SDR_MOD_ASSET_CACHE || join(homedir(), '.cache', 'solomon-dark', 'sdmod-assets')
}

export function assetSourceRoot(id) {
  return join(assetCacheRoot(), id)
}

export async function fetchAssetSource(source) {
  validateSource(source)
  const root = assetSourceRoot(source.id)
  const archivePath = join(root, source.downloadKind === 'file' ? 'source.bin' : 'source.zip')
  await mkdir(root, { recursive: true })
  let archive
  try {
    archive = await readFile(archivePath)
    if (sha256(archive) !== source.sha256) {
      throw new Error(`cached asset archive SHA-256 changed: ${source.id}`)
    }
  } catch (error) {
    if (error?.code !== 'ENOENT') throw error
    const response = await fetch(source.archiveUrl)
    if (!response.ok) throw new Error(`asset download failed (${response.status}): ${source.id}`)
    archive = Buffer.from(await response.arrayBuffer())
    const digest = sha256(archive)
    if (digest !== source.sha256) {
      throw new Error(`asset download SHA-256 mismatch for ${source.id}: ${digest}`)
    }
    await writeFile(archivePath, archive)
  }

  const paths = [...new Set([
    ...source.selectedFiles,
    ...(source.licenseFile ? [source.licenseFile] : []),
  ])]
  const entries = source.downloadKind === 'file'
    ? new Map([[source.fileName, archive]])
    : readZipEntries(archive, paths)
  for (const [path, bytes] of entries) {
    const output = join(root, path)
    await mkdir(dirname(output), { recursive: true })
    await writeFile(output, bytes)
  }
  return {
    id: source.id,
    license: source.license,
    selectedFiles: source.selectedFiles,
  }
}

function validateSource(source) {
  if (!source || typeof source !== 'object' || !/^[a-z0-9][a-z0-9.-]+$/.test(source.id) ||
      !/^https:\/\//.test(source.archiveUrl) || !/^[a-f0-9]{64}$/.test(source.sha256) ||
      !Array.isArray(source.selectedFiles) || source.selectedFiles.length === 0 ||
      (source.licenseFile !== null && typeof source.licenseFile !== 'string') ||
      (source.downloadKind === 'file' && (
        typeof source.fileName !== 'string' || source.selectedFiles.length !== 1 ||
        source.selectedFiles[0] !== source.fileName
      )) || (source.downloadKind !== undefined && source.downloadKind !== 'file')) {
    throw new Error('asset source entry is invalid')
  }
}

function sha256(bytes) {
  return createHash('sha256').update(bytes).digest('hex')
}
