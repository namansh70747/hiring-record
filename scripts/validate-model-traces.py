#!/usr/bin/env python3
"""Replay the simulator's supported approve/dismiss/ack traces in the Quint abstraction."""
import json, pathlib, subprocess, tempfile
root=pathlib.Path(__file__).resolve().parent.parent
model=(root/'spec/record.qnt').read_text().rstrip()
assert model.endswith('}')
runs=[]
for seed in (1,42,2026):
    result=json.loads(subprocess.check_output(['cargo','run','-q','-p','record-cli','--','simulate-json',str(seed)],cwd=root))
    for step in result['trace']:
        action='approve' if step['path']=='approved' else 'dismiss'
        ack='.then(acknowledge)' if step['acknowledged'] else ''
        stale='true' if step['stale'] else 'false'
        ack_value='true' if step['acknowledged'] else 'false'
        mode='2' if action=='approve' else '1'
        runs.append(f'  run seed{seed}round{step["round"]}Test = init.then({action}){ack}.then(all {{ assert(proposal == {mode}), assert(stale == {stale}), assert(ack == {ack_value}), assert(rubric == 0), proposal\' = proposal, stale\' = stale, ack\' = ack, rubric\' = rubric, hold\' = hold, support\' = support, beforeHire\' = beforeHire, allRejectsMet\' = allRejectsMet, bothGroups\' = bothGroups, hasSyncOperation\' = hasSyncOperation, predicted\' = predicted, synced\' = synced }})')
with tempfile.TemporaryDirectory() as directory:
    path=pathlib.Path(directory)/'traces.qnt'
    path.write_text(model[:-1]+'\n'+'\n'.join(runs)+'\n}\n')
    completed=subprocess.run(['npx','--yes','@informalsystems/quint@0.33.1','test',str(path),'--backend=typescript','--max-samples=1'],cwd=root,check=True,text=True,capture_output=True)
    if completed.stdout.count('seed') < len(runs):
        raise SystemExit('Quint did not report every generated trace test: '+completed.stdout)
print(f'PASS: {len(runs)} simulator traces replayed through the bounded Quint abstraction')
