'use client';
import { useState } from 'react';
import type { View } from '../lib/types';
type Quote = { id:string;person:string;amount_minor:number;currency:string;status:string };
type Fact = { id:string;person:string;key:string;value:string;source_id:string };
type Override = { id:string;person:string;outcome:string;reason:string;at:string };
export function WorkflowWorkbench({view,busy,historical,execute}:{view:View;busy:boolean;historical:boolean;execute:(command:unknown,message:string)=>Promise<void>}) {
  const [person,setPerson]=useState('priya');
  const [factKey,setFactKey]=useState('Availability');
  const [factValue,setFactValue]=useState('');
  const [reason,setReason]=useState('');
  const [outcome,setOutcome]=useState('hire');
  const selected=view.people.find(p=>p.id===person);
  const disabled=busy||historical;
  const quotes=(view.quotes??[]) as Quote[];
  const facts=(view.facts??[]) as Fact[];
  const overrides=(view.overrides??[]) as Override[];
  return <section className="workbench" aria-labelledby="workbench-heading"><div className="action-section-head"><div><div className="overline">EXPLORE THE FULL RECORD</div><h2 id="workbench-heading">Decisions need context.</h2><p>Try a fictional quote, retain conflicting sourced facts, or record a human override.</p></div></div>
    <label className="workbench-person">Person <select value={person} onChange={e=>setPerson(e.target.value)}>{view.people.map(p=><option key={p.id} value={p.id}>{p.name}</option>)}</select></label>
    <div className="workbench-grid">
      <article><h3>Quote and receipt</h3><p>Offer a synthetic USD 250 quote. Acceptance creates a pending delivery receipt; declining creates no receipt.</p><button disabled={disabled} onClick={()=>execute({type:'offer_quote',quote:{id:crypto.randomUUID(),person,amount_minor:25000,currency:'USD',status:'offered'}},'Synthetic quote offered.')}>Offer $250 quote</button>
      {quotes.filter(q=>q.person===person).map(q=><div className="workbench-item" key={q.id}><b>{q.currency} {(q.amount_minor/100).toFixed(2)} · {q.status}</b>{q.status==='offered'&&<div><button disabled={disabled} onClick={()=>execute({type:'accept_quote',quote_id:q.id},'Quote accepted. Delivery remains pending until acknowledged.')}>Accept</button><button disabled={disabled} onClick={()=>execute({type:'decline_quote',quote_id:q.id},'Quote declined. Nothing was sent.')}>Decline</button></div>}</div>)}
      </article>
      <article><h3>Sourced facts</h3><p>Store a human interpretation linked to this person’s synthetic source. A different value stays beside the earlier one.</p><form onSubmit={e=>{e.preventDefault();if(!selected?.source)return;void execute({type:'record_fact',fact:{id:crypto.randomUUID(),person,key:factKey,value:factValue,source_id:selected.source.id}},'Sourced fact added; earlier values retained.');setFactValue('');}}><label>Fact name<input value={factKey} required maxLength={100} onChange={e=>setFactKey(e.target.value)}/></label><label>Interpreted value<input value={factValue} required maxLength={1000} onChange={e=>setFactValue(e.target.value)}/></label><button disabled={disabled||!selected?.source}>Add sourced fact</button></form>
      {facts.filter(f=>f.person===person).map(f=><div className="workbench-item" key={f.id}><b>{f.key}: {f.value}</b><small>Source: {f.source_id} · human interpretation</small></div>)}
      </article>
      <article><h3>Human override</h3><p>Record a human decision and reason alongside the original assessment. Its score and citation remain in history.</p><form onSubmit={e=>{e.preventDefault();void execute({type:'override_decision',decision_override:{id:crypto.randomUUID(),person,outcome,reason,at:new Date().toISOString().slice(0,10)}},'Human override recorded alongside the original assessment.');setReason('');}}><label>Outcome<select value={outcome} onChange={e=>setOutcome(e.target.value)}><option value="hire">Hire</option><option value="reject">Reject</option></select></label><label>Human reason<textarea value={reason} required maxLength={1000} onChange={e=>setReason(e.target.value)}/></label><button disabled={disabled}>Record override</button></form>
      {overrides.filter(o=>o.person===person).map(o=><div className="workbench-item" key={o.id}><b>{o.outcome} · {o.at}</b><p>{o.reason}</p></div>)}
      </article>
    </div><p>All actions use synthetic data. Historical views are read-only. Source linkage does not verify the interpretation’s truth.</p>
  </section>;
}
