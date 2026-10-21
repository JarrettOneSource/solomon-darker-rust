// Browser-only source views need Pixi, image decode and shader construction. Keep their
// Vite cache outside the pinned checkout and retire both owned processes after recording.
import { resolve } from 'node:path'
import { pathToFileURL } from 'node:url'
import { websiteDir } from './oracle.mjs'

export async function withSourceOraclePage(record) {
  const { createServer } = await import(pathToFileURL(resolve(
    websiteDir, 'frontend/node_modules/vite/dist/node/index.js',
  )).href)
  const { chromium } = await import(pathToFileURL(resolve(
    websiteDir, 'frontend/node_modules/playwright-core/index.mjs',
  )).href)
  const server = await createServer({
    root: resolve(websiteDir, 'frontend'),
    configFile: resolve(websiteDir, 'frontend/vite.config.ts'),
    cacheDir: '/Users/jarrett/codex-runs/solomon-rust-parity-20261021/visuals-vite-cache',
    server: { host: '127.0.0.1', port: 4178, strictPort: true, open: false },
  })
  let browser
  try {
    await server.listen()
    browser = await chromium.launch({
      executablePath: '/Applications/Google Chrome.app/Contents/MacOS/Google Chrome',
      headless: true,
      args: ['--use-angle=metal'],
    })
    const page = await browser.newPage({ viewport: { width: 640, height: 360 } })
    await page.route('**/__view_oracle', route => route.fulfill({
      contentType: 'text/html', body: '<!doctype html><title>Source view oracle</title>',
    }))
    await page.goto('http://127.0.0.1:4178/__view_oracle')
    return await record(page)
  } finally {
    await browser?.close()
    await server.close()
  }
}
