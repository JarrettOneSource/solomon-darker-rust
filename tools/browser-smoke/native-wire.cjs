// Read-only native Welcome/checkpoint observation. The real browser owns ACKs,
// validation and persistence. Transfer bounds/sequence rules follow network/transfer.rs.
const {inflateSync} = require('node:zlib');
const MAX_SAVE_BYTES = 96 * 1024 * 1024;
const MAX_FRAME_BYTES = MAX_SAVE_BYTES * 2 + 64 * 1024;
const CHUNK_CHARACTERS = 8 * 1024;

function applySnapshotChange(value, change) {
  if(Array.isArray(change)) {
    if(change.length===1)return change[0];
    const [length,items]=change;
    if(change.length!==2||!Array.isArray(value)||!Number.isSafeInteger(length)||length<0
      ||!Array.isArray(items)||length>value.length+items.length) {
      throw new Error('The observed snapshot array update is out of bounds');
    }
    const next=value.slice(0,length);
    let previous=-1;
    for(const item of items) {
      if(!Array.isArray(item)||item.length!==2)throw new Error('The observed snapshot item is invalid');
      const [index,entry]=item;
      if(!Number.isSafeInteger(index)||index<=previous||index>=length||index>next.length) {
        throw new Error('The observed snapshot array update has missing or unordered entries');
      }
      if(index===next.length&&(!Array.isArray(entry)||entry.length!==1)) {
        throw new Error('The observed snapshot array append is not a complete item');
      }
      next[index]=applySnapshotChange(next[index],entry);
      previous=index;
    }
    if(next.length!==length)throw new Error('The observed snapshot array is incomplete');
    return next;
  }
  if(!change||typeof change!=='object'||!value||typeof value!=='object'||Array.isArray(value)) {
    throw new Error('The observed snapshot field update has no object');
  }
  const next={...value};
  for(const [key,entry] of Object.entries(change)) {
    if(entry===null) {
      if(!Object.hasOwn(next,key))throw new Error('The observed snapshot removes an absent field');
      delete next[key];
    } else {
      const updated=applySnapshotChange(Object.hasOwn(next,key)?next[key]:undefined,entry);
      Object.defineProperty(next,key,{value:updated,enumerable:true,writable:true,configurable:true});
    }
  }
  return next;
}

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
  let previousFrame = null;
  return function read(message) {
    if(message.type==='snapshot') {
      previousFrame=message.frame;
      return message;
    }
    if(message.type==='snapshot_delta') {
      if(!previousFrame||!Number.isSafeInteger(message.sequence)||!Number.isSafeInteger(message.base_sequence)
        ||message.base_sequence!==previousFrame.sequence||message.sequence<=message.base_sequence) {
        throw new Error('The observed snapshot update does not follow its baseline');
      }
      const compressed=Buffer.from(message.changes,'base64');
      if(compressed.toString('base64')!==message.changes)throw new Error('The observed snapshot compression is malformed');
      const bytes=inflateSync(compressed,{maxOutputLength:MAX_FRAME_BYTES});
      const change=JSON.parse(new TextDecoder('utf-8',{fatal:true}).decode(bytes));
      const frame=applySnapshotChange(previousFrame,change);
      if(frame.sequence!==message.sequence||Buffer.byteLength(JSON.stringify(frame))>MAX_FRAME_BYTES) {
        throw new Error('The observed snapshot update changed its sequence or exceeds its byte limit');
      }
      previousFrame=frame;
      return {type:'snapshot',frame};
    }
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
