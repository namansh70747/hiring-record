#!/usr/bin/env python3
"""Exercise the real Postgres outbox -> Restate -> synthetic Northwind -> API callback path."""
import http.cookiejar, json, os, subprocess, time, urllib.request, uuid
base=os.environ.get('RECORD_BASE_URL','http://127.0.0.1:8080').rstrip('/')
client=urllib.request.build_opener(urllib.request.HTTPCookieProcessor(http.cookiejar.CookieJar()))
def request(path,method='GET',headers=None):
    with client.open(urllib.request.Request(base+path,method=method,headers=headers or {}),timeout=8) as response:
        return json.load(response)
initial=request('/v1/roles/sept/view')
approved=request('/v1/roles/sept/proposals/B/approve','POST',{'idempotency-key':uuid.uuid4().hex,'if-match':initial['version']})
assert not approved['receipts'][0]['acknowledged'], 'receipt must start pending'
if os.environ.get('RESTART_NORTHWIND') == '1':
    subprocess.run(['docker','compose','-p','hiring-record','restart','northwind'],check=True,stdout=subprocess.DEVNULL)
for _ in range(45):
    time.sleep(1)
    state=request('/v1/roles/sept/view')
    if state['receipts'][0]['acknowledged']:
        assert len([e for e in state['events'] if e['kind']=='ReceiptAcknowledged'])==1
        print('PASS: durable synthetic receipt acknowledged exactly once, including retry/restart')
        break
else:
    raise SystemExit('FAIL: workflow remained pending: '+json.dumps(request('/v1/workflows')))
