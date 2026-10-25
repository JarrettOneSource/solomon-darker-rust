const assert = require('node:assert/strict');
const test = require('node:test');
const { createNativeMessageReader } = require('./native-wire.cjs');
const {deflateSync} = require('node:zlib');

function delta(baseSequence, sequence, changes) {
  return {type:'snapshot_delta', base_sequence:baseSequence, sequence,
    changes:deflateSync(JSON.stringify(changes)).toString('base64')};
}

test('compact observations reconstruct the written baseline without mutating earlier frames', () => {
  const read=createNativeMessageReader();
  const first={type:'snapshot',frame:{sequence:1,snapshot:{actors:[{x:1},{x:2}],name:'W🧙'}}};
  assert.equal(read(first),first);
  const next=read(delta(1,3,{sequence:[3],snapshot:{actors:[2,[[0,{x:[4]}],[1,[{x:5}]]]],name:[null]}}));
  assert.deepEqual(next,{type:'snapshot',frame:{sequence:3,snapshot:{actors:[{x:4},{x:5}],name:null}}});
  assert.equal(first.frame.snapshot.actors[0].x,1);
  assert.equal(first.frame.snapshot.name,'W🧙');
  const last=read(delta(3,4,{sequence:[4],snapshot:{actors:[0,[]],name:null,added:[{value:'new'}]}}));
  assert.deepEqual(last.frame.snapshot,{actors:[],added:{value:'new'}});
  assert.throws(()=>createNativeMessageReader()(delta(1,2,{sequence:[2]})),/baseline/);
  assert.throws(()=>read(delta(3,5,{sequence:[5]})),/baseline/);
});

test('compact observation bounds reject missing array entries and malformed compressed data', () => {
  for(const packet of [delta(1,2,{sequence:[2],snapshot:{actors:[1e9,[]]}}),
    delta(1,2,{sequence:[2],snapshot:{actors:[2,[[1,[7]]]]}}),
    {...delta(1,2,{sequence:[2]}),changes:'invalid'}]) {
    const read=createNativeMessageReader();
    read({type:'snapshot',frame:{sequence:1,snapshot:{actors:[]}}});
    assert.throws(()=>read(packet));
  }
});

test('the read-only observer admits ordinary and streamed native welcomes identically', () => {
  const message = {type: 'welcome', credentials: {player_id: 'player-2'},
    frame: {name: 'W🧙', gold: 4321}};
  const text = JSON.stringify(message);
  const split = text.indexOf('🧙') + 1;
  const first = Array.from({length: split}, (_, index) => text.charCodeAt(index));
  const rest = Array.from({length: text.length - split}, (_, index) => text.charCodeAt(split + index));
  const read = createNativeMessageReader();
  assert.equal(read(message), message);
  assert.equal(read({type: 'welcome_chunk', offset: 0, total_length: text.length, data: first}), null);
  assert.deepEqual(read({type: 'welcome_chunk', offset: split, total_length: text.length, data: rest}), message);
  const snapshot = {type: 'snapshot', frame: {ticks: 100}};
  assert.equal(read(snapshot), snapshot);
});

test('the observer does not silently accept a missing prefix or changing transfer length', () => {
  const read = createNativeMessageReader();
  assert.throws(() => read({type: 'welcome_chunk', offset: 1, total_length: 3, data: [123]}), /out of order/);
  assert.equal(read({type: 'welcome_chunk', offset: 0, total_length: 3, data: [123]}), null);
  assert.throws(() => read({type: 'welcome_chunk', offset: 1, total_length: 4, data: [125]}), /out of order/);
});

function checkpoint(sequence, offset, totalLength, text, reason = 'progress') {
  return {type: 'hosted', message: {kind: 'save_checkpoint_chunk', sequence, reason,
    offset, total_length: totalLength,
    data: Array.from({length: text.length}, (_, index) => text.charCodeAt(index))}};
}

test('checkpoint chunks expose only the complete original save string, including split pairs', () => {
  const read = createNativeMessageReader();
  const save = '{"name":"W🧙"}';
  const split = save.indexOf('🧙') + 1;
  assert.equal(read(checkpoint(1, 0, save.length, save.slice(0, split))), null);
  const snapshot = {type: 'snapshot', frame: {ticks: 100}};
  assert.equal(read(snapshot), snapshot);
  assert.deepEqual(read(checkpoint(1, split, save.length, save.slice(split))),
    {type: 'hosted', message: {kind: 'save_checkpoint', sequence: 1,
      reason: 'progress', save}});
  assert.equal(read(checkpoint(1, 0, save.length, save)), null);
  assert.deepEqual(read(checkpoint(2, 0, 3, 'new', 'game-over')),
    {type: 'hosted', message: {kind: 'save_checkpoint', sequence: 2,
      reason: 'game-over', save: 'new'}});
});

test('newer atomic and streamed saves replace older incomplete transfers like the client', () => {
  const read = createNativeMessageReader();
  assert.equal(read(checkpoint(1, 0, 10, 'hello')), null);
  const atomic = {type: 'hosted', message: {kind: 'save_checkpoint', sequence: 2,
    reason: 'game-over', save: 'final'}};
  assert.equal(read(atomic), atomic);
  assert.equal(read(checkpoint(1, 5, 10, 'world')), null);
  assert.equal(read(checkpoint(3, 0, 6, 'old')), null);
  assert.equal(read(checkpoint(4, 0, 6, 'new')), null);
  assert.equal(read(checkpoint(3, 3, 6, 'old')), null);
  assert.deepEqual(read(checkpoint(4, 3, 6, 'est')),
    {type: 'hosted', message: {kind: 'save_checkpoint', sequence: 4,
      reason: 'progress', save: 'newest'}});
});

test('checkpoint prefixes, bounds and metadata must match the actual transfer', () => {
  for (const malformed of [
    checkpoint(1, 1, 3, 'x'),
    checkpoint(1, 0, 0, ''),
    checkpoint(1, 0, 3, ''),
    checkpoint(1, 0, 2, 'abc'),
    {...checkpoint(1, 0, 1, 'x'), message: {...checkpoint(1, 0, 1, 'x').message, data: [65536]}},
  ]) assert.throws(() => createNativeMessageReader()(malformed));
  const read = createNativeMessageReader();
  assert.equal(read(checkpoint(0, 0, 1, 'x')), null);
  assert.equal(read(checkpoint(1, 0, 4, 'ab')), null);
  assert.throws(() => read(checkpoint(1, 2, 4, 'cd', 'game-over')), /pending transfer/);
  assert.throws(() => read(checkpoint(1, 2, 5, 'cd')), /pending transfer/);
  assert.throws(() => read(checkpoint(2, 2, 4, 'cd')), /first chunk/);
});
