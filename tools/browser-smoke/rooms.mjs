// Actual keyboard journeys through each room, using the pinned Website geometry to choose
// courtyard waypoints. Run with Node's --experimental-strip-types, PW and URL like smoke.cjs.
import { createRequire } from 'node:module'
import { mkdirSync, writeFileSync } from 'node:fs'
import { resolve } from 'node:path'
import { loadKernel } from '../website-oracle/oracle.mjs'

const require = createRequire(import.meta.url)
const { chromium } = require(resolve(process.env.PW))
const { startNewGame } = require('./menu-journey.cjs')
const ui = require('./gpu-ui.cjs')
const regions = await loadKernel('core-kernels/hub-regions.ts')
const url = process.env.URL || 'http://127.0.0.1:4183/'
const output = resolve(process.env.SHOTS || 'room-smoke')
mkdirSync(output, { recursive: true })
const receipt = { url, journeys: [], errors: [] }
const browser = await chromium.launch({
  headless: true,
  executablePath: process.env.CHROME || '/Applications/Google Chrome.app/Contents/MacOS/Google Chrome',
  args: ['--use-angle=swiftshader', '--enable-unsafe-swiftshader', '--ignore-gpu-blocklist'],
})
const page = await browser.newPage({ viewport: { width: 1600, height: 900 } })
ui.watchErrors(page, { errors: receipt.errors })
const state = () => page.evaluate(() => Array.from(window.wasmBindings.wizard_state()))
const held = new Set()

async function keys(desired) {
  for (const key of held) {
    if (!desired.includes(key)) {
      await page.keyboard.up(key)
      held.delete(key)
    }
  }
  for (const key of desired) {
    if (!held.has(key)) {
      await page.keyboard.down(key)
      held.add(key)
    }
  }
}

function clearPath(start, end) {
  return regions.isHubRegionPathTraversable('courtyard', start, end, 25.1)
}

// A small breadth-first grid finds a route; visibility pruning leaves just the corners.
// Portal triggers are excluded so a route cannot enter an unintended room along the way.
function courtyardPath(start, target) {
  const step = 10
  const gridPoint = (x,y) => ({ x:x * step, y:y * step })
  const startX = Math.round(start.x / step)
  const startY = Math.round(start.y / step)
  const queue = [[startX, startY]]
  const parents = new Map([[`${startX},${startY}`, null]])
  let found = null
  for (let cursor = 0; cursor < queue.length; cursor += 1) {
    const [x,y] = queue[cursor]
    const position = gridPoint(x,y)
    if (Math.hypot(position.x - target.x, position.y - target.y) < 35 && clearPath(position,target)) {
      found = `${x},${y}`
      break
    }
    for (const [dx,dy] of [[1,0],[-1,0],[0,1],[0,-1],[1,1],[-1,1],[1,-1],[-1,-1]]) {
      const nx = x + dx
      const ny = y + dy
      const key = `${nx},${ny}`
      if (parents.has(key) || nx < 1 || nx > 199 || ny < 1 || ny > 101) continue
      const next = gridPoint(nx,ny)
      if (!regions.isHubRegionTraversable('courtyard',next,25.1)) continue
      if (regions.hubPortalAt('courtyard',next,25)) continue
      if (!clearPath(position,next)) continue
      parents.set(key,`${x},${y}`)
      queue.push([nx,ny])
    }
  }
  if (!found) throw new Error(`no courtyard route from ${JSON.stringify(start)} to ${JSON.stringify(target)} (${queue.length} reachable cells)`)
  const path = [target]
  for (let key = found; key !== null; key = parents.get(key)) {
    const [x,y] = key.split(',').map(Number)
    path.push(gridPoint(x,y))
  }
  path.push(start)
  path.reverse()
  const corners = []
  let cursor = 0
  while (cursor < path.length - 1) {
    let next = cursor + 1
    for (let index = next + 1; index < path.length; index += 1) {
      if (clearPath(path[cursor], path[index])) next = index
    }
    corners.push(path[next])
    cursor = next
  }
  return corners
}

async function walkTo(target) {
  const deadline = Date.now() + 30000
  while (Date.now() < deadline) {
    const [x,y,,, , ,region] = await state()
    if (region !== 0) throw new Error(`unexpected region ${region} en route to ${JSON.stringify(target)}`)
    const dx = target.x - x
    const dy = target.y - y
    if (Math.hypot(dx,dy) < 7) {
      await keys([])
      await page.waitForTimeout(120)
      return
    }
    const direction = []
    if (Math.abs(dx) > Math.abs(dy) * 0.45) direction.push(dx > 0 ? 'KeyD' : 'KeyA')
    if (Math.abs(dy) > Math.abs(dx) * 0.45) direction.push(dy > 0 ? 'KeyS' : 'KeyW')
    await keys(direction)
    await page.waitForTimeout(40)
  }
  throw new Error(`walk timed out toward ${JSON.stringify(target)} at ${(await state()).slice(0,2)}`)
}

async function through(direction, expectedRegion) {
  await keys(direction)
  try {
    await page.waitForFunction(region => window.wasmBindings.wizard_state()[6] === region,
      expectedRegion, { timeout: 20000 })
  } finally {
    await keys([])
  }
  await page.waitForFunction(region => {
    const state = window.wasmBindings.wizard_state()
    return state[6] === region && state[7] === -1
  }, expectedRegion, { timeout: 10000 })
}

try {
  await page.goto(url, { waitUntil: 'load' })
  await ui.ready(page)
  await startNewGame(page)
  const rooms = [
    { name:'mortuary', id:1, approach:{x:260,y:610}, keys:['KeyA','KeyW'] },
    { name:'library', id:2, approach:{x:1800,y:650}, keys:['KeyD','KeyW'] },
    { name:'storeroom', id:3, approach:{x:628,y:230}, keys:['KeyW'] },
    { name:'office', id:4, approach:{x:950.64,y:164.04}, keys:['KeyW'] },
  ]
  for (const room of rooms) {
    if (process.env.ROOM && process.env.ROOM !== room.name) continue
    const [x,y] = await state()
    const path = courtyardPath({x,y},room.approach)
    console.log(`${room.name}: courtyard route ${JSON.stringify(path)}`)
    for (const point of path) await walkTo(point)
    await through(room.keys,room.id)
    const entered = await state()
    const expectedEntry = regions.hubIncomingPlacement('courtyard',room.name).scriptedTarget
    if (Math.hypot(entered[0] - expectedEntry.x, entered[1] - expectedEntry.y) > 0.01) {
      throw new Error(`${room.name} entrance stopped at the wrong placement`)
    }
    const screenshot = `${output}/${room.name}.png`
    await page.screenshot({ path:screenshot })
    await through(['KeyS'],0)
    const returned = await state()
    const expectedReturn = regions.hubIncomingPlacement(room.name,'courtyard').scriptedTarget
    if (Math.hypot(returned[0] - expectedReturn.x, returned[1] - expectedReturn.y) > 0.01) {
      throw new Error(`${room.name} return stopped at the wrong placement`)
    }
    await page.screenshot({ path:`${output}/${room.name}-returned.png` })
    receipt.journeys.push({ room:room.name, entered, returned, screenshot, path })
    writeFileSync(`${output}/receipt.json`,`${JSON.stringify(receipt,null,2)}\n`)
    console.log(`${room.name}: entered and returned`)
  }
  await ui.assertGpuOnly(page)
  await ui.settleErrors(page)
  if (receipt.errors.length) throw new Error(`browser errors: ${JSON.stringify(receipt.errors)}`)
  receipt.passed = true
} catch (error) {
  receipt.passed = false
  receipt.failure = error.stack
  receipt.state = await state().catch(() => [])
  await page.screenshot({ path:`${output}/failure.png` })
  process.exitCode = 1
} finally {
  await keys([])
  writeFileSync(`${output}/receipt.json`,`${JSON.stringify(receipt,null,2)}\n`)
  await browser.close()
}
console.log(JSON.stringify(receipt))
