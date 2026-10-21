import { execFile } from 'node:child_process'
import { readFile } from 'node:fs/promises'
import { dirname, resolve } from 'node:path'
import { fileURLToPath } from 'node:url'
import { promisify } from 'node:util'

import {
  assetSourceRoot,
  fetchAssetSource,
  readAssetSources,
} from './assets.mjs'

const execFileAsync = promisify(execFile)
const BLENDER_SCRIPT = fileURLToPath(new URL('./render-directional-sprite.py', import.meta.url))

export async function renderSprite(args, io = console) {
  const [recipeArg] = args
  if (!recipeArg) throw new Error('usage: sdmod render-sprite <recipe.json>')
  const recipePath = resolve(recipeArg)
  const recipe = JSON.parse(await readFile(recipePath, 'utf8'))
  const catalog = await readAssetSources()
  const source = catalog.sources.find(candidate => candidate.id === recipe.source)
  if (!source) throw new Error(`sprite recipe uses unknown asset source: ${recipe.source}`)
  await fetchAssetSource(source)
  const output = resolve(dirname(recipePath), recipe.output)
  if (!output.endsWith('.png')) throw new Error('sprite recipe output must be a PNG path')
  const blender = process.env.SDR_BLENDER_PATH || 'blender'
  const { stdout, stderr } = await execFileAsync(blender, [
    '--background',
    '--factory-startup',
    '--python', BLENDER_SCRIPT,
    '--',
    '--recipe', recipePath,
    '--source-root', assetSourceRoot(source.id),
    '--output', output,
  ], { maxBuffer: 32 * 1024 * 1024 })
  const rendered = stdout.split('\n').find(line => line.startsWith('SDR_SPRITE_RESULT='))
  if (!rendered) {
    throw new Error(`Blender sprite render failed:\n${stdout}${stderr}`.trim())
  }
  io.log(output)
  return output
}
