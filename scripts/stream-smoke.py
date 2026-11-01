#!/usr/bin/env python3
"""Check SSE snapshot IDs and reconnect after a committed synthetic write."""
import http.cookiejar,json,urllib.request,uuid
base='http://127.0.0.1:8080'; op=urllib.request.build_opener(urllib.request.HTTPCookieProcessor(http.cookiejar.CookieJar()))
def get(path,method='GET',headers=None):
    return op.open(urllib.request.Request(base+path,method=method,headers=headers or {}),timeout=8)
with get('/v1/roles/sept/view') as response: view=json.load(response)
old=view['version']
with get('/v1/roles/sept/proposals/B/dismiss','POST',{'Idempotency-Key':uuid.uuid4().hex,'If-Match':old}) as response: updated=json.load(response)
assert updated['version']!=old
with get('/v1/roles/sept/stream',headers={'Last-Event-ID':old}) as response:
    text=''
    while '\n\n' not in text: text+=response.readline().decode()
    assert 'id: '+updated['version'] in text and 'event: snapshot' in text, text
print('PASS: SSE reconnect returned the committed snapshot')
