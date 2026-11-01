import { PDFDocument, StandardFonts } from '../apps/web/node_modules/pdf-lib/dist/pdf-lib.esm.js';
import assert from 'node:assert/strict';

const bundle = JSON.stringify({ format: 'hiring-record-bundle/v1', algorithm: 'Ed25519', public_key: 'ab', payload: '{"format":"hiring-record-payload/v2","role_id":"sept"}', signature: 'cd' });
const pdf = await PDFDocument.create();
const font = await pdf.embedFont(StandardFonts.Helvetica);
const page = pdf.addPage([612, 792]);
page.drawText('Verify the embedded signed record. PDF page bytes are not digitally signed.', { x: 48, y: 700, size: 11, font });
await pdf.attach(new TextEncoder().encode(bundle), 'hiring-record-sealed-bundle.json', { mimeType: 'application/json' });
const bytes = new Uint8Array(await pdf.save({ useObjectStreams: false }));
const text = new TextDecoder('latin1').decode(bytes);
const streams = text.matchAll(/stream\r?\n([\s\S]*?)\r?\nendstream/g);
const found = [];
for (const match of streams) {
  const raw = Uint8Array.from(match[1], (char) => char.charCodeAt(0));
  try {
    const inflated = await new Response(new Blob([raw]).stream().pipeThrough(new DecompressionStream('deflate'))).text();
    if (inflated.includes('hiring-record-bundle/v1')) found.push(inflated);
  } catch {}
}
assert.equal(found.length, 1);
assert.equal(JSON.parse(found[0]).payload.includes('sept'), true);
assert.equal(text.includes('/EmbeddedFile'), true);
console.log('pdf embedded-json round-trip ok');
