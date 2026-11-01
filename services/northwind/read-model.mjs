import fs from 'node:fs';

export function emptyStore() {
  return { revisions: {}, faults: {} };
}

export function readStore(path) {
  try {
    const parsed = JSON.parse(fs.readFileSync(path, 'utf8'));
    return { revisions: parsed.revisions ?? {}, faults: parsed.faults ?? {} };
  } catch {
    return emptyStore();
  }
}

export function writeStore(path, store) {
  const index = path.lastIndexOf('/');
  if (index > 0) fs.mkdirSync(path.slice(0, index), { recursive: true });
  fs.writeFileSync(path, JSON.stringify(store));
}

export function observe(store, sessionId, operationId, outcome) {
  const key = `${sessionId}:${operationId}`;
  const revision = (store.revisions[key]?.revision ?? 0) + 1;
  store.revisions[key] = { revision, outcome, at: new Date().toISOString() };
  return store.revisions[key];
}

export function latest(store, sessionId, operationId) {
  return store.revisions[`${sessionId}:${operationId}`] ?? null;
}

export function setFaults(store, sessionId, faults) {
  store.faults[sessionId] = {
    delay_ms: Math.min(Math.max(Number(faults.delay_ms ?? 0), 0), 60000),
    fail_next: Math.min(Math.max(Number(faults.fail_next ?? 0), 0), 5),
    drop_ack: Boolean(faults.drop_ack),
    duplicate: Boolean(faults.duplicate),
  };
  return store.faults[sessionId];
}

export function takeFailure(store, sessionId) {
  const faults = store.faults[sessionId] ?? { delay_ms: 0, fail_next: 0, drop_ack: false, duplicate: false };
  if (faults.fail_next > 0) {
    faults.fail_next -= 1;
    store.faults[sessionId] = faults;
    return { ...faults, failed: true };
  }
  return { ...faults, failed: false };
}
