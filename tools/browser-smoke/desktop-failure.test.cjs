const assert = require('node:assert/strict');
const {spawnSync} = require('node:child_process');
const {join} = require('node:path');
const {test} = require('node:test');

test('a fatal desktop failure retains context without exposing invitation stacks or metadata', () => {
  const helper = join(__dirname, 'desktop-failure.cjs');
  const result = spawnSync(process.execPath, ['-e', `
    const assert = require('node:assert/strict');
    const {redact, sanitizeError} = require(${JSON.stringify(helper)});
    const invite = 'solomondarker://fixture-private-capability';
    function originalCaller() { assert.equal(invite, 'a different invitation'); }
    try { originalCaller(); }
    catch (error) {
      error.cause = new Error(invite);
      console.error(JSON.stringify({errors: [redact('Join failed: ' + invite)]}));
      throw sanitizeError(error);
    }
  `], {encoding: 'utf8', timeout: 5000});
  assert.equal(result.error, undefined);
  assert.equal(result.status, 1);
  assert.doesNotMatch(result.stderr, /fixture-private-capability/);
  assert.match(result.stderr, /\[redacted-invite\]/);
  assert.match(result.stderr, /AssertionError/);
  assert.match(result.stderr, /originalCaller/);
});
