import { createHash } from 'node:crypto'
import { createReadStream } from 'node:fs'
import { rm, writeFile } from 'node:fs/promises'
import { join, resolve } from 'node:path'
import { repositoryRoot, run } from './desktop-build.mjs'

const target = `${process.platform}-${process.arch}`
const directory = `Solomon Darker-${target}`
const filename = `Solomon-Darker-${target}.zip`
const archive = resolve('dist-desktop', filename)
await rm(archive, { force: true })
if (process.platform === 'darwin') {
  await run('ditto', ['-c', '-k', '--sequesterRsrc', '--keepParent', resolve('dist-desktop', directory), archive])
} else if (process.platform === 'win32') {
  await run(join(process.env.SystemRoot, 'System32', 'tar.exe'), ['-a', '-cf', archive, '-C', resolve('dist-desktop'), directory])
} else {
  await run('zip', ['-qry', archive, directory], resolve('dist-desktop'))
}
await run(process.env.PYTHON || (process.platform === 'win32' ? 'python' : 'python3'),
  [join(repositoryRoot, 'scripts/verify-desktop-archive.py'), archive])
const hash = createHash('sha256')
for await (const chunk of createReadStream(archive)) hash.update(chunk)
await writeFile(`${archive}.sha256`, `${hash.digest('hex')}  ${filename}\n`)
