import assert from 'node:assert/strict'
import test from 'node:test'

import {
  PINNED_NODE_VERSION,
  nodeArchiveDescriptor,
} from './stage-node-runtime.mjs'

test('runtime archive mapping is explicit and rejects unsupported targets', () => {
  assert.deepEqual(nodeArchiveDescriptor('linux', 'x64'), {
    archive: `node-v${PINNED_NODE_VERSION}-linux-x64.tar.xz`,
    executable: `node-v${PINNED_NODE_VERSION}-linux-x64/bin/node`,
    extractor: 'tar-xz',
    sha256: '325c0f1261e0c61bcae369a1274028e9cfb7ab7949c05512c5b1e630f7e80e12',
  })
  assert.equal(
    nodeArchiveDescriptor('win32', 'x64').executable,
    `node-v${PINNED_NODE_VERSION}-win-x64/node.exe`,
  )
  assert.throws(() => nodeArchiveDescriptor('win32', 'arm64'), /Unsupported/)
})

test('every supported runtime target pins an immutable SHA-256', () => {
  for (const [platform, arch] of [
    ['linux', 'arm64'],
    ['darwin', 'x64'],
    ['darwin', 'arm64'],
    ['win32', 'x64'],
  ]) {
    assert.match(nodeArchiveDescriptor(platform, arch).sha256, /^[a-f0-9]{64}$/)
  }
})
