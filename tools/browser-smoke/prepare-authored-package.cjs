// Prepare an unchanged Source example through the actual isolated backend.
// Registration credentials stay in the same mode-0600 convention as hosted-social.
const assert = require('node:assert/strict');
const fs = require('node:fs');
const path = require('node:path');
const crypto = require('node:crypto');
const api = new URL(process.env.API || 'http://127.0.0.1:5229/');
assert.equal(api.hostname, '127.0.0.1', 'this producer is only for the local staging database');
assert.equal(api.port, '5229', 'the isolated backend must be selected explicitly');
const output = path.resolve(process.env.OUT);
const accountFile = path.join(output, 'private-account.json');
const packagePath = path.resolve(process.env.PACKAGE);
const packageName = process.env.PACKAGE_NAME || 'Apprentice Apothecary';
const packageId = process.env.PACKAGE_ID || 'showcase.apprentice-apothecary';
const source = process.env.PACKAGE_SOURCE || 'Website/frontend/examples/web-lua/apprentice-apothecary at ed0a2d59';
const graphSha256 = process.env.GRAPH_SHA256 || 'a27f450e7ea138d129d9b948a617be6a9f382eda1643ae5c86550f40eb3cf17f';
const bytes = fs.readFileSync(packagePath);
const receiptFile = path.join(output, 'package.json');
fs.mkdirSync(output, {recursive: true});
async function body(response, label) {
  const value = await response.json();
  assert.ok(response.ok, `${label}: ${response.status} ${value.error || value.message || ''}`);
  return value;
}
(async () => {
  let account;
  if (fs.existsSync(accountFile)) {
    account = JSON.parse(fs.readFileSync(accountFile));
  } else {
    const configuration = JSON.parse(fs.readFileSync(process.env.PRIVATE_CONFIG));
    const suffix = Date.now().toString(36);
    const username = `Authored${suffix}`;
    const email = `${username.toLowerCase()}@example.invalid`;
    const registered = await body(await fetch(new URL('api/auth/register', api), {
      method: 'POST', headers: {'Content-Type': 'application/json'},
      body: JSON.stringify({username, email, password: configuration.testPassword}),
    }), 'local QA registration');
    account = {username, email, password: configuration.testPassword, token: registered.token};
    fs.writeFileSync(accountFile, JSON.stringify(account, null, 2), {mode: 0o600});
  }
  fs.chmodSync(accountFile, 0o600);
  if (!account.token) {
    const loggedIn = await body(await fetch(new URL('api/auth/login', api), {
      method: 'POST', headers: {'Content-Type': 'application/json'},
      body: JSON.stringify({usernameOrEmail: account.email, password: account.password}),
    }), 'local QA login');
    account.token = loggedIn.token;
    fs.writeFileSync(accountFile, JSON.stringify(account, null, 2), {mode: 0o600});
  }
  const authorization = {Authorization: `Bearer ${account.token}`};
  const packageSha256 = crypto.createHash('sha256').update(bytes).digest('hex');
  let detail;
  if (fs.existsSync(receiptFile)) {
    const previous = JSON.parse(fs.readFileSync(receiptFile));
    assert.equal(previous.packageSha256, packageSha256, 'retain the accepted package identity');
    detail = await body(await fetch(new URL(`api/mods/${previous.slug}`, api), {headers: authorization}), 'retained local package');
  } else {
    const form = new FormData();
    form.set('name', packageName);
    form.set('summary', 'Unchanged pinned Source showcase for local Rust parity acceptance.');
    form.set('description', `Local acceptance only: the original ${packageName} example, with its unchanged Source Lua, CC0 art and audio.`);
    form.set('version', '1.0.0');
    form.set('visibility', 'public');
    form.set('tags', 'lua');
    form.set('file', new Blob([bytes], {type: 'application/zip'}), path.basename(packagePath));
    detail = await body(await fetch(new URL('api/mods', api), {method: 'POST', headers: authorization, body: form}), 'local example upload');
  }
  const active = await body(await fetch(new URL('api/mods/active', api), {headers: authorization}), 'actual enabled content');
  assert.equal(detail.packageId, packageId, 'the backend admitted the exact Source package identity');
  assert.ok(!active.mods.some(mod => mod.id === packageId),
    'activation remains an actual browser Cloud action in the held journey');
  const receipt = {
    apiOrigin: api.origin, slug: detail.slug, packageId: detail.packageId,
    version: '1.0.0', bytes: bytes.length, packageSha256,
    contentSha256: detail.versions.find(version => version.version === '1.0.0').contentSha256,
    source,
    graphSha256,
    account: account.username, activated: false,
  };
  fs.writeFileSync(receiptFile, JSON.stringify(receipt, null, 2));
  console.log(JSON.stringify(receipt));
})().catch(error => {console.error(error.message); process.exitCode = 1;});
