const fs=require('node:fs');
const assert=require('node:assert/strict');
const {documentRetirement,residentSuspension}=require('./browser-errors.cjs');
const {row}=require('./diagnostic-retired-rewind.json');
const failure={...row.failure,status:row.status,errorText:row.failure.failure.errorText};
assert.equal(residentSuspension(failure,row.retirement.before,row.retirement.before),null);
const result=documentRetirement(failure,row.retirement);
assert.equal(result?.reason,'loaded resident channel rewinding during the observed document replacement');
fs.writeFileSync('reports/browser-admission/captured-rewind.json',JSON.stringify(result,null,2)+'\n');
