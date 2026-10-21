import { createHash } from 'node:crypto'
import { spawn } from 'node:child_process'
import {
  chmod,
  copyFile,
  mkdir,
  mkdtemp,
  readFile,
  rm,
  stat,
  writeFile,
} from 'node:fs/promises'
import { tmpdir } from 'node:os'
import { join, posix, resolve } from 'node:path'
import { pathToFileURL } from 'node:url'

export const PINNED_NODE_VERSION = '22.17.0'
const NODE_DOWNLOAD_ROOT = `https://nodejs.org/dist/v${PINNED_NODE_VERSION}`

export function nodeArchiveDescriptor(platform, arch) {
  if (!['x64', 'arm64'].includes(arch)) throw new Error(`Unsupported Node architecture: ${arch}`)
  if (platform === 'linux') {
    const directory = `node-v${PINNED_NODE_VERSION}-linux-${arch}`
    return {
      archive: `${directory}.tar.xz`,
      executable: posix.join(directory, 'bin', 'node'),
      extractor: 'tar-xz',
      sha256: arch === 'x64'
        ? '325c0f1261e0c61bcae369a1274028e9cfb7ab7949c05512c5b1e630f7e80e12'
        : '140aee84be6774f5fb3f404be72adbe8420b523f824de82daeb5ab218dab7b18',
    }
  }
  if (platform === 'darwin') {
    const directory = `node-v${PINNED_NODE_VERSION}-darwin-${arch}`
    return {
      archive: `${directory}.tar.gz`,
      executable: posix.join(directory, 'bin', 'node'),
      extractor: 'tar-gz',
      sha256: arch === 'x64'
        ? 'c39c8ec3cdadedfcc75de0cb3305df95ae2aecebc5db8d68a9b67bd74616d2ad'
        : '615dda58b5fb41fad2be43940b6398ca56554cbe05800953afadc724729cb09e',
    }
  }
  if (platform === 'win32' && arch === 'x64') {
    const directory = `node-v${PINNED_NODE_VERSION}-win-x64`
    return {
      archive: `${directory}.zip`,
      executable: posix.join(directory, 'node.exe'),
      extractor: 'zip',
      sha256: '721ab118a3aac8584348b132767eadf51379e0616f0db802cc1e66d7f0d98f85',
    }
  }
  throw new Error(`Unsupported Node runtime target: ${platform}-${arch}`)
}

export async function stagePinnedNodeRuntime({
  platform,
  arch,
  destination,
  cacheRoot = resolve('.cache', 'node-runtime'),
}) {
  const descriptor = nodeArchiveDescriptor(platform, arch)
  await mkdir(cacheRoot, { recursive: true })
  const archivePath = join(cacheRoot, descriptor.archive)
  if (!await exists(archivePath) || await sha256(archivePath) !== descriptor.sha256) {
    await download(`${NODE_DOWNLOAD_ROOT}/${descriptor.archive}`, archivePath)
  }
  const actual = await sha256(archivePath)
  if (actual !== descriptor.sha256) throw new Error(`Node runtime checksum mismatch for ${descriptor.archive}`)

  const extraction = await mkdtemp(join(tmpdir(), 'solomon-node-runtime-'))
  try {
    await extractArchive(archivePath, extraction, descriptor.extractor)
    const source = join(extraction, descriptor.executable)
    await mkdir(destination, { recursive: true })
    const target = join(destination, platform === 'win32' ? 'node.exe' : 'node')
    await copyFile(source, target)
    if (platform !== 'win32') await chmod(target, 0o755)
    return { executable: target, sha256: actual, version: PINNED_NODE_VERSION }
  } finally {
    await rm(extraction, { force: true, recursive: true })
  }
}

async function download(url, destination) {
  const response = await fetch(url)
  if (!response.ok) throw new Error(`Could not download ${url}: HTTP ${response.status}`)
  await writeFile(destination, Buffer.from(await response.arrayBuffer()))
}

async function sha256(path) {
  return createHash('sha256').update(await readFile(path)).digest('hex')
}

async function extractArchive(archive, destination, extractor) {
  if (extractor === 'zip') {
    if (process.platform === 'win32') await run(join(process.env.SystemRoot, 'System32', 'tar.exe'), ['-xf', archive, '-C', destination])
    else await run('unzip', ['-q', archive, '-d', destination])
    return
  }
  await run('tar', [extractor === 'tar-xz' ? '-xJf' : '-xzf', archive, '-C', destination])
}

function run(command, arguments_) {
  return new Promise((resolveRun, reject) => {
    const child = spawn(command, arguments_, { stdio: 'inherit' })
    child.once('error', reject)
    child.once('exit', (code, signal) => {
      if (code === 0) resolveRun()
      else reject(new Error(`${command} failed (${code ?? signal ?? 'unknown'})`))
    })
  })
}

async function exists(path) {
  try {
    return (await stat(path)).isFile()
  } catch {
    return false
  }
}

function argument(name) {
  const index = process.argv.indexOf(name)
  const value = index < 0 ? undefined : process.argv[index + 1]
  if (!value || value.startsWith('--')) throw new Error(`${name} requires a value`)
  return value
}

if (process.argv[1] && import.meta.url === pathToFileURL(resolve(process.argv[1])).href) {
  const staged = await stagePinnedNodeRuntime({
    platform: argument('--platform'),
    arch: argument('--arch'),
    destination: resolve(argument('--destination')),
  })
  process.stdout.write(`${JSON.stringify(staged)}\n`)
}
