#!/usr/bin/env python3
"""End-to-end synthetic API, MCP, replay and bundle verification smoke test."""
import json
import os
import subprocess
import tempfile
import urllib.error
import urllib.request
import uuid
from pathlib import Path

BASE = os.environ.get('RECORD_BASE_URL', 'http://127.0.0.1:8080').rstrip('/')
SESSION = 'smoke-' + uuid.uuid4().hex
ROOT = Path(__file__).resolve().parent.parent

def request(path, method='GET', body=None, headers=None):
    data = None if body is None else json.dumps(body).encode()
    request_headers = {'x-demo-session': SESSION, **(headers or {})}
    if data is not None:
        request_headers['content-type'] = 'application/json'
    req = urllib.request.Request(BASE + path, data=data, method=method, headers=request_headers)
    try:
        with urllib.request.urlopen(req, timeout=15) as response:
            raw = response.read()
            return response.status, json.loads(raw) if raw else None
    except urllib.error.HTTPError as error:
        return error.code, None

def expect(condition, message):
    if not condition:
        raise AssertionError(message)

def main():
    status, initial = request('/v1/roles/sept/view')
    expect(status == 200 and initial['standard_id'] == 'A', 'fresh standard')
    expect(all(not p['stale'] for p in initial['people']), 'initial scores current')
    priya = next(p for p in initial['people'] if p['id'] == 'priya')
    expect(priya['source']['id'] == 'src-priya' and priya['source']['quote'], 'source moment resolves')
    _, dismissed = request('/v1/roles/sept/proposals/B/dismiss', 'POST')
    expect(dismissed['standard_id'] == 'A' and all(not p['stale'] for p in dismissed['people']), 'dismiss leaves scores current')
    status, _ = request('/v1/roles/sept/reset', 'POST')
    expect(status == 200, 'reset')
    _, approved = request('/v1/roles/sept/proposals/B/approve', 'POST')
    expect(approved['standard_id'] == 'B2' and all(p['stale'] for p in approved['people']), 'approve marks prior assessments stale')
    expect(approved['interviews'][0]['rubric_version'] == 'A', 'interview binding preserved')
    lines = {line['criterion_id']: line for line in approved['report']['lines']}
    expect(lines['led_a_launch']['naive'] == '3 of 3 hires' and lines['led_a_launch']['refusal'] == 'R-AFTER-HIRE', 'late criterion refused')
    expect(lines['still_hands_on']['refusal'] == 'R-REJECTS-MET', 'reject evidence refused')
    status, _ = request('/v1/roles/sept/proposals/B/approve', 'POST')
    expect(status == 409, 'duplicate approval rejected')
    _, earlier = request('/v1/roles/sept/view?at=0')
    expect(earlier['standard_id'] == 'A' and earlier['live_total'] == 1, 'historical replay')
    _, acknowledged = request('/v1/receipts/nw-B2/ack', 'POST')
    expect(acknowledged['receipts'][0]['acknowledged'], 'receipt acknowledgement')
    status, _ = request('/v1/receipts/nw-B2/ack', 'POST')
    expect(status == 409, 'duplicate acknowledgement rejected')
    _, without_support = request('/v1/support/mina/remove', 'POST')
    support_line = next(line for line in without_support['report']['lines'] if line['criterion_id'] == 'built_from_zero')
    expect(support_line['naive'] == '3 of 3 hires' and support_line['refusal'] == 'R-SUPPORT-GONE', 'removed support preserves count and refuses claim')
    _, before_removal = request('/v1/roles/sept/view?at=2')
    expect(next(line for line in before_removal['report']['lines'] if line['criterion_id'] == 'built_from_zero')['eligible'], 'replay retains earlier support')
    _, bundle = request('/v1/roles/sept/bundle')
    _, key = request('/v1/signing-key')
    with tempfile.TemporaryDirectory() as directory:
        path = Path(directory) / 'bundle.json'
        path.write_text(json.dumps(bundle))
        verified = subprocess.run(['cargo', 'run', '-q', '-p', 'record-cli', '--', 'verify', str(path), '--trust-key', key['public_key']], cwd=ROOT, capture_output=True, text=True)
        expect(verified.returncode == 0 and 'signature: valid' in verified.stdout, 'trusted CLI bundle verification')
        bundle['payload'] = bundle['payload'].replace('3 of 3 hires', '4 of 3 hires', 1)
        path.write_text(json.dumps(bundle))
        tampered = subprocess.run(['cargo', 'run', '-q', '-p', 'record-cli', '--', 'verify', str(path)], cwd=ROOT, capture_output=True, text=True)
        expect(tampered.returncode != 0, 'modified bundle rejected')
    headers = {'accept': 'application/json, text/event-stream', 'mcp-protocol-version': '2025-11-25'}
    _, tools = request('/mcp', 'POST', {'jsonrpc':'2.0','id':1,'method':'tools/list','params':{}}, headers)
    names = {tool['name'] for tool in tools['result']['tools']}
    expect('explain_refusal' in names and not any('approve' in name for name in names), 'read-only MCP tools')
    _, answer = request('/mcp', 'POST', {'jsonrpc':'2.0','id':2,'method':'tools/call','params':{'name':'explain_refusal','arguments':{'code':'R-AFTER-HIRE','session_id':SESSION}}}, headers)
    expect('R-AFTER-HIRE' in answer['result']['content'][0]['text'], 'MCP sees the same scenario')
    _, simulation = request('/v1/simulate?seed=42&rounds=100')
    expect(simulation['rounds'] == 100 and not simulation['failures'], 'deterministic simulation')
    print('PASS: decision paths, replay, acknowledgements, signed bundles, MCP and simulation')

if __name__ == '__main__':
    main()
