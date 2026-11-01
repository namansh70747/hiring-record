'use client';
import { useEffect, useState } from 'react';
import type { View } from '../../lib/types';

type NewRole = {role_id:string;standard:{id:string;effective_at:string;criteria:{id:string;text:string;added_at:string;added_by:string}[]}};
const today=()=>new Date().toISOString().slice(0,10);
export default function RolesPage() {
  const [roles,setRoles]=useState<string[]>([]);const [selected,setSelected]=useState('');
  const [view,setView]=useState<View|null>(null);const [error,setError]=useState('');const [busy,setBusy]=useState(false);
  const [roleId,setRoleId]=useState('');const [criterion,setCriterion]=useState('');const [personName,setPersonName]=useState('');
  const [person,setPerson]=useState('');const [source,setSource]=useState('');const [met,setMet]=useState('true');const [proposalText,setProposalText]=useState('');
  async function api(path:string,method='GET',body?:unknown,version='0:0') {
    const response=await fetch(path,{method,credentials:'same-origin',headers:method==='POST'?{'Content-Type':'application/json','If-Match':version,'Idempotency-Key':crypto.randomUUID()}:{},body:body===undefined?undefined:JSON.stringify(body),cache:'no-store'});
    if(!response.ok)throw new Error(response.status===409?'The record changed. Reload it and try again.':`Request failed (${response.status}).`);
    return response.json();
  }
  async function refreshRoles(){const data=await api('/v1/roles');setRoles(data.role_ids);}
  async function load(id:string){setError('');try{const result:View=await api(`/v1/roles/${encodeURIComponent(id)}/view`);setView(result);setSelected(id);setPerson(result.people[0]?.id??'');}catch(e){setError(e instanceof Error?e.message:'Unable to load the role.')}}
  useEffect(()=>{void refreshRoles().catch(()=>setError('Could not load your roles.'));},[]);
  async function run(path:string,body:unknown,version:string,after?:(data:View)=>void){if(busy)return;setBusy(true);setError('');try{const next:View=await api(path,'POST',body,version);setView(next);after?.(next);await refreshRoles();}catch(e){setError(e instanceof Error?e.message:'The action failed.');}finally{setBusy(false);}}
  function command(body:unknown){if(view)void run(`/v1/roles/${encodeURIComponent(selected)}/commands`,body,view.version);}
  async function saveAssessment(){
    if(!view||busy||!person||!source.trim())return;
    setBusy(true);setError('');
    try {
      const id=`s-${crypto.randomUUID()}`;const path=`/v1/roles/${encodeURIComponent(selected)}/commands`;
      const next:View=await api(path,'POST',{type:'record_source',source:{id,person,kind:'Synthetic note',timestamp:'00:00:00',quote:source}},view.version);
      setView(next);
      const assessed:View=await api(path,'POST',{type:'assess',assessment:{id:`a-${crypto.randomUUID()}`,person,criterion:next.criteria[0].id,version:next.standard_id,met:met==='true',citation:id}},next.version);
      setView(assessed);setSource('');
    } catch(e) {setError(e instanceof Error?e.message:'Could not save the assessment.');} finally {setBusy(false);}
  }
  function create(event:React.FormEvent){event.preventDefault();const date=today();const body:NewRole={role_id:roleId,standard:{id:'v1',effective_at:date,criteria:[{id:'core',text:criterion,added_at:date,added_by:'presenter'}]}};void run('/v1/roles',body,'0:0',()=>{setSelected(roleId);setRoleId('');setCriterion('');});}
  return <main className="roles-page"><header className="roles-header"><a href="/">hiringrecord.</a><nav><a href="/record/">September demo</a><a href="/developers/">Developers</a></nav></header><div className="roles-wrap"><p className="roles-kicker">SYNTHETIC WORKSPACE / ROLES</p><h1>Build a record of your own.</h1><p>Use fictional people and evidence. Each role belongs only to this browser’s synthetic session.</p>
    {error&&<div className="roles-error" role="alert">{error} <button onClick={()=>selected&&void load(selected)}>Reload record</button></div>}
    <div className="roles-layout"><aside><h2>Your roles</h2>{roles.map(id=><button key={id} className={selected===id?'selected':''} onClick={()=>id==='sept'?window.location.assign('/record/'):void load(id)}>{id==='sept'?'September demo':id}</button>)}<form onSubmit={create}><h3>Create a role</h3><label>Role ID<input required pattern="[A-Za-z0-9_-]+" maxLength={100} value={roleId} onChange={e=>setRoleId(e.target.value)} placeholder="e.g. product-lead"/></label><label>First criterion<input required maxLength={300} value={criterion} onChange={e=>setCriterion(e.target.value)} placeholder="e.g. Built a product"/></label><button disabled={busy}>Create synthetic role</button></form></aside>
    <section>{!view?<div className="roles-empty"><h2>Start with a standard.</h2><p>Create a role or choose one from the list to add synthetic people, evidence and decisions.</p></div>:<><div className="roles-title"><div><p className="roles-kicker">SYNTHETIC ROLE</p><h2>{selected}</h2><p>Standard {view.standard_id} · event {view.at} · version {view.version}</p></div><button disabled={busy} onClick={()=>void load(selected)}>Refresh</button></div>
    <div className="roles-cards"><article><h3>Hiring standard</h3><ul>{view.criteria.map(c=><li key={c.id}>{c.text} <small>· {c.added_at}</small></li>)}</ul>{view.proposal?.status==='open'?<div><p><b>Proposed:</b> {view.proposal.add.map(c=>c.text).join(', ')}</p><button disabled={busy} onClick={()=>command({type:'dismiss',proposal_id:view.proposal?.id})}>Dismiss</button><button disabled={busy} onClick={()=>command({type:'approve',proposal_id:view.proposal?.id,new_version_id:`v-${crypto.randomUUID()}`})}>Approve change</button></div>:<form onSubmit={e=>{e.preventDefault();const date=today();command({type:'propose',proposal:{id:`p-${crypto.randomUUID()}`,base:view.standard_id,status:'open',add:[{id:`c-${crypto.randomUUID()}`,text:proposalText,added_at:date,added_by:'presenter'}]}});setProposalText('');}}><label>New criterion<input value={proposalText} required maxLength={300} onChange={e=>setProposalText(e.target.value)}/></label><button disabled={busy}>Propose change</button></form>}</article>
    <article><h3>People</h3><ul>{view.people.map(p=><li key={p.id}>{p.name} · {p.place.replaceAll('_',' ')} · {p.stale?'stale':p.incomplete?'incomplete':'current'}</li>)}</ul><form onSubmit={e=>{e.preventDefault();command({type:'add_person',person:{id:`p-${crypto.randomUUID()}`,name:personName,place:'in_review'}});setPersonName('');}}><label>Synthetic person name<input value={personName} required maxLength={100} onChange={e=>setPersonName(e.target.value)}/></label><button disabled={busy}>Add person</button></form></article>
    <article><h3>Assessment and decision</h3>{view.people.length===0?<p>Add a person to start.</p>:<><label>Person<select value={person} onChange={e=>setPerson(e.target.value)}><option value="">Choose a person</option>{view.people.map(p=><option key={p.id} value={p.id}>{p.name}</option>)}</select></label><label>Source excerpt<textarea value={source} maxLength={5000} onChange={e=>setSource(e.target.value)} placeholder="Synthetic source note"/></label><label>Assessment<select value={met} onChange={e=>setMet(e.target.value)}><option value="true">Criterion met</option><option value="false">Criterion not met</option></select></label><button disabled={busy||!person||!source.trim()} onClick={()=>void saveAssessment()}>Save source and assess first criterion</button><p>Decision actions record a human decision on the current standard.</p><button disabled={busy||!person||!!view.people.find(p=>p.id===person)?.decision} onClick={()=>command({type:'record_decision',decision:{person,outcome:'hire',at:today(),version:view.standard_id}})}>Record hire</button><button disabled={busy||!person||!!view.people.find(p=>p.id===person)?.decision} onClick={()=>command({type:'record_decision',decision:{person,outcome:'reject',at:today(),version:view.standard_id}})}>Record reject</button></>}</article></div>
    <section className="roles-report"><h3>Report · standard {view.standard_id}</h3>{view.report.lines.map(line=><div key={line.criterion_id}><b>{line.text}: {line.naive}</b><p>{line.eligible?'Supported observation':line.refusal} · {line.reason}</p></div>)}</section></>}</section></div><footer>Synthetic people, records and events only.</footer></div></main>;
}
