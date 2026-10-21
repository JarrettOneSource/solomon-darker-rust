// The source redraw owner creates a real Sprite/Mesh child. Its shader setup needs a
// browser, so this bounded oracle starts read-only Vite with a task-owned external cache.
import { withSourceOraclePage } from './browser-oracle.mjs'
import { writeFixture } from './oracle.mjs'

const fixture = await withSourceOraclePage(page => page.evaluate(async () => {
    const source = await fetch('/src/game/renderer/native-scenery-hit-view.ts')
      .then(response => response.text())
    const pixiUrl = source.match(/from\s*["']([^"']*pixi__js\.js[^"']*)["']/)?.[1]
    if (!pixiUrl) throw new Error('Source Pixi module URL is missing')
    const [pixi, scenery, surfaces, colors, puppet, lighting, staticLighting] = await Promise.all([
      import(pixiUrl),
      import('/src/game/renderer/native-scenery-hit-view.ts'),
      import('/src/game/renderer/boneyard-building-surface-view.ts'),
      import('/src/game/renderer/native-texture-color.ts'),
      import('/src/game/core-kernels/native-puppet-hit.ts'),
      import('/src/game/renderer/boneyard-lighting.ts'),
      import('/src/game/renderer/boneyard-static-lighting.ts'),
    ])
    const cases = []
    for (const input of [
      { kind: 'tree', complex: true, parentAlpha: .65, scalar: .7, tick: 120.25, strength: .8, timer: .8 },
      { kind: 'tree', complex: false, parentAlpha: .65, scalar: 1, tick: 120.75, strength: .8, timer: .8 },
      { kind: 'building', complex: true, parentAlpha: 1, scalar: .7, tick: 120.25, strength: 0, timer: .8 },
      { kind: 'building', complex: false, parentAlpha: 1, scalar: 1, tick: 120.75, strength: .8, timer: .8 },
      { kind: 'gravestone', complex: true, parentAlpha: 1, scalar: .7, tick: 120.25, strength: .8, timer: .8 },
      { kind: 'post', complex: false, parentAlpha: 1, scalar: 1, tick: 120.75, strength: .8, timer: .8 },
      { kind: 'tree', complex: true, parentAlpha: .65, scalar: .7, tick: 140.25, strength: .8, timer: .8 },
    ]) {
      const root = new pixi.Container({ isRenderGroup: true })
      const createResident = () => {
        const surfaceMesh = input.kind === 'building'
          ? surfaces.createNativeLitSurfaceGrid(pixi.Texture.WHITE, 10, 20, true) : null
        const sprite = surfaceMesh?.mesh ?? new pixi.Sprite(pixi.Texture.WHITE)
        sprite.alpha = input.parentAlpha
        sprite.tint = surfaceMesh ? 0xffffff : lighting.nativeBoneyardLightTint(input.scalar)
        if (surfaceMesh) surfaceMesh.update(new Float32Array(9).fill(input.scalar))
        root.addChild(sprite)
        return { sprite, surfaceMesh }
      }
      const main = createResident()
      const proxy = input.kind === 'tree' || input.kind === 'building' ? createResident() : null
      const layers = [input.kind === 'post'
        ? { kind: 'fence', part: 'post', pieceIndex: 2 }
        : { kind: 'object', object: { eid: input.kind } }]
      const targetId = input.kind === 'post' ? 'fencepost:2' : `scenery:${input.kind}`
      const view = new scenery.NativeSceneryHitView(layers, new Map([[0, main]]),
        input.kind === 'tree' ? new Map([[input.kind, { main, proxy }]]) : new Map(),
        input.kind === 'building' ? new Map([[input.kind, { main, roof: proxy }]]) : new Map())
      const feedback = { strength: input.strength, tick: 120, timer: input.timer }
      view.update([{ kind: 'scenery', targetId, feedback }], input.tick, input.complex)
      const body = main.sprite.children[0]
      const upper = proxy?.sprite.children[0]
      const group = body.getGlobalTint()
      const vertexShade = main.surfaceMesh?.colors[0] ?? 255
      cases.push({
        input,
        hitAlpha: puppet.nativePuppetHitAlpha(feedback, input.tick),
        hitTimer: puppet.nativePuppetHitTimer(feedback, input.tick),
        body: {
          alpha: body.alpha, tint: body.tint, visible: body.visible,
          diffuse: main.surfaceMesh
            ? body.shader.resources.nativeTextureColor.uniforms.uIgnoreTextureColor === 1
            : colors.usesNativeDiffuseColor(body),
          rgba: [
            (group >> 16 & 255) / 255 * vertexShade / 255,
            (group >> 8 & 255) / 255 * vertexShade / 255,
            (group & 255) / 255 * vertexShade / 255,
            (body.getGlobalAlpha() * 255 | 0) / 255,
          ],
        },
        proxyVisible: upper?.visible ?? false,
      })
      view.update([], input.tick + 1, input.complex)
      if (main.sprite.children.length !== 0 || proxy && proxy.sprite.children.length !== 0) {
        throw new Error('Source scenery redraw did not retire')
      }
      view.destroy()
      main.surfaceMesh?.destroy()
      proxy?.surfaceMesh?.destroy()
      root.destroy({ children: true })
    }
    const wobbles = []
    for (const phase of [0, -4.25, 7.5]) {
      const position = { x: 100, y: 200 }
      const texture = new pixi.Texture({ source: new pixi.BufferImageSource({
        resource: new Uint8Array(10 * 20 * 4).fill(255), width: 10, height: 20,
        alphaMode: 'no-premultiply-alpha', format: 'rgba8unorm', scaleMode: 'linear',
      }) })
      const resident = (x, y) => {
        const sprite = new pixi.Sprite(texture)
        sprite.position.set(x, y)
        return { x, y, w: 10, h: 20, sprite }
      }
      const main = resident(80, 170)
      const proxy = resident(70, 100)
      const owner = new staticLighting.BoneyardStaticLighting(
        { runId: 'fixture', scene: { objects: [{ eid: 'tree' }] } }, [], new Map(), new Map(),
        new Map([['tree', { main, proxy }]]),
        [{ eid: 'tree', mainVariant: 0, position, secondaryVariant: 0, secondaryVisible: true }], 120)
      owner.update({ tick: 120, secondaryAbilities: { actors: [{
        kind: 'earthquake-scenery-wobble', worldKey: 'boneyard:fixture', targetId: 0, phase,
      }] } }, position, [], true,
      new lighting.NativeBoneyardLightIndex({ width: 400, height: 400 }), () => .7)
      for (const [role, view] of [['main', main], ['proxy', proxy]]) {
        const matrix = view.sprite.getGlobalTransform()
        const corners = [[0, 0], [view.w, 0], [view.w, view.h], [0, view.h]]
          .map(([x, y]) => matrix.apply({ x, y }))
        wobbles.push({ phase, position, role, bounds: [view.x, view.y, view.w, view.h], corners })
        view.sprite.destroy()
      }
      texture.destroy(true)
    }
    return { cases, wobbles }
}))
writeFixture('scenery-views-goldens.json', 'tools/website-oracle/scenery-views-goldens.mjs',
  'Pinned Website renderer/native-scenery-hit-view.ts actual managed Sprite/Mesh children and Pixi inherited batch color', fixture)
