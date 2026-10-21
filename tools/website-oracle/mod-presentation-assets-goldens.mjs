import { createHash } from 'node:crypto'
import { withSourceOraclePage } from './browser-oracle.mjs'
import { writeFixture } from './oracle.mjs'

const fixture = await withSourceOraclePage(async page => {
  const images = await page.evaluate(() => {
    return [[64, 48], [4080, 170], [4080, 850], [4080, 1700], [3910, 170], [4080, 340]]
      .map(([width, height]) => {
        const canvas = document.createElement('canvas')
        canvas.width = width
        canvas.height = height
        return { width, height, data: canvas.toDataURL().split(',')[1] }
      })
  })
  const resources = new Map()
  const assets = images.map((image, index) => {
    const bytes = Buffer.from(image.data, 'base64')
    const sha256 = createHash('sha256').update(bytes).digest('hex')
    resources.set(sha256, bytes)
    return {
      byteLength: bytes.length, contentType: 'image/png', kind: 'image',
      modId: 'Example.Art', path: `image-${index}.png`, sha256,
    }
  })
  await page.route('**/api/game/content/*', route => {
    const bytes = resources.get(new URL(route.request().url()).pathname.split('/').at(-1))
    return route.fulfill({ status: bytes ? 200 : 404, contentType: 'image/png', body: bytes })
  })
  return page.evaluate(async assets => {
    const source = await fetch('/src/game/renderer/mod-presentation-assets.ts').then(response => response.text())
    const pixiUrl = source.match(/from\s*["']([^"']*pixi__js\.js[^"']*)["']/)?.[1]
    if (!pixiUrl) throw new Error('Source Pixi module URL is missing')
    const [{ Sprite }, mod] = await Promise.all([
      import(pixiUrl), import('/src/game/renderer/mod-presentation-assets.ts'),
    ])
    const store = await mod.loadModPresentationTextures(assets)
    const frame = {
      x: 3, y: 5, width: 17, height: 23, logicalWidth: 40, logicalHeight: 32,
      centerOffsetX: 2.5, centerOffsetY: -3.25, contentWidth: 15, contentHeight: 21,
    }
    const icon = {
      contentId: 'example.art:item', modId: 'example.art', key: 'item',
      icon: { imagePath: 'IMAGE-0.PNG', frame }, iconTrimImagePath: 'image-0.png',
    }
    const sprites = []
    for (const [role, texture] of [
      ['icon', store.texture(icon)], ['trim', store.iconTrim(icon)],
      ['sprite', store.spriteFrame('fixture', 'EXAMPLE.ART', 'image-0.png', frame)],
    ]) {
      for (const input of [
        { center: [100, 200], scale: [1, 1], anchor: [.5, .5], rotation: 0 },
        { center: [-5, 20], scale: [-1.5, .75], anchor: [0, 1], rotation: .72 },
      ]) {
        const sprite = new Sprite(texture)
        sprite.position.set(...input.center)
        sprite.scale.set(...input.scale)
        sprite.anchor.set(...input.anchor)
        sprite.rotation = input.rotation
        const bounds = sprite.visualBounds
        const matrix = sprite.getGlobalTransform()
        const corners = [
          [bounds.minX, bounds.minY], [bounds.maxX, bounds.minY],
          [bounds.maxX, bounds.maxY], [bounds.minX, bounds.maxY],
        ].map(([x, y]) => matrix.apply({ x, y }))
        sprites.push({ role, frame, input, corners })
        sprite.destroy()
      }
    }
    const cached = store.spriteFrame('fixture', 'example.art', 'image-0.png', { ...frame, x: 0 })
    if (cached.frame.x !== frame.x) throw new Error('Source frame cache changed its first admitted frame')
    const wearables = []
    for (const [slot, imageIndex] of [['hat', 1], ['robe', 2], ['staff', 3]]) {
      const content = { ...icon, contentId: `wearable:${slot}`, wearable: {
        slot, wornImagePath: `image-${imageIndex}.png`,
      } }
      const bank = store.wearable(content)
      for (const [heading, pose] of [[0, 0], [23, 99]]) {
        const texture = mod.modWearableFrame(bank, 'primary', heading, pose)
        const cell = texture.frame
        wearables.push({
          slot, width: texture.source.width, height: texture.source.height, heading, pose,
          rows: bank.primary[0].length, cell: [cell.x, cell.y, cell.width, cell.height],
          scaleMode: texture.source.style.scaleMode,
        })
      }
    }
    const failures = []
    for (const [key, wearable] of [
      ['columns', { slot: 'hat', wornImagePath: 'image-4.png' }],
      ['hat-rows', { slot: 'hat', wornImagePath: 'image-5.png' }],
      ['trim-rows', { slot: 'robe', wornImagePath: 'image-2.png', wornTrimImagePath: 'image-5.png' }],
    ]) {
      try {
        store.wearable({ ...icon, contentId: key, wearable })
        throw new Error('Invalid source wearable was admitted')
      } catch (error) {
        if (!error.message.startsWith('mod wearable')) throw error
        failures.push({ key, message: error.message })
      }
    }
    store.destroy()
    let destroyed = false
    try { store.texture(icon) } catch (error) {
      destroyed = error.message === 'mod presentation textures are destroyed'
    }
    if (!destroyed) throw new Error('Source retained destroyed custom textures')
    return { sprites, wearables, failures }
  }, assets)
})
writeFixture('mod-presentation-assets-goldens.json', 'tools/website-oracle/mod-presentation-assets-goldens.mjs',
  'Pinned Website renderer/mod-presentation-assets.ts and actual Pixi Sprite visual bounds', fixture)
