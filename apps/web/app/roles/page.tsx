'use client';
import { useEffect, useRef, useState } from 'react';

type RoleCard = { id: string };
const today = () => new Date().toISOString().slice(0, 10);

export default function RolesPage() {
  const [roles, setRoles] = useState<RoleCard[]>([]);
  const [error, setError] = useState('');
  const [pending, setPending] = useState(false);
  const [name, setName] = useState('');
  const [roleId, setRoleId] = useState('');
  const [criterion, setCriterion] = useState('');
  const draft = useRef<{ roleId: string; standardId: string; criterionId: string; key: string } | null>(null);

  useEffect(() => {
    void fetch('/v1/roles', { credentials: 'same-origin', cache: 'no-store' })
      .then(async response => { if (!response.ok) throw new Error('Could not load your roles.'); return response.json(); })
      .then((data: { role_ids: string[] }) => setRoles(data.role_ids.map(id => ({ id }))))
      .catch((err: unknown) => setError(err instanceof Error ? err.message : 'Could not load your roles.'));
  }, []);

  function ensureDraft() {
    if (!draft.current || draft.current.roleId !== roleId) {
      draft.current = { roleId, standardId: 'v1', criterionId: 'core', key: crypto.randomUUID() };
    }
    return draft.current;
  }

  async function create(event: React.FormEvent) {
    event.preventDefault();
    const current = ensureDraft();
    setPending(true); setError('');
    const date = today();
    try {
      const response = await fetch('/v1/roles', {
        method: 'POST', credentials: 'same-origin', cache: 'no-store',
        headers: { 'Content-Type': 'application/json', 'If-Match': '0:0', 'Idempotency-Key': current.key },
        body: JSON.stringify({ role_id: current.roleId, standard: { id: current.standardId, effective_at: date, criteria: [{ id: current.criterionId, text: criterion, added_at: date, added_by: name || 'presenter' }] } }),
      });
      if (!response.ok) throw new Error(response.status === 409 ? 'That role ID is already in this session. Choose another.' : 'The role was not created.');
      window.location.assign(`/record/?role=${encodeURIComponent(current.roleId)}`);
    } catch (err) {
      setError(err instanceof Error ? err.message : 'The role was not created.');
      setPending(false);
    }
  }

  return <main className="roles-page"><header className="roles-header"><a href="/">hiringrecord.</a><nav aria-label="Primary"><a href="/record/">Record</a><a href="/roles/" aria-current="page">Roles</a><a href="/lab/">Simulation</a><a href="/verify/">Verify</a><a href="/developers/">Developers</a></nav></header>
    <div className="roles-wrap"><p className="roles-kicker">SYNTHETIC WORKSPACE</p><h1>Your synthetic roles</h1><p>Each role belongs only to this browser session. Opening one uses the same record workspace. Nothing here is a real candidate or employer.</p>
      {error && <div className="roles-error" role="alert">{error}</div>}
      <div className="roles-layout"><aside><h2>Role library</h2>{roles.map(role => <a key={role.id} href={role.id === 'sept' ? '/record/' : `/record/?role=${encodeURIComponent(role.id)}`}>{role.id === 'sept' ? 'September hiring record' : role.id}{role.id === 'sept' ? ' · demonstration' : ''}</a>)}
        <form onSubmit={create}><h3>Create role</h3><label>Display name<input value={name} maxLength={100} onChange={e => setName(e.target.value)} placeholder="Presenter"/></label><label>Role ID<input required pattern="[A-Za-z0-9_-]+" maxLength={100} value={roleId} onChange={e => setRoleId(e.target.value)} placeholder="e.g. product-lead"/></label><label>First criterion<input required maxLength={300} value={criterion} onChange={e => setCriterion(e.target.value)} placeholder="e.g. Built a product"/></label><button disabled={pending}>{pending ? 'Creating…' : 'Create synthetic role'}</button></form>
      </aside><section><h2>What a new role contains</h2><p>No hold, receipt, interview, or positive operational check is invented for an empty role. Add a person and a criterion in the record workspace.</p></section></div>
      <footer>Synthetic people and records only.</footer></div></main>;
}
