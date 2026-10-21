// Actual keyboard journeys through each room, using the pinned Website geometry to choose
// courtyard waypoints. Run with Node's --experimental-strip-types, PW and URL like smoke.cjs.
import { createRequire } from 'node:module'
import { mkdirSync, writeFileSync } from 'node:fs'
import { resolve } from 'node:path'
import { loadKernel } from '../website-oracle/oracle.mjs'

const require = createRequire(import.meta.url)
const { chromium } = require(resolve(process.env.PW))
const { startNewGame } = require('./menu-journey.cjs')
const regions = await loadKernel('core-kernels/hub-regions.ts')
const url = process.env.URL || 'http://127.0.0.1:4185/'
const output = resolve(process.env.SHOTS || 'dialogue-room-smoke')
mkdirSync(output, { recursive: true })
const receipt = { url, journeys: [], errors: [] }
const browser = await chromium.launch({
  headless: true,
  executablePath: process.env.CHROME || '/Applications/Google Chrome.app/Contents/MacOS/Google Chrome',
  args: ['--use-angle=swiftshader', '--enable-unsafe-swiftshader', '--ignore-gpu-blocklist'],
})
const page = await browser.newPage({ viewport: { width: 1600, height: 900 } })
page.on('pageerror', error => receipt.errors.push(error.message))
page.on('console', message => {
  if (message.type() === 'error') receipt.errors.push(message.text())
})
page.on('response', response => {
  if (response.status() >= 400) receipt.errors.push(`${response.status()} ${response.url()}`)
})
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

async function walkTo(target, expectedRegion = 0) {
  const deadline = Date.now() + 30000
  while (Date.now() < deadline) {
    const [x,y,,, , ,region] = await state()
    if (region !== expectedRegion) throw new Error(`unexpected region ${region} en route to ${JSON.stringify(target)}`)
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
  await page.waitForFunction(() => window.wasmBindings?.wizard_state().length > 0
    && document.getElementById('status').textContent === '', null, { timeout: 60000 })
  await startNewGame(page)
  let [x,y] = await state()
  for(const point of courtyardPath({x,y},{x:620,y:745})) await walkTo(point)
  // The real pointer path projects a canvas point back through the current camera.
  ;[x,y] = await state()
  const originX=Math.max(0,Math.min(2000-1600/1.2,x-800/1.2))
  const originY=Math.max(0,Math.min(1024-900/1.2,y-450/1.2))
  await page.mouse.click((576.5-originX)*1.2,(710.5-originY)*1.2)
  await page.getByRole('dialog',{name:'Professor Machinimbus',exact:true}).waitFor()
  await page.locator('[data-panel-action="dialogue-next"]').click()
  await page.getByRole('button',{name:'Per$uade',exact:true}).click()
  await page.getByRole('dialog',{name:'SELECT A SPELL',exact:true}).waitFor()
  await page.waitForTimeout(450)
  await page.screenshot({path:`${output}/selector-teacher-rust.png`})
  if(await page.locator('[data-panel-action^="dialogue-selector:"]').count()!==8) throw new Error('teacher lost its eight locked spells')
  await page.locator('[data-panel-action="dialogue-selector:79"]').focus()
  await page.waitForTimeout(200)
  const visible=await page.locator('[data-panel-action="dialogue-selector:79"]').evaluate(node=>{
    const row=node.getBoundingClientRect(),clip=document.querySelector('.panel-selector-viewport').getBoundingClientRect()
    return row.top>=clip.top-1&&row.bottom<=clip.bottom+1
  })
  if(!visible) throw new Error('selector controller focus did not reveal a clipped teacher row')
  receipt.journeys.push({interaction:'teacher',state:await state(),rows:8,focusedLastRow:true})
  await page.locator('[data-panel-action="dialogue-selector:79"]').click()
  await page.waitForTimeout(200)
  if(await page.locator('[data-panel-action^="dialogue-selector:"]').count()!==8) throw new Error('rejected teacher purchase unlocked a spell')
  await page.keyboard.press('Escape')
  await page.keyboard.press('Escape')
  await page.locator('.player-panels').waitFor({state:'hidden'})
  ;[x,y] = await state()
  for(const point of courtyardPath({x,y},{x:1800,y:650})) await walkTo(point)
  await through(['KeyD','KeyW'],2)
  await walkTo({x:512,y:700},2)
  await page.keyboard.press('KeyE')
  await page.getByRole('dialog',{name:'Professor Semicus',exact:true}).waitFor()
  await page.locator('[data-panel-action="dialogue-next"]').click()
  await page.getByRole('button',{name:'Inquire about Books',exact:true}).click()
  await page.getByRole('dialog',{name:'SELECT A BOOK',exact:true}).waitFor()
  await page.waitForTimeout(450)
  await page.screenshot({path:`${output}/selector-books-rust.png`})
  if(await page.locator('[data-panel-action^="dialogue-selector:"]').count()!==26) throw new Error('library catalog lost a native book')
  await page.locator('[data-panel-action="dialogue-selector:25"]').focus()
  await page.waitForTimeout(200)
  await page.screenshot({path:`${output}/selector-books-lace-rust.png`})
  await page.locator('[data-panel-action="dialogue-selector:25"]').click()
  await page.locator('[data-dialogue-key="BOOK25_LACE"]').waitFor()
  await page.locator('[data-panel-action="dialogue-next"]').click()
  await page.getByRole('button',{name:'Inquire about Books',exact:true}).click()
  if(await page.locator('[data-panel-action="dialogue-selector:25"]').count()!==0) throw new Error('one-shot Lace book reappeared after the accepted canonical action')
  receipt.journeys.push({interaction:'librarian',state:await state(),rowsBefore:26,oneShotLaceRemoved:true})
  if(receipt.errors.length) throw new Error(receipt.errors.join('; '))
  receipt.passed=true
} catch(error) {
  receipt.passed=false
  receipt.failure=error.stack
  receipt.state=await state().catch(()=>[])
  await page.screenshot({path:`${output}/failure.png`})
  process.exitCode=1
} finally {
  await keys([])
  writeFileSync(`${output}/receipt.json`,`${JSON.stringify(receipt,null,2)}\n`)
  await browser.close()
}
console.log(JSON.stringify(receipt))
