import assert from 'node:assert/strict'
import test from 'node:test'
import { checkForDesktopUpdate } from './updates.mjs'

const repository = 'https://github.com/JarrettOneSource/solomon-dark-website'
const current = { revision: 'a'.repeat(40), sourceTimestamp: 100 }
const newer = { revision: 'b'.repeat(40), sourceTimestamp: 200, schemaVersion: 1 }

function releaseRequest(manifest = newer, releaseOverrides = {}) {
  const base = `${repository}/releases/download/desktop-${manifest.revision}`
  const release = {
    tag_name: `desktop-${manifest.revision}`,
    draft: false,
    prerelease: false,
    assets: ['desktop-release.json', 'Solomon-Darker-win32-x64.zip'].map(name => ({ name, browser_download_url: `${base}/${name}` })),
    ...releaseOverrides,
  }
  return async url => new Response(JSON.stringify(url.endsWith('/latest') ? release : manifest))
}

test('only a newer compatible published package is offered, with a fixed repository download URL', async () => {
  const update = await checkForDesktopUpdate({ build: current, platform: 'win32', arch: 'x64', request: releaseRequest() })
  assert.deepEqual(update, {
    revision: newer.revision,
    url: `${repository}/releases/download/desktop-${newer.revision}/Solomon-Darker-win32-x64.zip`,
  })
  for (const manifest of [
    { ...newer, revision: current.revision },
    { ...newer, sourceTimestamp: 99 },
  ]) {
    assert.equal(await checkForDesktopUpdate({ build: current, platform: 'win32', arch: 'x64', request: releaseRequest(manifest) }), null)
  }
  assert.equal(await checkForDesktopUpdate({ build: current, platform: 'linux', arch: 'arm64', request: releaseRequest() }), null)
})

test('unpublished and malformed releases cannot become update prompts or external launch targets', async () => {
  for (const overrides of [
    { draft: true }, { prerelease: true }, { tag_name: 'other-product' },
    { assets: [{ name: 'desktop-release.json', browser_download_url: 'file:///etc/passwd' }] },
  ]) {
    assert.equal(await checkForDesktopUpdate({ build: current, platform: 'win32', arch: 'x64', request: releaseRequest(newer, overrides) }), null)
  }
  await assert.rejects(checkForDesktopUpdate({ build: current, platform: 'win32', arch: 'x64', request: async () => { throw new Error('offline') } }), /offline/)
  assert.equal(await checkForDesktopUpdate({ build: current, platform: 'win32', arch: 'x64', request: async () => new Response('', { status: 404 }) }), null)
})


test('the Rust package records its own release owner without trusting release-supplied URLs', async () => {
  const owner = 'ExampleOwner/solomon-darker-rust'
  const urls = []
  const base = `https://github.com/${owner}/releases/download/desktop-${newer.revision}`
  const request = async url => {
    urls.push(url)
    return new Response(JSON.stringify(url.endsWith('/latest') ? {
      tag_name: `desktop-${newer.revision}`, draft: false, prerelease: false,
      assets: ['desktop-release.json', 'Solomon-Darker-win32-x64.zip'].map(name => ({ name, browser_download_url: `${base}/${name}` })),
    } : newer))
  }
  const update = await checkForDesktopUpdate({ build: { ...current, releaseRepository: owner }, platform: 'win32', arch: 'x64', request })
  assert.equal(update.url, `${base}/Solomon-Darker-win32-x64.zip`)
  assert.equal(urls[0], `https://api.github.com/repos/${owner}/releases/latest`)
  await assert.rejects(checkForDesktopUpdate({ build: { ...current, releaseRepository: 'file:///outside' }, platform: 'win32', arch: 'x64', request }), /Invalid desktop release repository/)
})
