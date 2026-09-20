#!/usr/bin/env python3
"""Synthetic engine benchmark. Never substitutes for the XPS acceptance gate."""
import json,os,pathlib,statistics,subprocess,tempfile,time,uuid
root=pathlib.Path(__file__).resolve().parents[1];cli=os.environ.get('OMAPRESS_CLI',str(root/'target/release/omapress'))
def rpc(path,cmd,args={}):
    p=subprocess.run([cli,'rpc'],input=json.dumps({'schema':1,'command':cmd,'path':str(path),'args':args}),text=True,capture_output=True,check=True)
    v=json.loads(p.stdout);assert v['ok'],v;return v['result']
with tempfile.TemporaryDirectory()as t:
    pub=pathlib.Path(t)/'pub';rpc(pub,'init',{'name':'Benchmark','author':'Fixture','base_url':'https://example.com'})
    meta=None
    for i in range(1000):
        meta={'schema':1,'id':str(uuid.uuid4()),'title':f'Sample article {i}','summary':'Synthetic engine benchmark','series':'today-in-omarchy','slug':f'article-{i}','status':'ready','published_at':'2026-01-02T21:00:00+00:00','tags':[],'x_caption':''}
        (pub/'content'/f'{i}.md').write_text('---\n'+json.dumps(meta)+'\n---\n'+('A short paragraph with a [source](https://example.com).\n\n'*8))
    source=rpc(pub,'inspect')['source_hash'];start=time.perf_counter();rpc(pub,'build',{'expected_source_hash':source});elapsed=time.perf_counter()-start
    samples=[]
    for i in range(25):
        start=time.perf_counter();rpc(pub,'render-document',{'meta':meta,'body':f'A changed paragraph {i}.'});samples.append((time.perf_counter()-start)*1000)
    print(json.dumps({'kind':'synthetic-container','articles':1000,'full_build_seconds':round(elapsed,3),'incremental_preview_p95_ms':round(sorted(samples)[23],2),'includes_process_start':True,'hardware_acceptance':False},indent=2))
