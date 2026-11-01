import { PDFDocument, StandardFonts } from 'pdf-lib';

const marker = 'hiring-record-bundle/v1';

export async function pdfWithBundle(bundleJson: string): Promise<Uint8Array> {
  const pdf = await PDFDocument.create();
  const font = await pdf.embedFont(StandardFonts.Helvetica);
  const page = pdf.addPage([612, 792]);
  page.drawText('Hiring Record sealed report', { x: 48, y: 740, size: 18, font });
  page.drawText('Verify the embedded signed record. PDF page bytes are not digitally signed.', { x: 48, y: 700, size: 11, font, maxWidth: 520 });
  await pdf.attach(new TextEncoder().encode(bundleJson), 'hiring-record-sealed-bundle.json', {
    mimeType: 'application/json',
    description: 'Authoritative signed Hiring Record bundle',
  });
  return pdf.save({ useObjectStreams: false });
}

export async function extractEmbeddedBundle(bytes: Uint8Array): Promise<string> {
  const text = new TextDecoder('latin1').decode(bytes);
  const streams = text.matchAll(/stream\r?\n([\s\S]*?)\r?\nendstream/g);
  const found: string[] = [];
  for (const match of streams) {
    const raw = Uint8Array.from(match[1], (char) => char.charCodeAt(0));
    try {
      const inflated = await inflateZlib(raw);
      if (inflated.includes(marker)) found.push(inflated);
    } catch {
      continue;
    }
  }
  if (found.length !== 1) {
    throw new Error(found.length === 0 ? 'The PDF has no embedded signed record.' : 'The PDF has more than one signed record. Choose one JSON bundle instead.');
  }
  return found[0];
}

async function inflateZlib(bytes: Uint8Array): Promise<string> {
  const stream = new Blob([Uint8Array.from(bytes)]).stream().pipeThrough(new DecompressionStream('deflate'));
  return new Response(stream).text();
}
