// The actual Rust compiler/test owner. This adapter never builds or falls back to TS.
import { spawn } from 'node:child_process'

async function invoke(args) {
  const executable = process.env.SOLOMON_SDMOD_BINARY || 'solomon-sdmod'
  return new Promise((resolve, reject) => {
    const child = spawn(executable, args, { stdio: ['ignore', 'pipe', 'pipe'] })
    const stdout = []
    const stderr = []
    child.stdout.on('data', chunk => stdout.push(chunk))
    child.stderr.on('data', chunk => stderr.push(chunk))
    child.on('error', reject)
    child.on('close', code => {
      const error = Buffer.concat(stderr).toString('utf8').trim()
      if (code !== 0) {
        reject(new Error(error || `solomon-sdmod exited with status ${code}`))
        return
      }
      try {
        resolve(JSON.parse(Buffer.concat(stdout).toString('utf8')))
      } catch (cause) {
        reject(new Error(`solomon-sdmod returned invalid JSON: ${cause.message}`))
      }
    })
  })
}

function checkedValue(value) {
  return {
    ...value,
    files: new Map(Object.entries(value.files).map(([path, bytes]) => [path, Buffer.from(bytes, 'base64')])),
    scripts: new Map(Object.entries(value.scripts)),
  }
}

export function readAuthoringInventory() {
  return invoke(['reference'])
}

export async function checkPackage(root, admit = true) {
  return checkedValue(await invoke([admit ? 'check' : 'inspect', root]))
}

export async function bundlePackage(root) {
  return checkedValue(await invoke(['bundle', root]))
}

export function testPackage(root) {
  return invoke(['test', root])
}
