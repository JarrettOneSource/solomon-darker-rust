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
