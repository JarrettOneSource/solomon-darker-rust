// Build the unchanged Website forms with deployment-scoped HTTP and storage paths.
import { createRequire } from 'node:module'
import { readFile, writeFile, mkdir, mkdtemp, copyFile, symlink, rm, realpath } from 'node:fs/promises'
import { tmpdir } from 'node:os'
import { dirname, resolve, join } from 'node:path'
import { fileURLToPath, pathToFileURL } from 'node:url'
import { execFileSync } from 'node:child_process'
import { createHash } from 'node:crypto'

const [websiteArgument, outputArgument, mount = '/game-stg'] = process.argv.slice(2)
if (!websiteArgument || !outputArgument || !/^\/[a-z0-9/-]+$/.test(mount) || mount.endsWith('/')) {
  throw new Error('Usage: node build.mjs WEBSITE_FRONTEND OUTPUT_DIRECTORY /game-stg')
}
const website = resolve(websiteArgument)
const output = resolve(outputArgument)
const ownDirectory = dirname(fileURLToPath(import.meta.url))
const require = createRequire(join(website, 'package.json'))
const load = async name => import(pathToFileURL(require.resolve(name)).href)
const { build } = await load('vite')
const { default: react } = await load('@vitejs/plugin-react')
const { default: tailwindcss } = await load('@tailwindcss/vite')
const work = await realpath(await mkdtemp(join(tmpdir(), 'solomon-account-')))
try {
  await symlink(join(website, 'node_modules'), join(work, 'node_modules'), 'dir')
  await copyFile(join(ownDirectory, 'main.tsx'), join(work, 'main.tsx'))
  await writeFile(join(work, 'index.html'), `<!doctype html><html lang="en"><head>
  <meta charset="utf-8"><meta name="viewport" content="width=device-width, initial-scale=1">
  <link rel="icon" href="${mount}/assets/stock/favicon.png"><title>Solomon Darker — Sign In</title>
  </head><body><div id="root"></div><script type="module" src="./main.tsx"></script></body></html>\n`)
  await writeFile(join(work, 'account.css'), `@import ${JSON.stringify(join(website, 'src/index.css'))};
  @source ${JSON.stringify(join(website, 'src/pages/Login.tsx'))};
  @source ${JSON.stringify(join(website, 'src/pages/Register.tsx'))};
  @source ${JSON.stringify(join(website, 'src/components/ui.tsx'))};\n`)
  await writeFile(join(work, 'tsconfig.json'), JSON.stringify({
    extends: join(website, 'tsconfig.app.json'),
    compilerOptions: {
      paths: { '@website/*': [join(website, 'src/*')] },
      tsBuildInfoFile: join(work, 'account.tsbuildinfo'),
    },
    include: ['main.tsx'],
  }))
  execFileSync(process.execPath, [require.resolve('typescript/bin/tsc'), '-p', join(work, 'tsconfig.json')],
    { cwd: work, stdio: 'inherit' })
  const apiPath = join(website, 'src/lib/api.ts')
  const sourceRevision = execFileSync('git', ['rev-parse', 'HEAD'], { cwd: website, encoding: 'utf8' }).trim()
  const mountApi = {
    name: 'mounted-account-api',
    enforce: 'pre',
    transform(code, id) {
      if (id !== apiPath) return null
      const token = "const TOKEN_KEY = 'sdr.token'"
      const request = 'fetch(path, { ...init, headers })'
      if (code.split(token).length !== 2 || code.split(request).length !== 2) {
        throw new Error('The Website account API changed; review the mounted boundary')
      }
      return { code: code.replace(token, `const TOKEN_KEY = ${JSON.stringify(`sdr.token:${mount}`)}`)
        .replace(request, `fetch(${JSON.stringify(mount)} + path, { ...init, headers })`), map: null }
    },
  }
  await build({
    configFile: false, root: work, publicDir: false, base: `${mount}/account/`,
    define: { __GAME_MOUNT__: JSON.stringify(mount) },
    resolve: { alias: { '@website': join(website, 'src') }, dedupe: ['react', 'react-dom'] },
    plugins: [mountApi, react(), tailwindcss()],
    build: { outDir: output, emptyOutDir: true },
  })
  const references = ['src/pages/Login.tsx', 'src/pages/Register.tsx', 'src/lib/auth.tsx', 'src/lib/api.ts']
  const hashes = {}
  for (const reference of references) {
    hashes[reference] = createHash('sha256').update(await readFile(join(website, reference))).digest('hex')
  }
  await mkdir(output, { recursive: true })
  await writeFile(join(output, 'account-build.json'), JSON.stringify({ sourceRevision, mount, sourceFiles: hashes }, null, 2) + '\n')
  console.log(JSON.stringify({ output, sourceRevision, mount }))
} finally {
  await rm(work, { recursive: true, force: true })
}
