import test from 'node:test';
import assert from 'node:assert/strict';
import fs from 'node:fs';
import os from 'node:os';
import path from 'node:path';
import { observe, latest, setFaults, takeFailure, readStore, writeStore, emptyStore } from './read-model.mjs';

test('read model revision only advances when an observation is recorded', () => {
  const store = emptyStore();
  assert.equal(latest(store, 'session-a', 'nw-B2'), null);
  const first = observe(store, 'session-a', 'nw-B2', 'acknowledged');
  assert.equal(first.revision, 1);
  observe(store, 'session-a', 'nw-B2', 'acknowledged');
  assert.equal(latest(store, 'session-a', 'nw-B2').revision, 2);
  assert.equal(latest(store, 'session-b', 'nw-B2'), null);
});

test('fault controls are isolated and fail_next is consumed', () => {
  const store = emptyStore();
  setFaults(store, 'session-a', { fail_next: 2, drop_ack: true, duplicate: false, delay_ms: 10 });
  setFaults(store, 'session-b', { fail_next: 0 });
  assert.equal(takeFailure(store, 'session-a').failed, true);
  assert.equal(takeFailure(store, 'session-a').failed, true);
  assert.equal(takeFailure(store, 'session-a').failed, false);
  assert.equal(takeFailure(store, 'session-a').drop_ack, true);
  assert.equal(takeFailure(store, 'session-b').failed, false);
});

test('store file round-trips', () => {
  const file = path.join(os.tmpdir(), `northwind-${process.pid}.json`);
  const store = emptyStore();
  observe(store, 's', 'op', 'acknowledged');
  writeStore(file, store);
  const loaded = readStore(file);
  assert.equal(loaded.revisions['s:op'].revision, 1);
  fs.unlinkSync(file);
});
