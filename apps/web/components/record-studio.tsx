'use client';

import { useCallback, useEffect, useMemo, useRef, useState } from 'react';
import { ArrowRight, ArrowUpRight, Braces, Check, ChevronDown, ChevronLeft, ChevronRight, CircleAlert, CircleCheck, Clock3, Copy, Download, FileCheck2, GitBranch, History, Info, Layers3, LockKeyhole, MoreHorizontal, Play, Radio, RotateCcw, ShieldCheck, Sparkles, Users, X } from 'lucide-react';
import { WorkflowWorkbench } from './workflow-workbench';
import { verifyChain } from '../lib/integrity';
import { compareInBrowser } from '../lib/engine';
import { codeName, type Claim, type Event, type Person, type Comparison, type View } from '../lib/types';

function Mark() { return <span className="brand-mark" aria-hidden="true"><i/><i/><i/></span>; }
const labelForEvent: Record<string,string> = { DomainCommandV2:'Context recorded', ProposalDismissed:'Proposal dismissed', StandardApproved:'Standard approved', ReceiptAcknowledged:'Receipt acknowledged', HoldConfirmed:'Hold confirmed', SupportRemoved:'Support removed' };
function displayStage(stage: string) { return stage === 'in_review' ? 'In review' : stage === 'hired' ? 'Hired' : 'Passed'; }
function initials(name: string) { return name.slice(0, 2).toUpperCase(); }
function shortHash(hash: string) { return hash.length > 20 ? `${hash.slice(0, 10)}…${hash.slice(-6)}` : hash; }
function useDialogFocus<T extends HTMLElement>(onClose: () => void) {
  const ref = useRef<T>(null);
  useEffect(() => {
    const previous = document.activeElement as HTMLElement | null;
    const frame = requestAnimationFrame(() => ref.current?.querySelector<HTMLElement>('button')?.focus());
    const onKey = (event: KeyboardEvent) => {
      if (event.key === 'Escape') { event.preventDefault(); onClose(); return; }
      if (event.key !== 'Tab' || !ref.current) return;
      const controls = [...ref.current.querySelectorAll<HTMLElement>('button:not([disabled]), a[href], input:not([disabled]), [tabindex]:not([tabindex="-1"])')];
      if (!controls.length) { event.preventDefault(); return; }
      const first = controls[0]; const last = controls[controls.length - 1];
      if (event.shiftKey && document.activeElement === first) { event.preventDefault(); last.focus(); }
      else if (!event.shiftKey && document.activeElement === last) { event.preventDefault(); first.focus(); }
    };
    document.addEventListener('keydown', onKey);
    return () => { cancelAnimationFrame(frame); document.removeEventListener('keydown', onKey); previous?.focus(); };
  }, [onClose]);
  return ref;
}

export function RecordStudio() {
  const [view, setView] = useState<View | null>(null);
  const [liveEvents, setLiveEvents] = useState<Event[]>([]);
  const [activePerson, setActivePerson] = useState<Person | null>(null);
  const [compare, setCompare] = useState<Comparison | null>(null);
  const [compareEngine, setCompareEngine] = useState<'browser' | 'server'>('browser');
  const [busy, setBusy] = useState(false);
  const [connection, setConnection] = useState('Connecting…');
  const busyFlag = useRef(false);
  const historicalFlag = useRef(false);
  const [notice, setNotice] = useState('');
  const [error, setError] = useState('');
  const [mobilePanel, setMobilePanel] = useState<'people'|'standard'|'report'>('standard');
  const [peopleFilter, setPeopleFilter] = useState<'all'|'in_review'>('all');
  const [showAllClaims, setShowAllClaims] = useState(false);
  const [verifyResult, setVerifyResult] = useState<'valid'|'invalid'|null>(null);
  const [showEvents, setShowEvents] = useState(false);
  const [info, setInfo] = useState(false);
  const [palette, setPalette] = useState(false);
  const [guide, setGuide] = useState(false);
  const session = useRef('');
  const latestLoad = useRef(0);
  const currentVersion = useRef('0:0');
  const noticeTimer = useRef<ReturnType<typeof setTimeout> | null>(null);

  const announce = useCallback((message: string) => {
    setNotice(message);
    if (noticeTimer.current) clearTimeout(noticeTimer.current);
    noticeTimer.current = setTimeout(() => setNotice(''), 4200);
  }, []);
  const request = useCallback(async (path: string, method = 'GET', body?: unknown) => {
    const headers: Record<string,string> = method === 'POST' ? { 'Idempotency-Key': crypto.randomUUID(), 'If-Match': currentVersion.current } : {};
    if (body !== undefined) headers['Content-Type'] = 'application/json';
    const options: RequestInit = { method, headers, body: body === undefined ? undefined : JSON.stringify(body), credentials: 'same-origin', cache: 'no-store' };
    let response: Response;
    try { response = await fetch(path, options); } catch { response = await fetch(path, options); }
    if (response.status === 401) throw new Error('Your scenario session expired. Choose Start new scenario to continue.');
    if (!response.ok) throw new Error(response.status === 409 ? 'That action is no longer available. Return to the live record or reset the scenario.' : `The record could not be loaded (${response.status}).`);
    const data = await response.json();
    return data;
  }, []);
  const load = useCallback(async (at?: number) => {
    const loadId = ++latestLoad.current;
    try {
      setError('');
      const next: View = await request(`/v1/roles/sept/view${at === undefined ? '' : `?at=${at}`}`);
      if (loadId !== latestLoad.current) return;
      currentVersion.current = next.version;
      setView(next);
      if (next.at === next.live_total) setLiveEvents(next.events);
    } catch (e) { setError(e instanceof Error ? e.message : 'Unable to load the record.'); }
  }, [request]);
  useEffect(() => {
    session.current = 'Protected by an HttpOnly session cookie';
    let source: EventSource | undefined;
    let disposed = false;
    void load().then(() => {
      if (disposed) return;
      source = new EventSource('/v1/roles/sept/stream');
      source.onopen = () => setConnection('Connected');
      source.onerror = () => setConnection('Reconnecting…');
      source.addEventListener('unavailable', () => setConnection('Record unavailable · retrying'));
      source.addEventListener('snapshot', (event) => {
        if (historicalFlag.current || busyFlag.current) return;
        const next: View = JSON.parse((event as MessageEvent).data);
        latestLoad.current++;
        currentVersion.current = next.version;
        setView(next); setLiveEvents(next.events);
      });
    });
    return () => { disposed = true; source?.close(); };
  }, [load]);
  useEffect(() => {
    return () => { if (noticeTimer.current) clearTimeout(noticeTimer.current); };
  }, [load]);
  const action = useCallback(async (path: string, message: string, command?: unknown) => {
    if (busy) return;
    if (path.endsWith('/reset') && !window.confirm('Reset this synthetic scenario? Your current path will be replaced.')) return;
    latestLoad.current++;
    setBusy(true); setError('');
    try {
      const next: View = await request(path, 'POST', command);
      currentVersion.current = next.version;
      setView(next); setLiveEvents(next.events); setCompare(null); setVerifyResult(null); announce(message);
    } catch (e) { setError(e instanceof Error ? e.message : 'The action could not be completed.'); }
    finally { setBusy(false); }
  }, [announce, busy, request]);
  const compareBranch = useCallback(async () => {
    if (!view || busy) return;
    setBusy(true); setError('');
    try {
      try {
        const result = await compareInBrowser(view.engine_state);
        setCompareEngine('browser'); setCompare(result);
      } catch {
        const result: Comparison = await request('/v1/roles/sept/counterfactual');
        setCompareEngine('server'); setCompare(result);
      }
    } catch (e) { setError(e instanceof Error ? e.message : 'Unable to calculate the comparison.'); }
    finally { setBusy(false); }
  }, [busy, request, view]);
  const isHistorical = !!view && view.at < view.live_total;
  busyFlag.current = busy;
  historicalFlag.current = isHistorical;
  useEffect(() => {
    function onKey(e: KeyboardEvent) {
      if ((['INPUT','TEXTAREA','SELECT'].includes((e.target as HTMLElement)?.tagName) || (e.target as HTMLElement)?.isContentEditable) || activePerson || compare || info) return;
      if ((e.metaKey || e.ctrlKey) && e.key.toLowerCase() === 'k') { e.preventDefault(); setPalette(p => !p); return; }
      if (e.metaKey || e.ctrlKey || e.altKey || palette) return;
      if (!view || busy) return;
      if (e.key === '[' && view.at > 0) { e.preventDefault(); void load(view.at - 1); }
      if (e.key === ']' && view.at < view.live_total) { e.preventDefault(); void load(view.at + 1); }
      if (!isHistorical && view.proposal_status === 'open' && e.key.toLowerCase() === 'd') { e.preventDefault(); void action('/v1/roles/sept/proposals/B/dismiss', 'Proposal dismissed. The standard and assessments are unchanged.'); }
      if (!isHistorical && view.proposal_status === 'open' && e.key.toLowerCase() === 'a') { e.preventDefault(); void action('/v1/roles/sept/proposals/B/approve', 'Standard B2 is active. Earlier assessments are now stale.'); }
    }
    window.addEventListener('keydown', onKey); return () => window.removeEventListener('keydown', onKey);
  }, [action, activePerson, busy, compare, info, isHistorical, load, palette, view]);
  const people = useMemo(() => view?.people.filter(p => peopleFilter === 'all' || p.place === 'in_review') ?? [], [view, peopleFilter]);
  const inFlight = view?.people.filter(p => p.place === 'in_review').length ?? 0;
  const staleCount = view?.people.filter(p => p.stale).length ?? 0;
  const incompleteCount = view?.people.filter(p => !p.stale && p.incomplete).length ?? 0;
  const claims = view?.report.lines.filter(l => !l.criterion_id.startsWith('op-')) ?? [];
  const ops = view?.report.lines.filter(l => l.criterion_id.startsWith('op-')) ?? [];
  const exportSnapshot = async () => {
    if (!view || isHistorical) return;
    setBusy(true);
    try {
      const bundle = await request('/v1/roles/sept/bundle');
      const a = document.createElement('a');
      const url = URL.createObjectURL(new Blob([JSON.stringify(bundle, null, 2)], { type: 'application/json' }));
      a.href = url; a.download = 'hiring-record-sealed-bundle.json'; a.click();
      window.setTimeout(() => URL.revokeObjectURL(url), 1000);
      announce('Signed bundle downloaded. Verify it on the verification page.');
    } catch (e) { setError(e instanceof Error ? e.message : 'Could not seal the report.'); }
    finally { setBusy(false); }
  };
  return <div className="studio">
    {palette && <CommandPalette onClose={()=>setPalette(false)} onCompare={()=>{setPalette(false);void compareBranch();}} onApprove={()=>{setPalette(false);void action('/v1/roles/sept/proposals/B/approve','Standard B2 is active. Earlier assessments are now stale.');}} onDismiss={()=>{setPalette(false);void action('/v1/roles/sept/proposals/B/dismiss','Proposal dismissed. The standard and assessments are unchanged.');}} onTimeline={()=>{setPalette(false);document.getElementById('timeline')?.scrollIntoView();}} canDecide={!!view && view.proposal_status==='open' && !busy && !isHistorical} canCompare={!!view && !busy && !isHistorical} />}
    <header className="studio-header"><a href="/" className="studio-brand"><Mark/><span>hiring<span>record</span><i>.</i></span></a><div className="header-divider"/><div className="studio-breadcrumb"><span>WORKSPACE</span><ChevronRight size={12}/><b>September role</b><ChevronDown size={13}/></div><div className="studio-header-right"><div className="synthetic-pill"><span/> SYNTHETIC WORKSPACE</div><button className="header-icon" onClick={() => setInfo(true)} aria-label="About this demo"><Info size={16}/></button><a className="back-to-site" href="/verify/">Verify <ArrowUpRight size={13}/></a><div className="account-avatar">HR</div></div></header>
    <div className="studio-frame"><aside className="studio-rail" aria-label="Workspace navigation"><a className="rail-button rail-active" href="/record/" title="Record" aria-label="Record"><Layers3 size={18}/></a><button className="rail-button" title="People" aria-label="Focus people" onClick={() => { setMobilePanel('people'); document.getElementById('people-panel')?.scrollIntoView({ behavior:'smooth', block:'nearest' }); }}><Users size={18}/></button><button className="rail-button" title="Timeline" aria-label="Open timeline" onClick={() => { setShowEvents(true); document.getElementById('timeline')?.scrollIntoView({ behavior:'smooth', block:'center' }); }}><History size={18}/></button><button className="rail-button" title="Report" aria-label="Focus report" onClick={() => { setMobilePanel('report'); document.getElementById('report-panel')?.scrollIntoView({ behavior:'smooth', block:'nearest' }); }}><FileCheck2 size={18}/></button><div className="rail-spacer"/><button className="rail-button" onClick={() => setInfo(true)} title="About" aria-label="About this demo"><Info size={18}/></button></aside>
      <main className="studio-main"><div className="studio-content"><div className="workspace-topline"><div className="breadcrumbs"><span>WORKSPACE</span><ChevronRight size={12}/><span>HIRING RECORD</span><ChevronRight size={12}/><b>SEPTEMBER ROLE</b></div><button className="text-control" onClick={() => action('/v1/roles/sept/reset', 'The synthetic scenario has been reset.')} disabled={busy}><RotateCcw size={13}/> Reset scenario</button></div>
        <div className="workspace-tools"><button onClick={()=>setPalette(true)}>Commands <kbd>⌘ K</kbd></button><button onClick={()=>setGuide(g=>!g)} aria-expanded={guide}>{guide ? 'Hide walkthrough' : 'Start Loom walkthrough'}</button></div>
        <div className="workspace-heading"><div><div className="workspace-kicker"><span/> LIVE SCENARIO &nbsp; / &nbsp; SEPTEMBER 2026</div><h1>Every decision has a history<span>.</span></h1><p>One hiring standard. The people already in flight. A report that knows its limits.</p></div><div className="workspace-heading-actions"><button className="quiet-button" onClick={compareBranch} disabled={!view || busy || isHistorical}><GitBranch size={15}/> Compare paths</button><a className="quiet-button" href="/report/">View report</a><button className="dark-button" onClick={exportSnapshot} disabled={!view || busy || isHistorical}><Download size={15}/> Seal &amp; export</button></div></div>
        <div className="workspace-meta"><div><span className="meta-icon"><Layers3 size={15}/></span><span><small>ROLE</small><b>Head of Engineering</b></span></div><div><span className="meta-icon"><Users size={15}/></span><span><small>PEOPLE IN FLIGHT</small><b>{inFlight.toString().padStart(2,'0')} in review</b></span></div><div><span className="meta-icon"><CircleCheck size={15}/></span><span><small>ACTIVE STANDARD</small><b>{view ? `Version ${view.standard_id}` : 'Loading…'}</b></span></div><div className="meta-last"><span className="meta-icon"><Radio size={15}/></span><span><small>RECORD STATUS</small><b>{isHistorical ? 'Viewing history' : connection}</b></span></div></div>
        {guide && <div className="guide-card" role="status"><strong>Walkthrough · {view?.proposal_status === 'open' ? '1 / Review the proposal' : view?.proposal_status === 'dismissed' ? '2 / Compare and reset' : '2 / Inspect the consequences'}</strong><p>{view?.proposal_status === 'open' ? 'Open a person, review the impact, then dismiss or approve the proposal.' : view?.proposal_status === 'dismissed' ? 'The assessments stay current. Compare the alternate path, then reset and approve it.' : 'Notice stale assessments and the frozen interview rubric. Read the report, replay history, compare paths, then export and verify.'}</p><a href="/report/">Open printable report →</a></div>}
        {error && <div className="studio-alert" role="alert"><CircleAlert size={16}/>{error}<button onClick={() => { if (error.includes('expired')) { void action('/v1/session/new', 'Started a new synthetic scenario.'); } else { setError(''); void load(); } }}>{error.includes('expired') ? 'Start new scenario' : 'Try again'}</button></div>}
        {notice && <div className="toast" role="status"><CircleCheck size={16}/>{notice}</div>}
        {isHistorical && <div className="history-banner"><History size={15}/><span>You’re viewing event {view?.at} of {view?.live_total}. Actions are paused in historical view.</span><button onClick={() => load()}>Return to live <ArrowRight size={14}/></button></div>}
        <div className="record-intro"><div><div className="overline">THE SHARED RECORD &nbsp; / &nbsp; 01</div><h2>One role, three perspectives<span>.</span></h2></div><div className="record-intro-note"><span className="live-indicator"/> {isHistorical ? 'HISTORICAL SNAPSHOT' : 'LIVE SYNTHETIC DATA'} <button onClick={() => setInfo(true)} aria-label="About live data"><Info size={13}/></button></div></div>
        <div className="mobile-panels" role="group" aria-label="Choose record panel">{(['people','standard','report'] as const).map(panel=><button key={panel} className={mobilePanel===panel?'selected':''} onClick={()=>setMobilePanel(panel)}>{panel === 'people'?'People':panel === 'standard'?'The standard':'The report'}</button>)}</div>
        <div className={`record-grid mobile-${mobilePanel}`}>
          <section className="record-panel people-panel" id="people-panel"><div className="panel-heading"><div className="panel-heading-left"><span className="panel-number">01</span><div><h3>People</h3><p>Everyone the bar touches</p></div></div><span className="panel-count">{view?.people.length.toString().padStart(2,'0') ?? '—'}</span></div><div className="panel-toolbar"><div className="filter-buttons"><button className={peopleFilter==='all'?'filter-selected':''} onClick={()=>setPeopleFilter('all')}>All people</button><button className={peopleFilter==='in_review'?'filter-selected':''} onClick={()=>setPeopleFilter('in_review')}>In flight <span>{inFlight}</span></button></div></div><div className="people-column-head"><span>PERSON / STAGE</span><span>ASSESSMENT</span></div><div className="people-list">{!view ? <div className="panel-loading">Loading people…</div> : people.map((person, index)=><button className="person-row" key={person.id} onClick={()=>setActivePerson(person)}><span className={`person-avatar tone-${index%5}`}>{initials(person.name)}</span><span className="person-text"><b>{person.name}</b><small>{displayStage(person.place)}{person.interview ? ' · Interview open' : ''}</small></span><span className={`assessment-status ${person.stale?'is-stale':''}`}><span/>{person.stale?'Stale':person.incomplete?'Incomplete':'Current'}<small>STANDARD {person.assessment?.version ?? '—'}</small></span><ChevronRight size={14} className="person-chevron"/></button>)}</div><div className="people-footer"><span><span className="green-dot"/> {view ? view.people.length-staleCount-incompleteCount : '—'} CURRENT</span><span><span className="amber-dot"/> {staleCount} STALE</span>{incompleteCount > 0 && <span>{incompleteCount} INCOMPLETE</span>}<button onClick={()=>setPeopleFilter(peopleFilter === 'all' ? 'in_review' : 'all')}>{peopleFilter==='all'?'Show in flight':'Show all'} <ArrowRight size={12}/></button></div></section>
          <section className="record-panel standard-panel" id="standard-panel"><div className="panel-heading"><div className="panel-heading-left"><span className="panel-number">02</span><div><h3>Hiring standard</h3><p>The bar, and how it changes</p></div></div><span className="version-pill">{view?.standard_id ?? '—'} <span>ACTIVE</span></span></div><div className="standard-body"><div className="section-micro"><span>CURRENT CRITERIA</span><span>{view?.criteria.length.toString().padStart(2,'0') ?? '—'} TOTAL</span></div><div className="criteria-list">{view?.criteria.map(c=><div className="criterion-row" key={c.id}><span className="criterion-check"><Check size={12}/></span><div><b>{c.text}</b><small>Added {c.added_at} · {c.added_by === 'mina' ? 'Mina' : c.added_by}</small></div><MoreHorizontal size={15}/></div>)}</div>
            <div className={`proposal-card ${view?.proposal_status==='approved'?'proposal-approved':view?.proposal_status==='dismissed'?'proposal-dismissed':''}`}><div className="proposal-top"><div><Sparkles size={14}/><span>PROPOSED CHANGE</span></div><span className="proposal-state">{view?.proposal_status==='open'?'NEEDS REVIEW':view?.proposal_status==='approved'?'APPROVED':view?.proposal_status==='dismissed'?'DISMISSED':'LOADING'}</span></div><h4>{view?.proposal_status==='approved'?'The standard has changed.':view?.proposal_status==='dismissed'?'The current standard stays.':'A change to the hiring bar'}</h4><p>{view?.proposal_status==='approved'?'The new criteria are active. Earlier assessments keep the version they were made on.':view?.proposal_status==='dismissed'?'The proposal was dismissed. Everyone remains on the same standard.':'Mina proposed adding two criteria to this role. Review the effect before deciding.'}</p><div className="proposal-diff">{view?.proposal?.add.map(c=><div key={c.id}><span>＋</span><b>{c.text}</b><small>ADD</small></div>)}</div>{view?.proposal_status==='open' && <><div className="impact-box"><div><span>IF APPROVED</span><button onClick={()=>setInfo(true)} aria-label="About impact preview"><Info size={12}/></button></div><div><b>{view.preview?.assessments_that_would_become_stale ?? '—'}</b><span>assessments become stale</span><b>{view.preview?.interviews_bound_to_earlier_rubric ?? '—'}</b><span>interview keeps its rubric</span></div></div><div className="proposal-actions"><button className="dismiss-button" disabled={busy || isHistorical} onClick={()=>action('/v1/roles/sept/proposals/B/dismiss','Proposal dismissed. The standard and assessments are unchanged.')}>Dismiss <kbd>D</kbd></button><button className="approve-button" disabled={busy || isHistorical} onClick={()=>action('/v1/roles/sept/proposals/B/approve','Standard B2 is active. Earlier assessments are now stale.')}>Approve change <kbd>A</kbd><ArrowUpRight size={14}/></button></div></>}{view && view.proposal_status!=='open' && <button className="proposal-followup" disabled={busy || isHistorical} onClick={()=>action('/v1/roles/sept/reset','The synthetic scenario has been reset.')}>Try the other path <ArrowRight size={13}/></button>}</div>
            <div className="rubric-note"><span className="rubric-icon"><LockKeyhole size={14}/></span><div><b>Priya’s interview is in progress</b><p>Its rubric stays on standard {view?.interviews[0]?.rubric_version ?? 'A'}, even when the bar changes.</p></div></div></div></section>
          <section className="record-panel report-panel" id="report-panel"><div className="panel-heading"><div className="panel-heading-left"><span className="panel-number">03</span><div><h3>The report</h3><p>What the record can support</p></div></div><span className="report-live"><span/> {isHistorical?'PAST':'LIVE'}</span></div><div className="report-body"><div className="report-paper"><div className="paper-head"><span>HIRING RECORD &nbsp; / &nbsp; SEPTEMBER</span><FileCheck2 size={15}/></div><div className="paper-title-row"><div><span>ROLE REPORT</span><h4>The evidence<br/>behind the bar<span>.</span></h4></div><span className="paper-version">VERSION<br/><b>{view?.standard_id ?? '—'}</b></span></div><p className="paper-intro">The observation stays visible. The explanation is checked against the record.</p><div className="report-rule"/><div className="claim-list">{claims.map((claim, index)=><ClaimCard claim={claim} index={index} key={claim.criterion_id}/>)}</div><button className="show-report-details" onClick={()=>setShowAllClaims(!showAllClaims)}>{showAllClaims?'Hide operation checks':'View operation checks'} <span>{ops.length.toString().padStart(2,'0')}</span><ChevronDown size={13} className={showAllClaims?'flip':''}/></button>{showAllClaims && <div className="operation-checks">{ops.map(line=><div key={line.criterion_id}><div><b>{line.text}</b><small>{line.naive}</small></div><span className={line.eligible?'check-good':'check-no'}>{line.eligible?'SUPPORTED':codeName(line.refusal)}</span><p>{line.reason}</p></div>)}</div>}<div className="paper-end"><span>END OF REPORT</span><span>ENGINE EVALUATED &nbsp; ●</span></div></div><div className="report-caption"><ShieldCheck size={15}/><span>Observations are not proof of prediction.</span><button onClick={()=>setInfo(true)}>Why? <ArrowUpRight size={12}/></button></div></div></section>
        </div>
        {view && <WorkflowWorkbench view={view} busy={busy} historical={isHistorical} execute={(command,message)=>action(`/v1/roles/${view.report.role_id}/commands`,message,command)}/>}
        <section className="action-section"><div className="action-section-head"><div><div className="overline">THE EFFECT LANE &nbsp; / &nbsp; 02</div><h2>Where things stand<span>.</span></h2><p>Actions, acknowledgements and the distinction between the two.</p></div><div className="action-section-label">CONNECTED CONTEXT <span>↗</span></div></div><div className="effect-grid"><div className="effect-card"><div className="effect-top"><span><Radio size={15}/> NORTHWIND</span><span className={view?.receipts[0]?.acknowledged?'effect-status done':'effect-status'}>{!view?.receipts.length?'NO REQUEST':view.receipts[0].acknowledged?'ACKNOWLEDGED':'AWAITING ACK'}</span></div><h3>{!view?.receipts.length?'No receipt yet':view.receipts[0].acknowledged?'Acknowledgement recorded':'Waiting for acknowledgement'}</h3><p>{!view?.receipts.length?'Approving the proposal creates a simulated receipt.':view.receipts[0].acknowledged?'The simulated system acknowledged this receipt.':'The action remains pending until the simulated system responds.'}</p><div className="effect-bottom"><span>WRITE CAPABILITY <b>NONE</b></span><button onClick={()=>action('/v1/receipts/nw-B2/ack','Northwind acknowledgement recorded. Sync is still unsupported.')} disabled={!view?.receipts.length || view.receipts[0].acknowledged || busy || isHistorical}>Simulate ack <ArrowUpRight size={13}/></button></div></div><div className="effect-card"><div className="effect-top"><span><Clock3 size={15}/> INTERVIEW HOLD</span><span className={view?.hold_confirmed?'effect-status done':'effect-status'}>{view?.hold_confirmed?'CONFIRMED':'UNCONFIRMED'}</span></div><h3>{view?.hold_confirmed?'The hold is confirmed':'A hold is not a booking'}</h3><p>{view?.hold_confirmed?'A human confirmation has been recorded for the debrief.':'The debrief has a proposed slot, but no confirmation yet.'}</p><div className="effect-bottom"><span>REPORT CHECK <b>{view?.hold_confirmed?'SUPPORTED':'R-UNCONFIRMED'}</b></span><button onClick={()=>action('/v1/holds/debrief/confirm','The debrief hold has been confirmed.')} disabled={!view || view.hold_confirmed || busy || isHistorical}>Confirm hold <ArrowUpRight size={13}/></button></div></div><div className="effect-card effect-card-dark"><div className="effect-top"><span><GitBranch size={15}/> COUNTERFACTUAL</span><span className="effect-status">SANDBOX</span></div><h3>What if we chose differently?</h3><p>Compare the other path without changing the live record.</p><div className="effect-bottom"><span>LIVE WRITES <b>ZERO</b></span><button onClick={compareBranch} disabled={!view || busy || isHistorical}>Compare paths <ArrowUpRight size={13}/></button></div></div><div className="effect-card"><div className="effect-top"><span><Users size={15}/> CLAIM SUPPORT</span><span className={view?.support_present?'effect-status done':'effect-status'}>{view?.support_present?'PRESENT':'REMOVED'}</span></div><h3>{view?.support_present?'Support is part of the record':'The claim lost its support'}</h3><p>{view?.support_present?'Mina supports the original criterion. See what happens if that support is removed.':'The historical count remains, but the supported claim is refused.'}</p><div className="effect-bottom"><span>REPORT CHECK <b>{view?.support_present?'SUPPORTED':'R-SUPPORT-GONE'}</b></span><button onClick={()=>action('/v1/support/mina/remove','Support removed. The historical count remains visible.')} disabled={!view || !view.support_present || busy || isHistorical}>Remove support <ArrowUpRight size={13}/></button></div></div></div></section>
        <section className="timeline-section" id="timeline"><div className="timeline-heading"><div><div className="overline">THE TIMELINE &nbsp; / &nbsp; 03</div><h2>Every change leaves a trail<span>.</span></h2><p>Move through the record. Earlier states remain exactly where they were.</p></div><button className="quiet-button" onClick={()=>setShowEvents(!showEvents)}><Braces size={15}/> {showEvents?'Hide event log':'Inspect event log'}</button></div><div className="timeline-card"><div className="timeline-card-top"><span><span className="green-dot"/> RECORD HISTORY</span><span>EVENT {view?.at ?? 0} OF {view?.live_total ?? 0}</span></div><div className="scrubber-row"><button onClick={()=>view && load(Math.max(0, view.at-1))} disabled={!view || view.at===0} aria-label="Previous event"><ChevronLeft size={17}/></button><div className="scrubber-track"><input aria-label="Record history" type="range" min="0" max={view?.live_total ?? 0} value={view?.at ?? 0} onChange={e=>load(Number(e.target.value))}/><div className="scrubber-labels"><span>RECORD OPENED</span><span>{(view?.live_total ?? 0)>0?`${view?.live_total} RECORDED EVENT${view?.live_total===1?'':'S'}`:'NO CHANGES YET'}</span><span>LIVE NOW</span></div></div><button onClick={()=>view && load(Math.min(view.live_total,view.at+1))} disabled={!view || view.at===view.live_total} aria-label="Next event"><ChevronRight size={17}/></button></div><div className="timeline-help"><span>USE <kbd>[</kbd> <kbd>]</kbd> TO MOVE THROUGH HISTORY</span><span><LockKeyhole size={12}/> HASH-CHAINED EVENT HISTORY</span></div></div>{showEvents && <div className="event-log"><div className="event-log-header"><div><span>EVENT LOG</span><p>Each event includes the previous event’s hash. This demo chain is not signed.</p></div><button onClick={()=>setVerifyResult(verifyChain(liveEvents)?'valid':'invalid')}><ShieldCheck size={14}/> Check chain</button></div>{liveEvents.length ? liveEvents.map(e=><div className="event-row" key={e.seq}><span>{String(e.seq).padStart(2,'0')}</span><div><b>{labelForEvent[e.kind] ?? e.kind}</b><small>{e.payload}</small></div><code title={e.hash}>{shortHash(e.hash)}</code><button onClick={()=>load(e.seq)}>View <ArrowUpRight size={12}/></button></div>):<p className="empty-events">No events yet. Approve or dismiss the proposal to start the record.</p>}{verifyResult && <div className={`verify-message ${verifyResult}`}>{verifyResult==='valid'?<CircleCheck size={15}/>:<CircleAlert size={15}/>} {verifyResult==='valid'?'The event chain is internally consistent. It is not a signature or an independent trust anchor.':'The event chain did not pass verification.'}</div>}</div>}</section>
        <section className="developer-section"><div className="developer-heading"><div><div className="overline">FOR BUILDERS &nbsp; / &nbsp; 04</div><h2>Bring your own questions<span>.</span></h2><p>Read this synthetic record from an agent, or run the same rules through a reproducible simulation.</p></div><div className="developer-links"><a className="quiet-button" href="/developers/"><Braces size={14}/> Developer reference</a><a className="quiet-button" href="/lab/"><Play size={14}/> Simulation lab</a></div></div><div className="developer-grid"><div className="developer-card"><div className="developer-card-top"><span><Braces size={15}/> MCP SERVER</span><b>READ ONLY</b></div><h3>Ask the record.</h3><p>Connect an MCP client to the local server. The tools can read the role, people, events and refusal explanations.</p><div className="developer-code"><code>http://127.0.0.1:8080/mcp</code><button aria-label="Copy MCP endpoint" onClick={async()=>{try{await navigator.clipboard.writeText('http://127.0.0.1:8080/mcp');announce('MCP endpoint copied.')}catch{announce('Copy the endpoint from the card.')}}}><Copy size={14}/></button></div><small>LOCAL ONLY &nbsp; / &nbsp; SYNTHETIC DATA &nbsp; / &nbsp; NO WRITE TOOLS</small></div><div className="developer-card"><div className="developer-card-top"><span><GitBranch size={15}/> YOUR SCENARIO</span><b>SESSION SECURITY</b></div><h3>Your private scenario.</h3><p>Your scenario is private to this browser. MCP tools use a separate, reproducible synthetic example.</p><div className="developer-code"><code>{session.current || 'Loading session…'}</code><button aria-label="Copy security description" onClick={async()=>{try{await navigator.clipboard.writeText(session.current);announce('Scenario session ID copied.')}catch{announce('Copy the session ID from the card.')}}}><Copy size={14}/></button></div><small>PERSISTENCE DEPENDS ON SERVER CONFIGURATION</small></div></div></section>
        <footer className="studio-footer"><span><Mark/> HIRING RECORD</span><p>Synthetic people, role and events. A product exploration built around a real rules engine.</p><button onClick={()=>setInfo(true)}>ABOUT THIS DEMO <ArrowUpRight size={12}/></button></footer>
      </div></main></div>
      {activePerson && <PersonDrawer person={activePerson} standard={view?.standard_id ?? 'A'} onClose={()=>setActivePerson(null)}/>}
      {compare && <CompareDialog comparison={compare} current={view?.standard_id ?? 'A'} engine={compareEngine} onClose={()=>setCompare(null)}/>}
      {info && <InfoDialog onClose={()=>setInfo(false)}/>}
    </div>;
}

function ClaimCard({claim,index}:{claim:Claim;index:number}) { return <div className="claim-card"><div className="claim-top"><span>0{index+1} / {claim.text.toUpperCase()}</span><span>{claim.eligible?'CHECKED':'REFUSED'}</span></div><div className="claim-observation"><strong>{claim.hires_met} <span>/ {claim.hires_total}</span></strong><div><b>hires met this criterion</b><small>{claim.text}</small></div></div><div className={`claim-verdict ${claim.eligible?'supported':'refused'}`}><span>{claim.eligible?<Check size={12}/>:<span>!</span>}</span><div><b>{claim.eligible?'The record supports this observation':'Prediction refused'}</b><small>{claim.eligible?'DESCRIPTIVE CHECK':codeName(claim.refusal)}</small></div></div><p>{claim.reason}</p></div>; }

function PersonDrawer({person,standard,onClose}:{person:Person;standard:string;onClose:()=>void}) { const dialog = useDialogFocus<HTMLElement>(onClose); return <div className="overlay" onMouseDown={onClose}><aside ref={dialog} className="person-drawer" role="dialog" aria-modal="true" aria-label={`${person.name} evidence`} onMouseDown={e=>e.stopPropagation()}><div className="drawer-top"><span>PERSON RECORD &nbsp; / &nbsp; SYNTHETIC</span><button onClick={onClose} aria-label="Close person details"><X size={18}/></button></div><div className="drawer-person"><span className="person-avatar tone-2">{initials(person.name)}</span><div><h2>{person.name}</h2><p>{displayStage(person.place)} · September role</p></div></div><div className="drawer-status"><span className={person.stale?'is-stale':''}><i/> {person.stale?'ASSESSMENT STALE':person.incomplete?'ASSESSMENT INCOMPLETE':'ASSESSMENT CURRENT'}</span><span>ACTIVE STANDARD {standard}</span></div><div className="drawer-section"><div className="section-micro"><span>ASSESSMENT</span><span>01</span></div><h3>{person.assessment?.criterion.replaceAll('_',' ') ?? 'No assessment recorded'}</h3><p>{person.assessment ? person.assessment.met ? 'The assessment records this criterion as met.' : 'The assessment records this criterion as not met.' : 'No assessment is recorded.'}</p><div className="source-box"><span><FileCheck2 size={15}/> SOURCE REFERENCE</span><b>{person.source?.kind ?? 'No matching source moment'} {person.source ? `· ${person.source.timestamp}` : ''}</b>{person.source && <blockquote>“{person.source.quote}”</blockquote>}<small>{person.source ? `${person.source.id} · synthetic transcript` : 'The assessment’s source could not be found for this person.'}</small></div><div className="drawer-detail"><span>ASSESSED AGAINST</span><b>Standard {person.assessment?.version ?? '—'}</b></div><div className="drawer-detail"><span>ASSESSMENT ID</span><b>{person.assessment?.id ?? '—'}</b></div></div><div className="drawer-section"><div className="section-micro"><span>OTHER RECORDED EVIDENCE</span><span>{person.evidence.length.toString().padStart(2,'0')}</span></div>{person.evidence.map(e=><div className="drawer-evidence" key={e.criterion}><span className={e.met?'evidence-yes':'evidence-no'}>{e.met?<Check size={12}/>:<X size={12}/>}</span><span>{e.criterion.replaceAll('_',' ')}</span><small>{e.met?'MET':'NOT MET'}</small></div>)}</div><div className="drawer-section"><div className="section-micro"><span>CONTEXT</span></div><div className="drawer-detail"><span>DECISION</span><b>{person.decision ? `${person.decision.outcome} · ${person.decision.at}` : 'No decision yet'}</b></div><div className="drawer-detail"><span>INTERVIEW RUBRIC</span><b>{person.interview ? `Standard ${person.interview.rubric_version} · ${person.interview.status}` : 'No open interview'}</b></div></div><div className="drawer-bottom">SYNTHETIC RECORD <span>NO REAL CANDIDATE DATA</span></div></aside></div>; }

function CompareDialog({comparison,current,engine,onClose}:{comparison:Comparison;current:string;engine:'browser'|'server';onClose:()=>void}) { const dialog = useDialogFocus<HTMLElement>(onClose); const report = comparison.report; const lines=report.lines.filter(l=>!l.criterion_id.startsWith('op-')); return <div className="overlay modal-overlay" onMouseDown={onClose}><section ref={dialog} className="compare-dialog" role="dialog" aria-modal="true" aria-labelledby="compare-heading" onMouseDown={e=>e.stopPropagation()}><div className="dialog-top"><span><GitBranch size={14}/> COUNTERFACTUAL SANDBOX · {engine === 'browser' ? 'BROWSER WASM' : 'SERVER ENGINE'}</span><button onClick={onClose} aria-label="Close comparison"><X size={18}/></button></div><h2 id="compare-heading">See the other path<span>.</span></h2><p>This preview evaluates the {comparison.path === 'dismissed' ? 'dismissed' : 'approved'} proposal in an isolated branch. It does not change the live record.</p><div className="compare-versions"><div><span>LIVE STANDARD</span><b>{current}</b><small>Your current record</small></div><ArrowRight size={20}/><div><span>SIMULATED STANDARD</span><b>{report.standard_id}</b><small>Hypothetical {comparison.path === 'dismissed' ? 'dismissal' : 'approval'} · {comparison.stale_assessments} stale</small></div></div><div className="compare-list"><div className="section-micro"><span>IF THE PROPOSAL IS {comparison.path.toUpperCase()}</span><span>REPORT CHECKS</span></div>{lines.map(line=><div key={line.criterion_id}><span>{line.text}<small>{line.naive}</small></span><b className={line.eligible?'check-good':'check-no'}>{line.eligible?'SUPPORTED':codeName(line.refusal)}</b></div>)}</div>{comparison.omitted_events.length > 0 && <div className="compare-exclusions"><b>Not replayed on this path</b>{comparison.omitted_events.map(note=><p key={note}>{note}</p>)}</div>}<div className="compare-foot"><span><LockKeyhole size={13}/> NO LIVE WRITES</span><button onClick={onClose}>Back to the record <ArrowRight size={14}/></button></div></section></div>; }

function InfoDialog({onClose}:{onClose:()=>void}) { const dialog = useDialogFocus<HTMLElement>(onClose); return <div className="overlay modal-overlay" onMouseDown={onClose}><section ref={dialog} className="compare-dialog info-dialog" role="dialog" aria-modal="true" aria-labelledby="info-heading" onMouseDown={e=>e.stopPropagation()}><div className="dialog-top"><span><Info size={14}/> ABOUT THE SCENARIO</span><button onClick={onClose} aria-label="Close about dialog"><X size={18}/></button></div><h2 id="info-heading">A record you can question<span>.</span></h2><p>This is a synthetic, interactive scenario. This browser has an isolated synthetic scenario protected by an expiring cookie. The server can use in-memory or PostgreSQL storage.</p><div className="info-points"><div><span>01</span><p><b>The count is descriptive.</b> Three of three hires meeting a criterion is not, by itself, proof that the criterion predicted success.</p></div><div><span>02</span><p><b>Changes keep their context.</b> Approval changes the standard, while existing assessments and interview bindings keep their original versions.</p></div><div><span>03</span><p><b>Capabilities are explicit.</b> Northwind is simulated, report bundles are signed with a persisted demo key, and external sending or calendar booking is not connected.</p></div></div><div className="compare-foot"><span><ShieldCheck size={13}/> SYNTHETIC DATA ONLY</span><button onClick={onClose}>Got it <Check size={14}/></button></div></section></div>; }

function CommandPalette({onClose,onCompare,onApprove,onDismiss,onTimeline,canDecide,canCompare}:{onClose:()=>void;onCompare:()=>void;onApprove:()=>void;onDismiss:()=>void;onTimeline:()=>void;canDecide:boolean;canCompare:boolean}) {
  const dialog=useDialogFocus<HTMLElement>(onClose);
  return <div className="overlay modal-overlay" onMouseDown={onClose}><section className="command-palette" ref={dialog} role="dialog" aria-modal="true" aria-labelledby="palette-title" onMouseDown={e=>e.stopPropagation()}><div className="dialog-top"><h2 id="palette-title">Commands &amp; shortcuts</h2><button onClick={onClose} aria-label="Close commands"><X size={18}/></button></div><p>Explore the synthetic record using the keyboard or these controls.</p><div className="command-list"><button disabled={!canDecide} onClick={onApprove}>Approve proposal <kbd>A</kbd></button><button disabled={!canDecide} onClick={onDismiss}>Dismiss proposal <kbd>D</kbd></button><button disabled={!canCompare} onClick={onCompare}>Compare alternate path</button><button onClick={onTimeline}>Go to timeline <kbd>[ ]</kbd></button><a href="/report/">View printable report</a><a href="/verify/">Verify a bundle</a><a href="/lab/">Open simulation lab</a><a href="/developers/">Developer reference</a></div></section></div>;
}
