// The unchanged editor's complete lossless document, before its asset hydration.
import {createHash} from 'node:crypto'
import {readFileSync} from 'node:fs'
import {resolve} from 'node:path'
import {loadKernel, readFixture, websiteDir, writeFixture} from './oracle.mjs'
const parserPath = resolve(websiteDir, 'frontend/src/editor/format/boneyard.ts')
const parserSha256 = createHash('sha256').update(readFileSync(parserPath)).digest('hex')
if (parserSha256 !== '39a1fe9a8708b876ff777b993192bda4bd961a3eaf0493f66e80e533dbf42243') {
  throw new Error('The editor parser must match the retained fa08 source before recording.')
}
const {parseBoneyard} = await loadKernel('../editor/format/boneyard.ts')
const cases = readFixture('boneyard-binary-goldens.json').cases.map(({name, bytes}) => {
  try {return {name, bytes, document: parseBoneyard(new Uint8Array(Buffer.from(bytes, 'base64')))}}
  catch (error) {return {name, bytes, error: error.message}}
})
writeFixture('editor-document-goldens.json.zlib', 'editor-document-goldens.mjs',
  'Website fa08ae249ff5563570bdb2a653e2ef9358ce7b83 frontend/src/editor/format/boneyard.ts',
  {parserSha256, cases})
