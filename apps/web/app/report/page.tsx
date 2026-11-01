'use client';

import { useEffect, useState } from 'react';
import type { Event, Report } from '../../lib/types';

type Envelope = { format: string; algorithm: string; public_key: string; signature: string; payload: string };
type Payload = { role_id: string; standard_id: string; report: Report; events: Event[]; disclosure: string; source_references?: { id:string; person:string; kind:string; timestamp:string }[] };

export default function ReportPage() {
  const [bundle, setBundle] = useState<Envelope | null>(null);
  const [payload, setPayload] = useState<Payload | null>(null);
  const [error, setError] = useState('');
  useEffect(() => {
    fetch('/v1/roles/sept/bundle', { credentials: 'same-origin', cache: 'no-store' })
      .then(async response => { if (!response.ok) throw new Error('The report is unavailable. Return to the record and try again.'); return response.json(); })
      .then((data: Envelope) => { setPayload(JSON.parse(data.payload) as Payload); setBundle(data); })
      .catch((err: unknown) => setError(err instanceof Error ? err.message : 'The report is unavailable.'));
  }, []);
  const download = () => {
    if (!bundle) return;
    const url = URL.createObjectURL(new Blob([JSON.stringify(bundle, null, 2)], { type: 'application/json' }));
    const link = document.createElement('a'); link.href = url; link.download = 'hiring-record-sealed-bundle.json'; link.click();
    window.setTimeout(() => URL.revokeObjectURL(url), 1000);
  };
  return <main className="print-report">
    <nav className="report-nav"><a href="/record/">← Back to record</a><a href="/verify/">Verify bundle</a></nav>
    <header><p className="report-kicker">HIRING RECORD / SYNTHETIC SCENARIO</p><h1>The evidence behind the bar.</h1><p>A snapshot of the observations and the claims this record can support.</p></header>
    {error && <p role="alert">{error}</p>}
    {!payload && !error && <p role="status">Preparing your report…</p>}
    {payload && bundle && <>
      <div className="report-actions"><button onClick={download}>Download signed JSON</button><button onClick={() => window.print()}>Print / save as PDF</button></div>
      <div className="report-meta"><span>Role: <b>{payload.role_id}</b></span><span>Standard: <b>{payload.standard_id}</b></span><span>Events: <b>{payload.events.length}</b></span></div>
      <section><h2>Observations and checks</h2>{payload.report.lines.map(line => <article className="print-claim" key={line.criterion_id}><h3>{line.text}</h3><p><b>{line.naive}</b></p><p><strong>{line.eligible ? 'Supported observation' : `Refused · ${line.refusal}`}</strong> — {line.reason}</p></article>)}</section>
      {payload.source_references && payload.source_references.length > 0 && <section className="print-sources"><h2>Source references</h2><p>These synthetic references identify source moments; they are not complete documents.</p><ul>{payload.source_references.map(source=><li key={source.id}><b>{source.person}</b> · {source.kind} · {source.timestamp} · <code>{source.id}</code></li>)}</ul></section>}
      <section className="print-integrity"><h2>Integrity details</h2><p>Event range: {payload.events.length ? `1–${payload.events.length}` : 'No events'}</p><p>Root hash: <code>{payload.events.at(-1)?.hash ?? 'genesis'}</code></p><p>Signature ({bundle.algorithm}): <code>{bundle.signature}</code></p><p>Public key: <code>{bundle.public_key}</code></p><p>These values describe the signed JSON bundle. This printed page or PDF is a readable rendering; verify the JSON separately to check integrity and compare its public key with one you trust. A signature does not establish the truth of the evidence.</p></section>
    </>}
    <footer>All people and events are fictional. Northwind is simulated.</footer>
  </main>;
}
