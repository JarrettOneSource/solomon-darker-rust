// Compare the unchanged production Source pipeline, its one-option control, and
// the actual Rust renderer. Actor rows come from the current authority recording.
const fs = require('node:fs')
const path = require('node:path')
const { createHash } = require('node:crypto')
const { execFileSync } = require('node:child_process')
const { chromium } = require(process.env.PW || 'playwright-core')
const { referenceInit, pixels, compare } = require('./secondary.cjs')

const output = process.env.OUT
if (!output || !process.env.REFERENCE_URL) throw new Error('OUT and REFERENCE_URL are required')
if (!process.env.SOURCE_ONLY && (!process.env.URL || !process.env.RUST_REVISION)) {
  throw new Error('URL and RUST_REVISION are required for the Rust comparison')
}
const sha256 = bytes => createHash('sha256').update(bytes).digest('hex')

function measure(left, right) {
  const result = compare(left, right)
  let changedRgbaBytes = 0
  for (let index = 0; index < left.length; index++) {
    if (left[index] !== right[index]) changedRgbaBytes++
  }
  return { ...result, changedRgbaBytes }
}

async function main() {
  const { loadKernel, readFixture, websiteDir } = await import('../website-oracle/oracle.mjs')
  const { createNativeWorldManagerOrder } = await loadKernel('core-kernels/native-world-manager-order.ts')
  const { createNativeSecondarySimulation, enrollNativeSecondaryPainterOwners } =
    await loadKernel('core-kernels/native-secondary-abilities.ts')
  const { nativePuppetHitAlpha } = await loadKernel('core-kernels/native-puppet-hit.ts')
  const { nativePuppetHitTint } = await loadKernel('renderer/native-texture-color.ts')
  const sourceRevision = execFileSync('git', ['rev-parse', 'HEAD'], { cwd: websiteDir, encoding: 'utf8' }).trim()
  const fixture = readFixture('secondary-tick-goldens.json.zlib')
  const journey = fixture.cases.find(row => row.name === '11-maximum')
  const sample = journey?.samples.find(row => row.tick === 500)
  if (!sample) throw new Error('Missing recorded Leviathan authority snapshot')
  const actors = structuredClone(sample.state.actors)
  const order = createNativeWorldManagerOrder()
  for (const actor of actors) actor.painterRegistrations = []
  const state = enrollNativeSecondaryPainterOwners({ ...createNativeSecondarySimulation(0), actors }, order.register)
  const owner = state.actors.find(actor => actor.kind === 'leviathan')
  if (!owner) throw new Error('Recorded snapshot has no Leviathan owner')
  const inputs = []
  for (const scale of [0.75, 1, 1.5]) {
    const common = {
      actors: state.actors, staff: [], width: 640, height: 480,
      worldKey: owner.worldKey, scale, frame: null, pointGain: 0.75,
      origin: { x: owner.position.x - 320 / scale, y: owner.position.y - 280 / scale },
      referenceY: owner.position.y, saturation: 0.65, enhancedEffects: true, worldTint: 0xffffff,
      nativeWorldPixelCenters: true,
    }
    inputs.push({ name: `normal-${scale}`, ...common })
    for (const complexLighting of [false, true]) {
      const feedback = { strength: 1, tick: 500, timer: 0.7 }
      inputs.push({ name: `hit-${complexLighting}-${scale}`, ...common, hit: {
        id: owner.id, feedback, complexLighting,
        alpha: nativePuppetHitAlpha(feedback, feedback.tick),
        tint: nativePuppetHitTint(complexLighting),
      } })
    }
  }
  fs.mkdirSync(output, { recursive: true })
  const encodedInputs = JSON.stringify(inputs)
  fs.writeFileSync(path.join(output, 'inputs.json'), encodedInputs)
  const receipt = { sourceRevision, inputSha256: sha256(encodedInputs),
    inputOrigin: 'current secondary-tick-goldens 11-maximum tick500; fresh Source painter enrollment',
    productionOptions: { nativeWorldPixelCenters: true, installTextureAlphaShaders: false, opaqueWorld: true },
    controlDifference: 'nativeWorldPixelCenters false only; actor authority and all other options identical',
    rustRevision: process.env.RUST_REVISION || null, wasm: [], gpu: {}, errors: [], results: [] }
  const server = await chromium.launchServer({ headless: true, host: '127.0.0.1',
    executablePath: process.env.CHROME || '/Applications/Google Chrome.app/Contents/MacOS/Google Chrome',
    args: ['--use-angle=metal', '--ignore-gpu-blocklist'] })
  const browser = await chromium.connect(server.wsEndpoint())
  try {
    const context = await browser.newContext({ viewport: { width: 1600, height: 900 }, ignoreHTTPSErrors: true })
    async function page(label, url, sourceOptions) {
      const page = await context.newPage()
      page.on('pageerror', error => receipt.errors.push(`${label}: ${error.message}`))
      page.on('console', message => {
        if (message.type() === 'error' && !message.location().url.endsWith('/favicon.ico')) {
          receipt.errors.push(`${label}: ${message.text()}`)
        }
      })
      if (sourceOptions) {
        await page.route(new URL('/', url).href, route => route.fulfill({
          contentType: 'text/html', body: '<!doctype html><html><body></body></html>',
        }))
      }
      await page.goto(url, { waitUntil: 'networkidle' })
      if (sourceOptions) await page.evaluate(referenceInit, sourceOptions)
      else {
        await page.waitForFunction(() => typeof window.wasmBindings?.secondary_fixture_init === 'function')
        await page.evaluate(() => window.wasmBindings.secondary_fixture_init())
        for (const url of await page.evaluate(() => performance.getEntriesByType('resource')
          .map(row => row.name).filter(name => name.endsWith('.wasm')))) {
          const response = await context.request.get(url)
          receipt.wasm.push({ url, sha256: sha256(await response.body()) })
        }
      }
      receipt.gpu[label] = await page.evaluate(() => {
        const gl = window.secondaryFixtureGl || document.querySelector('#secondary-fixture').getContext('webgl2')
        const extension = gl.getExtension('WEBGL_debug_renderer_info')
        return { version: gl.getParameter(gl.VERSION), renderer: extension
          ? gl.getParameter(extension.UNMASKED_RENDERER_WEBGL) : gl.getParameter(gl.RENDERER),
          contextAttributes: gl.getContextAttributes(), samples: gl.getParameter(gl.SAMPLES) }
      })
      return page
    }
    const production = await page('source-production', process.env.REFERENCE_URL,
      { productionArena: true, nativeWorldPixelCenters: true })
    const control = await page('source-control', process.env.REFERENCE_URL,
      { productionArena: true, nativeWorldPixelCenters: false })
    const rust = process.env.SOURCE_ONLY ? null : await page('rust', process.env.URL)
    for (const input of inputs) {
      for (const source of [production, control]) {
        await source.evaluate(input => window.renderSourceSecondary(input), input)
      }
      if (rust) await rust.evaluate(input => window.wasmBindings.render_secondary_fixture(JSON.stringify(input)), input)
      const productionPixels = await pixels(production)
      const controlPixels = await pixels(control)
      const result = { name: input.name, sourceProductionSha256: sha256(productionPixels),
        sourceControlSha256: sha256(controlPixels), sourceOptionDifference: measure(productionPixels, controlPixels) }
      result.glErrors = {}
      if (rust) {
        const rustPixels = await pixels(rust)
        result.rustSha256 = sha256(rustPixels)
        result.rustVsProduction = measure(productionPixels, rustPixels)
        result.rustVsControl = measure(controlPixels, rustPixels)
      }
      for (const [label, target] of [['source-production', production], ['source-control', control], ['rust', rust]]) {
        if (!target) continue
        result.glErrors[label] = await target.evaluate(() => {
          const gl = window.secondaryFixtureGl || document.querySelector('#secondary-fixture').getContext('webgl2')
          return gl.getError()
        })
        if (result.glErrors[label] !== 0) receipt.errors.push(`${input.name} ${label}: GL ${result.glErrors[label]}`)
        await target.locator('#secondary-fixture').screenshot({ path: path.join(output, `${input.name}-${label}.png`) })
      }
      receipt.results.push(result)
      console.log(JSON.stringify(result))
    }
  } finally {
    fs.writeFileSync(path.join(output, 'receipt.json'), `${JSON.stringify(receipt, null, 2)}\n`)
    await browser.close()
    await server.close()
  }
  if (receipt.errors.length) throw new Error(receipt.errors.join('\n'))
}
main().catch(error => { console.error(error); process.exitCode = 1 })
