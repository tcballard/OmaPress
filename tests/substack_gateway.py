#!/usr/bin/env python3
"""Validate the optional gateway contract without contacting a Substack account."""
from distribution import DistributionTests
import datetime as dt
import json

class GatewayTests(DistributionTests):
    def setUp(self):
        super().setUp()
        self.config=self.root/'gateway.json'
        self.config.write_text(json.dumps(dict(gateway_url='http://127.0.0.1:5001',publication_url='https://test.substack.com',substack_sid='secret-session',connect_sid='')))
        self.config.chmod(0o600)
        self.env['PRESSROOM_SUBSTACK_GATEWAY_CREDENTIALS_FILE']=str(self.config)
        self.tool('curl',"""import os,sys,json
from pathlib import Path
p=Path(os.environ['PRESSROOM_TEST_CALLS'])
url=sys.argv[-1];method=sys.argv[sys.argv.index('--request')+1]
with p.open('a') as f:f.write(method+' '+url+'\\n')
payload=json.loads(Path(sys.argv[sys.argv.index('--data-binary')+1][1:]).read_text()) if '--data-binary' in sys.argv else None
if os.environ.get('GATEWAY_FAIL') and method=='POST' and url.endswith('/schedule'):sys.exit(28)
if url.endswith('/drafts') and method=='POST':
    Path(str(p)+'.draft').write_text(json.dumps(payload));print(json.dumps({'id':42,'uuid':'test'}))
elif url.endswith('/prepublish'):print(json.dumps({'errors':['blocked'] if os.environ.get('GATEWAY_BLOCK') else [],'suggestions':[]}))
elif url.endswith('/schedule'):
    if method=='POST':print(json.dumps(payload))
else:
    draft=json.loads(Path(str(p)+'.draft').read_text())
    if os.environ.get('GATEWAY_EDITED'):draft['body']+=' changed in browser'
    print(json.dumps(draft))
""")
    def draft(self):return self.rpc('substack-gateway-draft',**self.args)['receipt']
    def schedule(self,r,expect=True):
        return self.rpc('substack-gateway-schedule',expect=expect,**self.args,at='2030-01-01T12:00:00+00:00',timezone='Europe/London',post_audience='everyone',email_audience='everyone',reviewed_draft_hash=r['review_hash'],reviewed_in_substack=True)
    def test_draft_schedule_cancel_and_no_duplicate(self):
        r=self.draft();self.assertEqual(r['status'],'gateway_draft');self.draft()
        self.assertEqual(self.calls.read_text().count('POST http://127.0.0.1:5001/api/v1/drafts\n'),1)
        v=self.schedule(r);self.assertEqual(v['receipt']['status'],'scheduled')
        calls=self.calls.read_text();self.schedule(r);self.assertEqual(self.calls.read_text(),calls)
        v=self.rpc('substack-gateway-cancel',**self.args);self.assertEqual(v['receipt']['status'],'gateway_draft')
        self.assertIn('DELETE ',self.calls.read_text())
    def test_uncertain_schedule_never_replayed(self):
        r=self.draft();self.env['GATEWAY_FAIL']='1';self.schedule(r,False)
        calls=self.calls.read_text();self.schedule(r,False);self.assertEqual(calls,self.calls.read_text())
        self.assertEqual(self.rpc('distribution-plan',**self.args)['targets'][2]['status'],'gateway_unknown')
    def test_remote_edit_and_prepublish_errors_block(self):
        r=self.draft();self.env['GATEWAY_EDITED']='1'
        self.assertIn('changed after review',self.schedule(r,False));del self.env['GATEWAY_EDITED']
        self.env['GATEWAY_BLOCK']='1';self.assertIn('pre-publish',self.schedule(r,False))
        self.assertNotIn('/schedule',self.calls.read_text())
    def test_changed_connection_and_invalid_gateway_rejected(self):
        self.draft();c=json.loads(self.config.read_text());c['publication_url']='https://other.substack.com';self.config.write_text(json.dumps(c))
        self.assertIn('another connection',self.rpc('substack-gateway-draft',expect=False,**self.args))
        c['gateway_url']='http://untrusted.example';self.config.write_text(json.dumps(c))
        self.assertIn('HTTPS',self.rpc('substack-gateway-status',expect=False))
    def test_local_artwork_embedded_without_remote_fetch(self):
        media=self.pub/'media';media.mkdir(exist_ok=True);(media/'cover.png').write_bytes(b'\x89PNG\r\n\x1a\nimage')
        with (self.pub/self.args['article']).open('a') as f:f.write('\n![Artwork](/media/cover.png)\n')
        self.args['expected_source_hash']=self.rpc('inspect')['source_hash'];self.draft()
        body=json.loads(self.calls.with_suffix('.draft').read_text())['body']
        self.assertIn('data:image/png;base64,',body);self.assertNotIn('/media/cover.png',body)
    def test_secret_file_permissions_and_response_redaction(self):
        status=self.rpc('substack-gateway-status');self.assertNotIn('secret-session',json.dumps(status))
        self.config.chmod(0o644);self.assertIn('owner-only',self.rpc('substack-gateway-status',expect=False))

if __name__=='__main__':
    import unittest
    suite=unittest.TestSuite(GatewayTests(name) for name in GatewayTests.__dict__ if name.startswith('test_'))
    result=unittest.TextTestRunner(verbosity=2).run(suite)
    raise SystemExit(not result.wasSuccessful())
