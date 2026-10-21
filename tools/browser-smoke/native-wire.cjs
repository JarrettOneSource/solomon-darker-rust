// Read-only observation of the native protocol's UTF-16 Welcome prefix. The real
// browser remains responsible for ACKs and admission; this helper never writes.
function createNativeMessageReader() {
  let chunks = [];
  let offset = 0;
  let total = null;
  return function read(message) {
    if (message.type !== 'welcome_chunk') return message;
    if (!Number.isSafeInteger(message.offset) || !Number.isSafeInteger(message.total_length)
      || !Array.isArray(message.data) || message.offset !== offset
      || (total !== null && total !== message.total_length)
      || message.offset + message.data.length > message.total_length) {
      throw new Error('The observed Welcome chunks are out of order');
    }
    total = message.total_length;
    // Node's standard UTF-16 decoder also preserves pairs split between chunks.
    const bytes = Buffer.allocUnsafe(message.data.length * 2);
    for (let index = 0; index < message.data.length; index++) {
      bytes.writeUInt16LE(message.data[index], index * 2);
    }
    chunks.push(bytes);
    offset += message.data.length;
    if (offset !== total) return null;
    const complete = JSON.parse(Buffer.concat(chunks).toString('utf16le'));
    chunks = [];
    offset = 0;
    total = null;
    return complete;
  };
}

module.exports = { createNativeMessageReader };
