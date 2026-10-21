// Matched source/Rust GPU captures. The Website checkout is imported without edits.
const fs = require('node:fs')
const path = require('node:path')
const { chromium } = require(process.env.PW || 'playwright-core')

const root = path.resolve(__dirname, '../..')
const output = process.env.OUT || '/Users/jarrett/codex-runs/solomon-rust-fleet-20261020/npcs/secondary-gpu'
const read = name => {
  let filename = path.join(root, 'crates/sim/tests/fixtures/website', name)
  if (!fs.existsSync(filename)) filename += '.zlib'
  const bytes = fs.readFileSync(filename)
  return JSON.parse(filename.endsWith('.zlib') ? require('node:zlib').inflateSync(bytes) : bytes)
}
const clone = value => JSON.parse(JSON.stringify(value))

async function referenceInit(options = {}) {
  // Use Vite's current optimized module URL so Pixi registers its extensions once.
  const viewSource = await (await fetch('/src/game/renderer/native-secondary-world-view.ts')).text()
  const pixiUrl = viewSource.match(/from "([^"]*pixi__js\.js[^\"]*)"/)[1]
  const pixi = await import(pixiUrl)
  const { NativeSecondaryWorldView } = await import('/src/game/renderer/native-secondary-world-view.ts')
  const { PlayerHardenEffectView } = await import('/src/game/renderer/player-harden-effect-view.ts')
  const { PlayerStaffVfxView, PlayerStaffPikeBreakView } = await import('/src/game/renderer/player-staff-vfx-view.ts')
  const assets = await import('/src/game/renderer/native-secondary-assets.ts')
  const lootAssets = await import('/src/game/renderer/native-loot-assets.ts')
  const { NativeLootViews, NativeGoodieViews } = await import('/src/game/renderer/native-loot-view.ts')
  const lootPlans = await import('/src/game/renderer/native-loot-presentation.ts')
  const atlasModule = await import('/src/game/renderer/boneyard-combat-atlas.ts')
  const { boneyardCombatAssetSource } = await import('/src/game/renderer/boneyard-combat-asset-source.ts')
  const fixed = await import('/src/game/renderer/native-fixed-function-render-pipeline.ts')
  const { installNativeArenaRenderPipeline } = await import('/src/game/renderer/native-arena-render-pipeline.ts')
  const { primarySpells: primary } = await import('/src/lib/assets.ts')
  const { buildNativeRegionPainterOrder } = await import('/src/game/region-painter-order.ts')
  const sources = [...atlasModule.BONEYARD_COMBAT_ATLAS_SOURCES,
    ...assets.NATIVE_SECONDARY_ASSET_SOURCES.filter(source => !atlasModule.boneyardCombatAtlasSourceIsPacked(source))]
  const bases = new Map()
  await Promise.all(sources.map(async source => {
    const image = new Image()
    image.src = source
    await image.decode()
    const framed = assets.NATIVE_SECONDARY_STOCK_FRAMED_ASSET_SOURCES.includes(source)
    bases.set(source, framed ? fixed.nativeStockFramedTextureFromImage(image) : fixed.nativeStockTextureFromImage(image))
  }))
  const atlas = atlasModule.createBoneyardCombatAtlas(source => {
    const found = bases.get(source)
    if (!found) throw new Error(`Missing atlas page ${source}`)
    return found
  })
  const texture = source => {
    const found = atlasModule.boneyardCombatAtlasSourceIsPacked(source) ? atlas.single(source) : bases.get(source)
    if (!found) throw new Error(`Missing secondary texture ${source}`)
    return found
  }
  const airTexture = source => texture(boneyardCombatAssetSource(source))
  const textures = {
    base: Object.fromEntries(lootAssets.NATIVE_LOOT_ASSET_SOURCES.map(source => [source, texture(source)])),
    secondary: Object.fromEntries(assets.NATIVE_SECONDARY_SPRITE_RECORDS.map(record =>
      [assets.nativeSecondarySpriteKey(record.atlas, record.entry), texture(record.source)])),
    secondarySpecial: { etherPlane: texture(assets.NATIVE_SECONDARY_SPECIAL_ASSET_SOURCES.etherPlane) },
    primarySpells: { air: {
      branches: primary.air.branches.map(airTexture),
      circle: airTexture(primary.air.circle),
      forks: primary.air.forks.map(airTexture),
      ribbon: airTexture(primary.air.ribbon),
    } },
  }
  const app = new pixi.Application()
  await app.init({ width: 640, height: 480, resolution: 1, preference: 'webgl', antialias: false,
    autoStart: false, background: 0x1f1f1f, preserveDrawingBuffer: true })
  app.stop()
  fixed.installNativeFixedFunctionRenderPipeline(app.renderer, {
    installTextureAlphaShaders: !options.productionArena,
    nativeWorldPixelCenters: options.nativeWorldPixelCenters === true,
  })
  app.canvas.id = 'secondary-fixture'
  Object.assign(app.canvas.style, { position: 'fixed', left: '0', top: '0', zIndex: '1000000' })
  document.body.appendChild(app.canvas)
  const pre = new pixi.Container({ sortableChildren: true })
  const world = new pixi.Container({ sortableChildren: true, isRenderGroup: options.productionArena === true })
  installNativeArenaRenderPipeline(app.renderer, options.productionArena ? world : undefined)
  app.stage.addChild(pre, world)
  let view = null
  let staffViews = []
  window.renderSourceSecondary = input => {
    if (view) view.destroy()
    for (const view of staffViews) view.destroy()
    staffViews = []
    app.renderer.resize(input.width, input.height)
    for (const container of [pre, world]) {
      container.position.set(-input.origin.x * input.scale, -input.origin.y * input.scale)
      container.scale.set(input.scale)
    }
    view = new NativeSecondaryWorldView(world, textures, app.renderer, { preWorldRoot: pre })
    view.update({ actors: input.actors }, input.worldKey, input.frame ?? undefined, () => input.pointGain)
    const entries = view.painterLayers().filter(layer => layer.lane !== 'pre-world-queue')
    const staffRoots = new Map()
    for (const actor of input.staff || []) {
      const staffView = actor.kind === 'player-staff-pike-break'
        ? new PlayerStaffPikeBreakView(actor, textures) : new PlayerStaffVfxView(actor, textures)
      staffViews.push(staffView)
      staffView.setTint('', input.worldTint)
      world.addChild(...staffView.containers)
      const root = staffView.painterRoots()[0]
      const id = `staff:${actor.id}`
      staffRoots.set(id, root.container)
      entries.push({ id, worldY: root.worldY, sortBias: root.sortBias, visible: true,
        registration: actor.painterRegistrations[0] })
    }
    const order = buildNativeRegionPainterOrder({ entries, referenceY: input.referenceY })
    for (const layer of order.orderedLayers) {
      if (staffRoots.has(layer.id)) staffRoots.get(layer.id).zIndex = 1000 + layer.zIndex
      else view.setDepth(layer.id, 1000 + layer.zIndex)
    }
    if (input.hit) view.setPuppetHits(new Map([[`secondary:${input.hit.id}`, {
      feedback: input.hit.feedback, hitTick: input.hit.feedback.tick,
      kind: 'leviathan', targetId: `secondary:${input.hit.id}`,
    }]]), input.hit.complexLighting)
    app.renderer.render({ container: app.stage, clear: true })
    return { actors: view.count + staffViews.length, primitives: view.primitiveCount,
      painters: view.painterLayers().length + staffViews.length }
  }
  let loot = null
  let goodies = null
  window.renderSourceLoot = input => {
    if (view) { view.destroy(); view = null }
    for (const view of staffViews) view.destroy()
    staffViews = []
    if (input.reset || !loot) {
      if (loot) loot.destroy()
      if (goodies) goodies.destroy()
      loot = new NativeLootViews(world, textures, {}, [])
      goodies = new NativeGoodieViews(world, textures)
    }
    app.renderer.resize(input.width, input.height)
    world.position.set(-input.origin.x * input.scale, -input.origin.y * input.scale)
    world.scale.set(input.scale)
    loot.update(input.sourceActors)
    const hits = new Map(input.sourceHits.map(hit => [hit.targetId, hit]))
    goodies.update(input.sourceGoodies, input.tick, hits, input.complexLighting)
    const entries = [...input.sourceActors.map(lootPlans.nativeLootPainterLayer),
      ...input.sourceGoodies.map(lootPlans.nativeGoodiePainterLayer)]
    const order = buildNativeRegionPainterOrder({ entries, referenceY: input.referenceY })
    for (const layer of order.orderedLayers) {
      const [kind, id] = layer.id.split(':')
      const views = kind === 'loot' ? loot : goodies
      views.setDepth(Number(id), 1000 + layer.zIndex)
      views.setTint(Number(id), input.worldTint)
    }
    app.renderer.render({ container: app.stage, clear: true })
    return { actors: loot.size + goodies.size, painters: entries.length }
  }
  let hardenViews = []
  window.renderSourceHarden = input => {
    if (view) { view.destroy(); view = null }
    for (const prior of hardenViews) prior.destroy()
    hardenViews = []
    app.renderer.resize(input.width, input.height)
    world.position.set(-input.origin.x * input.scale, -input.origin.y * input.scale)
    world.scale.set(input.scale)
    const entries = []
    const roots = new Map()
    let queueDepth = 100000
    const plans = []
    for (const actor of input.sourceActors) {
      const current = new PlayerHardenEffectView(actor, textures.secondary)
      hardenViews.push(current)
      world.addChild(...current.containers)
      const root = current.painterRoots()[0]
      const sprite = root.container.children[0]
      plans.push({ lane: root.lane, worldY: root.worldY, sortBias: root.sortBias,
        record: actor.kind === 'harden-shard' ? actor.record : 15,
        position: { x: root.container.x, y: root.container.y },
        alpha: sprite.alpha, rotation: sprite.rotation, height: sprite.y,
        scale: { x: sprite.scale.x, y: sprite.scale.y } })
      if (root.lane === 'world-sorted') {
        const id = `harden:${actor.id}`
        roots.set(id, root.container)
        entries.push({ id, worldY: root.worldY, sortBias: root.sortBias, visible: true,
          registration: actor.painterRegistrations[0] })
      } else {
        root.container.zIndex = queueDepth++
      }
    }
    const order = buildNativeRegionPainterOrder({ entries, referenceY: input.referenceY })
    for (const layer of order.orderedLayers) roots.get(layer.id).zIndex = 1000 + layer.zIndex
    app.renderer.render({ container: app.stage, clear: true })
    return { actors: hardenViews.length, worldPainters: entries.length, plans }
  }
  window.secondaryFixtureGl = app.renderer.gl
}

async function pixels(page) {
  const encoded = await page.evaluate(() => {
    const canvas = document.querySelector('#secondary-fixture')
    const gl = window.secondaryFixtureGl || canvas.getContext('webgl2')
    const data = new Uint8Array(canvas.width * canvas.height * 4)
    gl.readPixels(0, 0, canvas.width, canvas.height, gl.RGBA, gl.UNSIGNED_BYTE, data)
    let text = ''
    for (let offset = 0; offset < data.length; offset += 32768) {
      text += String.fromCharCode(...data.subarray(offset, offset + 32768))
    }
    return btoa(text)
  })
  return Buffer.from(encoded, 'base64')
}

function compare(left, right) {
  if (left.length !== right.length) throw new Error('Framebuffer dimensions differ')
  let difference = 0
  let maximum = 0
  let changedPixels = 0
  let contentPixels = 0
  for (let offset = 0; offset < left.length; offset += 4) {
    let changed = false
    let content = false
    for (let channel = 0; channel < 3; channel++) {
      const delta = Math.abs(left[offset + channel] - right[offset + channel])
      difference += delta
      maximum = Math.max(maximum, delta)
      changed ||= delta > 4
      content ||= left[offset + channel] !== 31 || right[offset + channel] !== 31
    }
    if (changed) changedPixels++
    if (content) contentPixels++
  }
  return { meanRgbDifference: difference / (left.length / 4 * 3), maximum, changedPixels, contentPixels }
}

async function main() {
  const { loadKernel } = await import('../website-oracle/oracle.mjs')
  const secondary = await loadKernel('core-kernels/native-secondary-abilities.ts')
  const { nativePuppetHitAlpha } = await loadKernel('core-kernels/native-puppet-hit.ts')
  const { createNativeWorldManagerOrder } = await loadKernel('core-kernels/native-world-manager-order.ts')
  const ticks = read('secondary-tick-goldens.json')
  const plans = read('secondary-presentation-goldens.json')
  const staffActions = read('staff-action-goldens.json')
  const staffPresentation = read('staff-presentation-goldens.json')
  const staffKernel = await loadKernel('core-kernels/native-player-staff-action.ts')
  const hardenFixture = read('player-harden-host-goldens.json')
  const hardenCases = [0, 1, 5, 10].map((tick, index) => {
    const source = clone(hardenFixture.cases.find(row => row.before === 1 && row.after === 0 && row.chips.length > 0).expected.spells.transients)
    return { name: `harden-break-${tick}`, tick, source, scale: [1, 1.5, 0.75, 2][index] }
  })
  const hardenKernel = await loadKernel('core-kernels/native-harden-effects.ts')
  const nativeRng = await loadKernel('core-kernels/native-rng.ts')
  const cases = []
  const recorded = (name, tick, scale) => {
    const journey = ticks.cases.find(value => value.name === name)
    const sample = journey?.samples.find(value => value.tick === tick)
    if (!sample) throw new Error(`Missing recorded case ${name}:${tick}`)
    cases.push({ name: `${name}-${tick}`, actors: sample.state.actors, scale })
  }
  recorded('11-normal', 250, 1.5)
  recorded('27-normal', 100, 1)
  recorded('27-maximum', 100, 1)
  recorded('27-enhanced', 150, 0.75)
  recorded('45-normal', 150, 1.5)
  recorded('45-normal', 500, 1.5)
  recorded('45-enhanced', 500, 1.5)
  recorded('21-normal', 50, 1)
  recorded('35-enhanced', 50, 0.75)
  recorded('30-normal', 50, 0.75)
  recorded('72-enhanced', 100, 0.75)
  recorded('74-normal', 100, 1)
  recorded('76-normal', 150, 0.5)
  recorded('11-maximum', 500, 1)
  const hitActors = cases.at(-1).actors
  for (const complexLighting of [false, true]) for (const timer of [0.25, 0.7, 1]) {
    const feedback = { strength: 1, tick: 500, timer }
    cases.push({ name: `leviathan-hit-${complexLighting}-${timer}`, actors: hitActors, scale: 1,
      hit: { id: hitActors.find(actor => actor.kind === 'leviathan').id, feedback,
        alpha: nativePuppetHitAlpha(feedback, feedback.tick), complexLighting,
        tint: Math.trunc((complexLighting ? Math.fround(0.65) : 1) * 255) << 16 } })
  }
  for (const kind of ['plane-orb-shot', 'dampen-wave', 'dampened-projectile', 'shield-explosion', 'golem-death', 'comet-impact']) {
    cases.push({ name: `${kind}-9`, actors: [plans.cases.find(value => value.name === `${kind}-9`).actor], scale: 0.75 })
  }
  for (const age of [0, 1]) cases.push({ name: `storm-strike-${age}`,
    actors: [plans.cases.find(value => value.name === 'storm-strike-0').actor].map(actor => ({ ...actor, ageTicks: age })), scale: 1 })
  for (const outcome of ['knockback', 'critical-hit', 'disabling-hit', 'whirl']) {
    const presentation = staffActions.presentations.find(value => value.action.outcome === outcome && value.targetIds.length === 3)
    let staff = presentation.result.vfx
    for (let age = 0; age <= 5; age++) {
      if (age === 0 || age === 5) cases.push({ name: `staff-${outcome}-${age}`, actors: [], staff, scale: 1 })
      staff = staff.map(staffKernel.stepNativePlayerStaffVfx).filter(Boolean)
    }
  }
  for (const age of [0, 25, 50, 75, 99]) cases.push({ name: `staff-pike-${age}`, actors: [],
    staff: [staffPresentation.pikes.find(value => value.actor.ageTicks === age).actor], scale: 1.5 })
  const lootFixture = read('loot-presentation-goldens.json')
  const layout = rows => rows.map((row, index) => ({ ...clone(row), id: index + 1,
    position: { x: 75 + index % 5 * 115, y: 120 + Math.floor(index / 5) * 100 },
    painterRegistration: { managerLane: 'actor', registrationOrdinal: index + 60 } }))
  const select = predicate => lootFixture.actors.filter(({ actor }) => predicate(actor)).map(({ actor }) => actor)
  const lootCases = [
    { name: 'loot-orbs', loot: layout(select(a => a.kind === 'orb' && a.animationPhase === 37.5)), goodies: [], reset: true },
    { name: 'loot-gold-scatter-0', loot: layout(select(a => a.kind === 'gold' && a.scatterActive && a.scatterProgress === 3.8 && a.animationPhase === 90)), goodies: [], reset: true },
    { name: 'loot-gold-scatter-1', loot: layout(select(a => a.kind === 'gold' && a.scatterActive && a.scatterProgress === 3.8 && a.animationPhase === 90)), goodies: [], reset: false },
    { name: 'loot-gold-settled', loot: layout(select(a => a.kind === 'gold' && !a.scatterActive && a.scatterProgress === 0)), goodies: [], reset: true },
    { name: 'loot-sacks', loot: layout(select(a => a.kind === 'sack' && (a.tier === 0 || a.itemNativeTypeId === 7002))), goodies: [], reset: true },
    { name: 'loot-bonuses', loot: layout(select(a => a.kind === 'bonus' && a.framePhase === 7.9)), goodies: [], reset: true },
  ]
  for (const complexLighting of [false, true]) {
    const rows = lootFixture.goodies.filter(value => value.tick === 19 && value.goodie.active && value.goodie.timer === 99 && value.complexLighting === complexLighting)
    lootCases.push({ name: `loot-goodies-hit-${complexLighting}`, loot: [], reset: true, complexLighting,
      goodies: rows.map(({ goodie }, index) => ({ ...goodie, id: index + 1, sceneryRegistrationOrdinal: index + 4,
        position: { x: 105 + index % 3 * 190, y: 170 + Math.floor(index / 3) * 170 } })),
      hits: rows.flatMap(({ hit }, index) => hit ? [{ feedback: hit, hitTick: 19, kind: 'goodie', targetId: `goodie:${index + 1}` }] : []),
    })
  }
  fs.mkdirSync(output, { recursive: true })
  const browser = await chromium.launch({ headless: true, executablePath: '/Applications/Google Chrome.app/Contents/MacOS/Google Chrome',
    args: ['--use-angle=swiftshader', '--enable-unsafe-swiftshader', '--disable-background-timer-throttling'] })
  const problems = []
  const results = []
  try {
    const sourcePage = await browser.newPage({ viewport: { width: 1600, height: 900 } })
    const rustPage = await browser.newPage({ viewport: { width: 1600, height: 900 } })
    // The isolated source view needs Vite modules and assets, not the Website shell/API.
    await sourcePage.route('http://127.0.0.1:4178/', route => route.fulfill({
      contentType: 'text/html', body: '<!doctype html><html><body></body></html>',
    }))
    for (const [label, page] of [['js', sourcePage], ['rust', rustPage]]) {
      page.on('pageerror', error => { problems.push(`${label}: ${error.message}`); console.error(problems.at(-1)) })
      page.on('console', message => {
        if (message.type() === 'error' && !message.location().url.endsWith('/favicon.ico')) {
          problems.push(`${label}: ${message.text()}`); console.error(problems.at(-1))
        }
      })
      page.on('response', response => {
        if (response.status() >= 400) console.error(`${label}: HTTP ${response.status()} ${response.url()}`)
      })
    }
    console.log('Loading matched pages')
    await sourcePage.goto(process.env.REFERENCE_URL || 'http://127.0.0.1:4178', { waitUntil: 'networkidle' })
    await rustPage.goto(process.env.URL || 'http://127.0.0.1:4184', { waitUntil: 'networkidle' })
    console.log('Loading source GPU view')
    await sourcePage.evaluate(referenceInit)
    console.log('Loading Rust GPU view')
    await rustPage.waitForFunction(() => window.wasmBindings?.secondary_fixture_init)
    await rustPage.evaluate(() => window.wasmBindings.secondary_fixture_init())
    console.log('Capturing effects')
    for (const value of cases.filter(value => (!process.env.ONLY || value.name === process.env.ONLY)
      && (!process.env.GROUP || value.name.startsWith(process.env.GROUP)))) {
      const actors = clone(value.actors)
      const staff = clone(value.staff || [])
      for (const actor of actors) actor.painterRegistrations = []
      const order = createNativeWorldManagerOrder()
      const state = secondary.enrollNativeSecondaryPainterOwners({ ...secondary.createNativeSecondarySimulation(0), actors }, order.register)
      for (const actor of staff) actor.painterRegistrations = [order.register('actor')]
      const focus = actors.find(actor => ['leviathan', 'storm-cloud', 'golem'].includes(actor.kind)) || actors[0] || staff[0]
      if (!focus) throw new Error(`No active actors in ${value.name}`)
      const input = { actors: state.actors, staff, width: 640, height: 480, worldKey: focus.worldKey,
        origin: { x: focus.position.x - 320 / value.scale, y: focus.position.y - 280 / value.scale },
        scale: value.scale, frame: null, pointGain: 0.75, referenceY: focus.position.y, saturation: 0.65,
        hit: value.hit, worldTint: 0xb0c0d0 }
      const js = await sourcePage.evaluate(input => window.renderSourceSecondary(input), input)
      const rust = await rustPage.evaluate(input => JSON.parse(window.wasmBindings.render_secondary_fixture(JSON.stringify(input))), input)
      const delta = compare(await pixels(sourcePage), await pixels(rustPage))
      await sourcePage.locator('#secondary-fixture').screenshot({ path: path.join(output, `${value.name}-js.png`) })
      await rustPage.locator('#secondary-fixture').screenshot({ path: path.join(output, `${value.name}-rust.png`) })
      const result = { name: value.name, kinds: [...new Set([...actors, ...staff].map(actor => actor.kind))], js, rust, ...delta }
      results.push(result)
      console.log(JSON.stringify(result))
    }
    for (const value of lootCases.filter(value => (!process.env.ONLY || value.name === process.env.ONLY)
      && (!process.env.GROUP || value.name.startsWith(process.env.GROUP)))) {
      const canonicalActor = actor => ({ activation_delay_ticks: 0, age_ticks: 0, alpha: actor.alpha,
        amount: 0, animation_phase: actor.animationPhase, bonus_kind: ['SkillChoice', 'SkillRank', 'DamageX4'][actor.bonusKind] || null,
        bounce_height: actor.bounceHeight, bounce_velocity: 0, frame_phase: actor.framePhase, id: actor.id,
        item: actor.itemNativeTypeId === null ? null : { contents: null, equipment_type: null, generated_level: null,
          icon_records: [], icon_tints: null, id: actor.id, inventory_slot: null, kind: 'Equipment', name: 'source loot',
          native_subtype: actor.itemNativeSubtype, native_selector: null, native_effects: null, native_type_id: actor.itemNativeTypeId,
          quantity: 1, rarity: null, recipe_index: null },
        kind: { orb: 'Orb', gold: 'Gold', sack: 'Sack', bonus: 'Bonus' }[actor.kind], last_step_tick: null,
        lifetime_ticks_remaining: null, native_type_id: 0, orb_kind: actor.orbKind === 'health' ? 'Health' : 'Mana',
        orb_value: actor.orbValue, painter_registration: { manager_lane: 'Actor', registration_ordinal: actor.painterRegistration.registrationOrdinal },
        position: actor.position, rotation_deg: actor.rotationDeg, scatter_seed: actor.scatterSeed,
        scatter_active: actor.scatterActive, scatter_progress: actor.scatterProgress, settled_sound_emitted: false,
        source: 'Script', spawn_tick: 0, tier: actor.tier })
      const input = { width: 640, height: 480, origin: { x: 0, y: 0 }, scale: 1, referenceY: 240,
        tick: 19, saturation: 0.65, worldTint: 0xb0c0d0, complexLighting: value.complexLighting ?? true,
        sourceActors: value.loot, sourceGoodies: value.goodies, sourceHits: value.hits || [], reset: value.reset,
        lootActors: value.loot.map(canonicalActor),
        goodies: value.goodies.map(goodie => ({ active: goodie.active, activated_by_player_id: null,
          eid: `goodie:${goodie.id}`, exhausted: false, id: goodie.id, phase: ['Intact', 'Opening', 'Broken'][goodie.phase],
          position: goodie.position, reward_seed: 0, scenery_registration_ordinal: goodie.sceneryRegistrationOrdinal, subtype: 0, timer: goodie.timer })),
        hits: (value.hits || []).map(hit => ({ feedback: hit.feedback, hit_tick: hit.hitTick, kind: 'Goodie', target_id: hit.targetId })),
      }
      const js = await sourcePage.evaluate(input => window.renderSourceLoot(input), input)
      const rust = await rustPage.evaluate(input => JSON.parse(window.wasmBindings.render_loot_fixture(JSON.stringify(input))), input)
      const delta = compare(await pixels(sourcePage), await pixels(rustPage))
      await sourcePage.locator('#secondary-fixture').screenshot({ path: path.join(output, `${value.name}-js.png`) })
      await rustPage.locator('#secondary-fixture').screenshot({ path: path.join(output, `${value.name}-rust.png`) })
      const result = { name: value.name, js, rust, ...delta }
      results.push(result)
      console.log(JSON.stringify(result))
    }
    for (const value of hardenCases.filter(value => (!process.env.ONLY || value.name === process.env.ONLY)
      && (!process.env.GROUP || value.name.startsWith(process.env.GROUP)))) {
      let source = value.source
      let rng = nativeRng.createNativeRng(721)
      for (let tick = 203; tick <= 202 + value.tick; tick++) {
        const retained = []
        for (const actor of source) {
          const result = hardenKernel.stepNativeHardenEffect(actor, tick, rng, () => true)
          rng = result.rng
          if (result.effect) retained.push(result.effect)
        }
        source = retained
      }
      const canonical = actor => {
        const common = { age_ticks: actor.ageTicks, id: actor.id, light_registration: null,
          owner_id: actor.ownerId, world_key: actor.worldKey,
          painter_registrations: actor.painterRegistrations.map(row => ({ manager_lane: row.managerLane === 'actor' ? 'Actor' : 'Transient', registration_ordinal: row.registrationOrdinal })) }
        const owner = { common, birth_tick: actor.birthTick }
        return { Harden: actor.kind === 'harden-burst' ? { Burst: { owner, alpha: actor.alpha, position: actor.position } }
          : { Shard: { owner, body: { bounce_velocity: actor.bounceVelocity, height: actor.height, life: actor.life,
            position: actor.position, record: actor.record, rotation_degrees: actor.rotationDegrees,
            rotation_step_degrees: actor.rotationStepDegrees, velocity: actor.velocity, vertical_velocity: actor.verticalVelocity } } } }
      }
      const input = { sourceActors: source, spells: { next_id: 101, projectiles: [], transients: source.map(canonical) },
        width: 640, height: 480, origin: { x: 321.25 - 320 / value.scale, y: 245.5 - 280 / value.scale },
        scale: value.scale, referenceY: 245.5, saturation: 0.65 }
      const js = await sourcePage.evaluate(input => window.renderSourceHarden(input), input)
      const rust = await rustPage.evaluate(input => JSON.parse(window.wasmBindings.render_harden_fixture(JSON.stringify(input))), input)
      if (js.actors !== rust.actors || js.worldPainters !== rust.worldPainters) throw new Error(`Harden painter mismatch: ${value.name}`)
      const delta = compare(await pixels(sourcePage), await pixels(rustPage))
      await sourcePage.locator('#secondary-fixture').screenshot({ path: path.join(output, `${value.name}-js.png`) })
      await rustPage.locator('#secondary-fixture').screenshot({ path: path.join(output, `${value.name}-rust.png`) })
      const result = { name: value.name, js, rust, ...delta }
      results.push(result)
      console.log(JSON.stringify(result))
    }
  } finally {
    fs.writeFileSync(path.join(output, 'results.json'), JSON.stringify({ results, problems }, null, 2))
    await browser.close()
  }
  if (problems.length) throw new Error(problems.join('\n'))
}
module.exports = { referenceInit, pixels, compare }
if (require.main === module) main().catch(error => { console.error(error); process.exitCode = 1 })
