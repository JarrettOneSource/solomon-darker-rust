// Read-only native Welcome/checkpoint observation. The real browser owns ACKs,
// validation and persistence. Transfer bounds/sequence rules follow network/transfer.rs.
const MAX_SAVE_BYTES = 96 * 1024 * 1024;
const MAX_FRAME_BYTES = MAX_SAVE_BYTES * 2 + 64 * 1024;
const CHUNK_CHARACTERS = 8 * 1024;

function createStringReader(label, maximumBytes) {
  let chunks = [];
  let offset = 0;
  let total = null;
  function close() {
    chunks = [];
    offset = 0;
    total = null;
  }
  return {
    close,
    accept(chunk) {
      if (!Number.isSafeInteger(chunk.offset) || chunk.offset < 0
        || !Number.isSafeInteger(chunk.total_length) || chunk.total_length <= 0
        || chunk.total_length > maximumBytes || !Array.isArray(chunk.data)
        || chunk.data.length === 0 || chunk.data.length > CHUNK_CHARACTERS
        || !chunk.data.every(unit => Number.isInteger(unit) && unit >= 0 && unit <= 65535)
        || chunk.offset !== offset || (total !== null && total !== chunk.total_length)
        || chunk.offset + chunk.data.length > chunk.total_length) {
        throw new Error(`The observed ${label} chunks are out of order or bounds`);
      }
      total = chunk.total_length;
      // Decode only after concatenation: a surrogate pair may cross chunks.
      const bytes = Buffer.allocUnsafe(chunk.data.length * 2);
      for (let index = 0; index < chunk.data.length; index++) {
        bytes.writeUInt16LE(chunk.data[index], index * 2);
      }
      chunks.push(bytes);
      offset += chunk.data.length;
      if (offset !== total) return null;
      const text = Buffer.concat(chunks).toString('utf16le');
      close();
      if (Buffer.byteLength(text, 'utf8') > maximumBytes) {
        throw new Error(`The observed ${label} exceeds its transfer byte limit`);
      }
      return text;
    },
  };
}

function createNativeMessageReader() {
  const welcome = createStringReader('Welcome', MAX_FRAME_BYTES);
  const saves = createStringReader('checkpoint', MAX_SAVE_BYTES);
  let completedSequence = 0;
  let pendingSave = null;
  return function read(message) {
    if (message.type === 'welcome_chunk') {
      const text = welcome.accept(message);
      if (text === null) return null;
      const complete = JSON.parse(text);
      if (complete.type !== 'welcome') throw new Error('The observed Welcome contains another message');
      return complete;
    }
    if (message.type !== 'hosted') return message;
    const event = message.message;
    if (event.kind === 'save_checkpoint') {
      if (event.sequence > completedSequence) {
        completedSequence = event.sequence;
        if (pendingSave && pendingSave.sequence <= event.sequence) {
          saves.close();
          pendingSave = null;
        }
      }
      return message;
    }
    if (event.kind !== 'save_checkpoint_chunk') return message;
    if (!Number.isSafeInteger(event.sequence) || event.sequence < 0) {
      throw new Error('The observed checkpoint sequence is invalid');
    }
    if (event.sequence <= completedSequence
      || (pendingSave && event.sequence < pendingSave.sequence)) return null;
    if (!pendingSave || event.sequence > pendingSave.sequence) {
      if (event.offset !== 0) throw new Error('The observed checkpoint is missing its first chunk');
      saves.close();
      pendingSave = {sequence: event.sequence, reason: event.reason, total: event.total_length};
    }
    if (event.reason !== pendingSave.reason || event.total_length !== pendingSave.total) {
      throw new Error('The observed checkpoint chunk does not match its pending transfer');
    }
    const save = saves.accept(event);
    if (save === null) return null;
    completedSequence = event.sequence;
    pendingSave = null;
    return {type: 'hosted', message: {kind: 'save_checkpoint',
      save, reason: event.reason, sequence: event.sequence}};
  };
}

module.exports = { createNativeMessageReader };
