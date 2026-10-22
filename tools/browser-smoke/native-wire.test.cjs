const assert = require('node:assert/strict');
const test = require('node:test');
const { createNativeMessageReader } = require('./native-wire.cjs');

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
