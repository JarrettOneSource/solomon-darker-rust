import { inflateRawSync } from 'node:zlib'

const CENTRAL_HEADER = 0x02014b50
const END_HEADER = 0x06054b50
const LOCAL_HEADER = 0x04034b50

export function readZipEntries(bytes, requestedPaths) {
  const archive = Buffer.from(bytes)
  const requested = new Set(requestedPaths)
  const end = findEndHeader(archive)
  const count = archive.readUInt16LE(end + 10)
  let cursor = archive.readUInt32LE(end + 16)
  const result = new Map()

  for (let index = 0; index < count; index += 1) {
    if (archive.readUInt32LE(cursor) !== CENTRAL_HEADER) throw new Error('ZIP central directory is invalid')
    const method = archive.readUInt16LE(cursor + 10)
    const expectedCrc = archive.readUInt32LE(cursor + 16)
    const compressedSize = archive.readUInt32LE(cursor + 20)
    const size = archive.readUInt32LE(cursor + 24)
    const nameLength = archive.readUInt16LE(cursor + 28)
    const extraLength = archive.readUInt16LE(cursor + 30)
    const commentLength = archive.readUInt16LE(cursor + 32)
    const localOffset = archive.readUInt32LE(cursor + 42)
    const path = archive.subarray(cursor + 46, cursor + 46 + nameLength).toString('utf8')
    cursor += 46 + nameLength + extraLength + commentLength

    if (!requested.has(path)) continue
    if (archive.readUInt32LE(localOffset) !== LOCAL_HEADER) throw new Error(`ZIP entry is invalid: ${path}`)
    const localNameLength = archive.readUInt16LE(localOffset + 26)
    const localExtraLength = archive.readUInt16LE(localOffset + 28)
    const start = localOffset + 30 + localNameLength + localExtraLength
    const compressed = archive.subarray(start, start + compressedSize)
    const value = method === 0
      ? Buffer.from(compressed)
      : method === 8
        ? inflateRawSync(compressed)
        : (() => { throw new Error(`ZIP entry uses unsupported compression ${method}: ${path}`) })()
    if (value.length !== size || crc32(value) !== expectedCrc) throw new Error(`ZIP entry checksum failed: ${path}`)
    result.set(path, value)
  }

  const missing = [...requested].filter(path => !result.has(path))
  if (missing.length > 0) throw new Error(`ZIP archive is missing: ${missing.join(', ')}`)
  return result
}

function findEndHeader(bytes) {
  const minimum = Math.max(0, bytes.length - 65_557)
  for (let cursor = bytes.length - 22; cursor >= minimum; cursor -= 1) {
    if (bytes.readUInt32LE(cursor) === END_HEADER) return cursor
  }
  throw new Error('ZIP end record is missing')
}

const CRC_TABLE = Array.from({ length: 256 }, (_, value) => {
  let crc = value
  for (let bit = 0; bit < 8; bit += 1) crc = (crc & 1) ? 0xedb88320 ^ (crc >>> 1) : crc >>> 1
  return crc >>> 0
})

function crc32(bytes) {
  let crc = 0xffffffff
  for (const byte of bytes) crc = CRC_TABLE[(crc ^ byte) & 0xff] ^ (crc >>> 8)
  return (crc ^ 0xffffffff) >>> 0
}
