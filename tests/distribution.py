#!/usr/bin/env python3
"""Exercise real engine RPC and crash-safe X receipts with local provider doubles."""
import json
import os
from pathlib import Path
import subprocess
import tempfile
import unittest

ROOT=Path(__file__).resolve().parents[1]
ENGINE=ROOT/'target/debug/pressroom'

class DistributionTests(unittest.TestCase):
    def setUp(self):
        self.temp=tempfile.TemporaryDirectory()
        self.addCleanup(self.temp.cleanup)
        self.root=Path(self.temp.name)
        self.pub=self.root/'publication'
        self.bin=self.root/'bin';self.bin.mkdir()
        self.calls=self.root/'calls'
        self.env=dict(os.environ,PATH=str(self.bin)+os.pathsep+os.environ['PATH'],PRESSROOM_TEST_CALLS=str(self.calls),XDG_STATE_HOME=str(self.root/'state'))
        self.tool('secret-tool',"print('test-user-token')")
        self.tool('curl',"""import os,sys,json
from pathlib import Path
p=Path(os.environ['PRESSROOM_TEST_CALLS'])
with p.open('a') as f:f.write(sys.argv[-1]+'\\n')
if os.environ.get('PRESSROOM_TEST_FAILURE'):sys.exit(28)
if '/draft' in sys.argv[-1]:
    payload=json.loads(Path(sys.argv[sys.argv.index('--data-binary')+1][1:]).read_text())
    Path(str(p)+'.payload').write_text(json.dumps(payload))
print(json.dumps({'data':{'id':'123'}} if sys.argv[-1].endswith('/draft') or sys.argv[-1].endswith('/upload') else {'data':{'post_id':'456'}}))
""")
        self.rpc('init',name='Test publication',author='Test author',base_url='https://example.com')
        p=self.pub/'content/today-in-omarchy/story.md'
        p.parent.mkdir(parents=True,exist_ok=True)
        p.write_text('''---
schema: 1
id: 018fc5c0-2f54-7ea8-aeb2-75e566f38345
title: A test article
series: today-in-omarchy
status: ready
published_at: 2026-01-02T21:00:00+00:00
summary: Testing distribution
slug: story
---
A plain paragraph with unicode 🌍.

Another paragraph.
''')
        self.args=dict(article='content/today-in-omarchy/story.md',expected_source_hash=self.rpc('inspect')['source_hash'])
    def tool(self,name,body):
        p=self.bin/name;p.write_text('#!/usr/bin/env python3\n'+body+'\n');p.chmod(0o755)
    def rpc(self,command,expect=True,**args):
        result=subprocess.run([str(ENGINE),'rpc'],input=json.dumps(dict(schema=1,command=command,path=str(self.pub),args=args)),text=True,capture_output=True,env=self.env,timeout=10)
        data=json.loads(result.stdout)
        self.assertEqual(data['ok'],expect,data)
        return data.get('result') if expect else data['error']
    def native(self,command,expect=True,origin=None,**args):
        import struct
        message=json.dumps(dict(command=command,**args)).encode()
        origin=origin or (ROOT/'companion/origin.txt').read_text().strip()
        result=subprocess.run([str(ENGINE),'native-message',origin],input=struct.pack('=I',len(message))+message,capture_output=True,env=self.env,timeout=10)
        self.assertEqual(result.returncode,0,result.stderr)
        length=struct.unpack('=I',result.stdout[:4])[0]
        self.assertEqual(len(result.stdout[4:]),length)
        data=json.loads(result.stdout[4:]);self.assertEqual(data['ok'],expect,data)
        return data.get('result') if expect else data['error']
    def test_substack_outbox_is_private_stale_safe_and_explicitly_confirmed(self):
        p=self.rpc('substack-prepare',**self.args);id=p['outbox_id']
        self.assertEqual(self.rpc('distribution-plan',**self.args)['targets'][2]['status'],'awaiting_browser')
        self.assertEqual(self.native('list')['items'][0]['id'],id)
        import base64
        bundle=json.loads(base64.b64decode(self.native('read',id=id)['data']))
        self.assertEqual(bundle['title'],'A test article')
        self.assertNotIn('root',bundle)
        self.assertEqual((self.root/'state/pressroom'/f'{id}.json').stat().st_mode & 0o777,0o600)
        self.native('confirm',id=id,url='https://author.substack.com/p/story')
        self.assertEqual(self.rpc('distribution-plan',**self.args)['targets'][2]['status'],'confirmed_by_user')
        self.assertIn('already recorded',self.rpc('substack-prepare',expect=False,**self.args))
        story=self.pub/self.args['article'];story.write_text(story.read_text()+'\nA correction.\n')
        self.native('read',expect=False,id=id)
        self.native('remove',id=id)
        self.assertEqual(self.native('list')['items'],[])
        self.assertEqual(self.routes(),[])
    def test_substack_only_batch_and_unknown_native_operations(self):
        p=self.rpc('distribution-publish',website=False,x=False,substack=True,**self.args)
        self.assertEqual(p['results'][0]['result']['status'],'awaiting_browser')
        id=p['results'][0]['result']['outbox_id']
        self.native('read',expect=False,id='../publication/publication.toml')
        self.native('x-connect',expect=False,id=id)
        self.native('confirm',expect=False,id=id,url='http://author.substack.com/p/story')
    def test_oauth_loopback_flow_refresh_and_disconnect(self):
        secret=self.root/'keyring'
        self.env['PRESSROOM_TEST_SECRET']=str(secret)
        self.tool('secret-tool',"""import os,sys
from pathlib import Path
p=Path(os.environ['PRESSROOM_TEST_SECRET'])
if sys.argv[1]=='store':p.write_text(sys.stdin.read())
elif sys.argv[1]=='clear':p.unlink()
else:print(p.read_text())
""")
        self.tool('xdg-open',"""import socket,sys,urllib.parse
url=urllib.parse.urlparse(sys.argv[1]);q=urllib.parse.parse_qs(url.query)
assert q['code_challenge_method']==['S256']
assert len(q['state'][0])==43
request='GET /callback?code=fixture-code&state='+q['state'][0]+' HTTP/1.1\\r\\nHost: 127.0.0.1:39123\\r\\n\\r\\n'
s=socket.create_connection(('127.0.0.1',39123));s.sendall(request.encode());s.close()
""")
        self.tool('curl',"""import json,sys
if sys.argv[-1].endswith('/token'):
    data=sys.stdin.read();assert 'grant_type=' in data
    print(json.dumps(dict(access_token='fixture-access',refresh_token='fixture-refresh',expires_in=7200)))
elif sys.argv[-1].endswith('/users/me'):print(json.dumps({'data':{'id':'42','username':'fixture'}}))
else:print(json.dumps({'data':{'id':'123'}}))
""")
        result=self.rpc('x-connect',client_id='fixture-client')
        self.assertEqual(result['username'],'fixture')
        self.assertNotIn('access_token',result)
        bundle=json.loads(secret.read_text());bundle['expires_at']=0;secret.write_text(json.dumps(bundle))
        self.rpc('x-draft',**self.args)
        self.assertGreater(json.loads(secret.read_text())['expires_at'],0)
        self.assertEqual(self.rpc('x-disconnect')['connected'],False)
        self.rpc('x-status',expect=False)

    def test_outbox_chunks_reassemble_unicode_without_truncation(self):
        story=self.pub/self.args['article'];story.write_text(story.read_text()+'\n'+('🌍 '*60000)+'\n')
        self.args['expected_source_hash']=self.rpc('inspect')['source_hash']
        id=self.rpc('substack-prepare',**self.args)['outbox_id']
        import base64,hashlib
        chunks=[];offset=0
        while True:
            part=self.native('read',id=id,offset=offset);chunks.append(base64.b64decode(part['data']));offset=part['next']
            if offset==part['total']:break
        self.assertGreater(len(chunks),1)
        data=b''.join(chunks);self.assertEqual(hashlib.sha256(data).hexdigest(),part['hash'])
        self.assertIn('🌍 '*100,json.loads(data)['html'])
    def test_x_draft_cannot_be_published_with_another_account(self):
        self.rpc('x-draft',**self.args)
        self.tool('secret-tool',"print('another-user-token')")
        self.assertIn('different or legacy connection',self.rpc('x-publish',expect=False,**self.args))
        self.assertEqual(len(self.routes()),1)

    def test_native_origin_is_restricted(self):
        result=subprocess.run([str(ENGINE),'native-message','chrome-extension://untrusted/'],input=b'',capture_output=True,env=self.env,timeout=10)
        self.assertNotEqual(result.returncode,0)
        self.assertEqual(result.stdout,b'')

    def routes(self):return self.calls.read_text().splitlines() if self.calls.exists() else []
    def test_prepare_publish_and_repeat_never_duplicate(self):
        p=self.rpc('x-draft',**self.args);self.assertEqual(p['targets'][1]['status'],'draft')
        self.rpc('x-draft',**self.args);self.assertEqual(len(self.routes()),1)
        p=self.rpc('x-publish',**self.args);self.assertEqual(p['targets'][1]['status'],'published')
        self.assertEqual(p['targets'][1]['receipt']['url'],'https://x.com/i/status/456')
        self.rpc('x-publish',**self.args)
        self.assertEqual(self.routes(),['https://api.x.com/2/articles/draft','https://api.x.com/2/articles/123/publish'])
    def test_timeout_survives_restart_and_blocks_retry(self):
        self.env['PRESSROOM_TEST_FAILURE']='1'
        self.rpc('x-publish',expect=False,**self.args)
        del self.env['PRESSROOM_TEST_FAILURE']
        error=self.rpc('x-publish',expect=False,**self.args)
        self.assertIn('unknown outcome',error)
        self.assertEqual(len(self.routes()),1)
        self.assertEqual(self.rpc('distribution-plan',**self.args)['targets'][1]['status'],'unknown')
    def test_failed_publish_retains_remote_draft_id(self):
        self.rpc('x-draft',**self.args)
        self.env['PRESSROOM_TEST_FAILURE']='1'
        self.rpc('x-publish',expect=False,**self.args)
        p=self.rpc('distribution-plan',**self.args)
        self.assertEqual(p['targets'][1]['receipt']['remote_id'],'123')
        self.assertEqual(p['targets'][1]['status'],'unknown')
    def test_rich_article_media_and_cover_use_provider_schema(self):
        story=self.pub/self.args['article']
        story.write_text(story.read_text().replace('slug: story','slug: story\nheader_image: /media/pixel.png')+'\n# Heading\n\n🌍 **bold** [link](https://example.com)\n\n![alt](/media/pixel.png)\n\n| A | B |\n|---|---|\n| 1 | 2 |\n\n- Item\n  - Nested\n\n```rust\nlet a=1;\n```\n')
        import base64
        (self.pub/'media/pixel.png').write_bytes(base64.b64decode('iVBORw0KGgoAAAANSUhEUgAAAAEAAAABCAQAAAC1HAwCAAAAC0lEQVR42mP8/x8AAwMCAO+aXioAAAAASUVORK5CYII='))
        self.args['expected_source_hash']=self.rpc('inspect')['source_hash']
        self.rpc('x-draft',**self.args)
        payload=json.loads(Path(str(self.calls)+'.payload').read_text())
        schema=json.loads((ROOT/'tests/contracts/x-article-schema.json').read_text())
        # Small strict validator for this frozen provider subset; no third-party dependency.
        def check(value,node):
            if '$ref' in node:
                target=schema
                for part in node['$ref'][2:].split('/'):target=target[part]
                return check(value,target)
            if 'enum' in node:self.assertIn(value,node['enum'])
            if node.get('type')=='object':
                self.assertIsInstance(value,dict)
                for key in node.get('required',[]):self.assertIn(key,value)
                if node.get('additionalProperties') is False:self.assertFalse(set(value)-set(node['properties']))
                for key,v in value.items():
                    if key in node.get('properties',{}):check(v,node['properties'][key])
            elif node.get('type')=='array':
                self.assertIsInstance(value,list)
                for v in value:check(v,node['items'])
            elif node.get('type')=='string':self.assertIsInstance(value,str)
            elif node.get('type')=='integer':self.assertIsInstance(value,int)
        check(payload,schema)
        self.assertEqual(payload['cover_media']['media_id'],'123')
        self.assertEqual(self.routes().count('https://api.x.com/2/media/upload'),1)
        self.assertNotIn('media/pixel.png',json.dumps(payload))

    def test_selected_x_destination_reports_result(self):
        p=self.rpc('distribution-publish',website=False,x=True,**self.args)
        self.assertEqual(p['distribution']['targets'][1]['status'],'published')
        self.assertEqual(len(p['results']),1)
        self.assertEqual(p['results'][0]['target'],'x')

if __name__=='__main__':unittest.main(verbosity=2)
