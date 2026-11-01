import assert from 'node:assert/strict';
import { readFile } from 'node:fs/promises';
import init, { compare_other_path, evaluate_report } from '../apps/web/public/engine/record_wasm.js';

const base = process.env.RECORD_BASE_URL || 'http://127.0.0.1:8080';
const headers = {};
let version = '0:0';
await init({ module_or_path: await readFile(new URL('../apps/web/public/engine/record_wasm_bg.wasm', import.meta.url)) });
async function get(path, method = 'GET') {
  const response = await fetch(base + path, { method, headers: { ...headers, 'Idempotency-Key': crypto.randomUUID(), 'If-Match': version } });
  const cookie = response.headers.get('set-cookie');
  if (cookie) headers.cookie = cookie.split(';')[0];
  if (!response.ok) throw new Error(`${path}: HTTP ${response.status}`);
  const result = await response.json();
  if (result.version) version = result.version;
  return result;
}
for (const stage of ['initial', 'approved']) {
  const view = await get('/v1/roles/sept/view');
  const clientReport = JSON.parse(evaluate_report(JSON.stringify(view.engine_state)));
  const serverReport = await get('/v1/roles/sept/report');
  assert.deepEqual(clientReport, serverReport, `${stage} report differs between WASM and API`);
  const clientComparison = JSON.parse(compare_other_path(JSON.stringify(view.engine_state)));
  const serverComparison = await get('/v1/roles/sept/counterfactual');
  assert.deepEqual(clientComparison, serverComparison, `${stage} comparison differs between WASM and API`);
  assert.equal(clientComparison.path, stage === 'initial' ? 'approved' : 'dismissed');
  if (stage === 'initial') await get('/v1/roles/sept/proposals/B/approve', 'POST');
}
console.log('PASS: browser WASM and API agree on reports and both counterfactual paths');
