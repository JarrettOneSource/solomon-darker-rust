// Read the browser's actual completed save. These helpers never write authority.
const assert = require('node:assert/strict');

async function readSave(page) {
  return page.evaluate(() => new Promise((resolve, reject) => {
    const open = indexedDB.open('solomon-darker-rust-saves', 1);
    open.onerror = () => reject(open.error);
    open.onsuccess = () => {
      const database = open.result;
      const request = database.transaction('slots').objectStore('slots').get(0);
      request.onerror = () => {
        database.close();
        reject(request.error);
      };
      request.onsuccess = () => {
        database.close();
        if (request.result === undefined) {
          resolve(null);
          return;
        }
        const stored = JSON.parse(request.result);
        resolve({ ...stored, ...JSON.parse(stored.document) });
      };
    };
  }));
}

async function waitForSave(page, predicate) {
  const deadline = Date.now() + 20000;
  do {
    const record = await readSave(page);
    if (record && predicate(record)) return record;
    await page.waitForTimeout(100);
  } while (Date.now() < deadline);
  throw new Error(`Save did not commit: ${await page.locator('#status').textContent()}`);
}

function localParticipant(record) {
  assert.equal(record.schema_version, 2, 'the browser writes the participant save schema');
  // Standalone restore selects the first admitted participant, not map-key order.
  const id = record.checkpoint.participant_order[0];
  assert.equal(typeof id, 'string', 'the save retains its local participant order');
  const participant = record.checkpoint.participants[id];
  assert.equal(participant?.id, id, 'the saved local participant exists');
  return participant;
}

function savedWizardState(record) {
  const wizard = localParticipant(record).wizard;
  return [
    wizard.position.x,
    wizard.position.y,
    wizard.heading_index,
    wizard.gait_degrees,
    wizard.walk_cycle_primary,
    record.checkpoint.ticks,
  ];
}

module.exports = { readSave, waitForSave, localParticipant, savedWizardState };
