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
        self.env=dict(os.environ,PATH=str(self.bin)+os.pathsep+os.environ['PATH'],PRESSROOM_TEST_CALLS=str(self.calls))
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
