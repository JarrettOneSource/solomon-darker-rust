import { spawn } from 'node:child_process'
import { createHash } from 'node:crypto'
import { createReadStream } from 'node:fs'
import { writeFile } from 'node:fs/promises'
import { join, resolve } from 'node:path'

const target = `${process.platform}-${process.arch}`
const directory = `Solomon Darker-${target}`
const filename = `Solomon-Darker-${target}.zip`
const archive = resolve('dist-desktop', filename)
if (process.platform === 'darwin') {
  await run('ditto', ['-c', '-k', '--sequesterRsrc', '--keepParent', resolve('dist-desktop', directory), archive])
} else if (process.platform === 'win32') {
  await run(join(process.env.SystemRoot, 'System32', 'tar.exe'), ['-a', '-cf', archive, '-C', resolve('dist-desktop'), directory])
} else {
  await run('zip', ['-qry', archive, directory], resolve('dist-desktop'))
}
const hash = createHash('sha256')
for await (const chunk of createReadStream(archive)) hash.update(chunk)
await writeFile(`${archive}.sha256`, `${hash.digest('hex')}  ${filename}\n`)

function run(command, args, cwd) {
  return new Promise((resolveRun, reject) => {
    const child = spawn(command, args, { cwd, stdio: 'inherit' })
    child.once('error', reject)
    child.once('exit', code => code === 0 ? resolveRun() : reject(new Error(`${command} failed (${code}).`)))
  })
}
