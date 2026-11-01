import http from 'node:http';
import * as restate from '@restatedev/restate-sdk';
import { latest, observe, readStore, setFaults, takeFailure, writeStore } from './read-model.mjs';

const storePath = process.env.NORTHWIND_STORE ?? '/tmp/northwind-read-model.json';
const controlPort = Number(process.env.NORTHWIND_CONTROL_PORT ?? 9081);

function update(mutator) {
  const store = readStore(storePath);
  const result = mutator(store);
  writeStore(storePath, store);
  return result;
}

const control = http.createServer((request, response) => {
  const url = new URL(request.url ?? '/', 'http://northwind');
  if (request.method === 'GET' && url.pathname.startsWith('/observations/')) {
    const [, , sessionId, operationId] = url.pathname.split('/');
    const found = latest(readStore(storePath), decodeURIComponent(sessionId), decodeURIComponent(operationId));
    response.writeHead(found ? 200 : 404, { 'content-type': 'application/json' });
    response.end(JSON.stringify(found ?? { synthetic: true, found: false }));
    return;
  }
  if (request.method === 'POST' && url.pathname === '/faults') {
    const chunks = [];
    request.on('data', (chunk) => chunks.push(chunk));
    request.on('end', () => {
      const body = JSON.parse(Buffer.concat(chunks).toString() || '{}');
      if (!body.session_id) {
        response.writeHead(400).end('{"error":"session_id required"}');
        return;
      }
      const faults = update((store) => setFaults(store, body.session_id, body));
      response.writeHead(200, { 'content-type': 'application/json' });
      response.end(JSON.stringify({ synthetic: true, faults }));
    });
    return;
  }
  response.writeHead(404).end();
});
control.listen(controlPort, '0.0.0.0');

const receiptDelivery = restate.workflow({
  name: 'ReceiptDelivery',
  handlers: {
    run: async (ctx, envelope) => {
      const job = JSON.parse(envelope.payload);
      const decision = update((store) => takeFailure(store, job.session_id));
      if (decision.failed) {
        throw new Error('synthetic delivery failure');
      }
      if (decision.drop_ack) {
        return { synthetic: true, status: 'acknowledgement dropped by fault control' };
      }
      const delay = decision.delay_ms > 0 ? decision.delay_ms : Math.min(Math.max(job.delay_ms ?? 15000, 0), 60000);
      await ctx.sleep({ milliseconds: delay });
      const observed = update((store) => observe(store, job.session_id, job.receipt_id || job.kind || 'operation', 'submitted'));
      const deliver = (name) => ctx.run(name, async () => {
        const callback = await fetch(`${process.env.RECORD_API_URL ?? 'http://host.docker.internal:8080'}/internal/workflow/complete`, {
          method: 'POST', headers: { 'content-type': 'application/json' }, body: JSON.stringify(envelope),
          signal: AbortSignal.timeout(10000),
        });
        if (callback.status === 401) throw new restate.TerminalError('Callback signature is no longer valid');
        if (!callback.ok) throw new Error(`Callback HTTP ${callback.status}`);
        return callback.json();
      });
      const result = await deliver(`acknowledge-${observed.revision}`);
      if (decision.duplicate) await deliver(`acknowledge-${observed.revision}-duplicate`);
      return { synthetic: true, revision: observed.revision, ...result };
    },
  },
});

restate.serve({ services: [receiptDelivery], port: 9080 });
