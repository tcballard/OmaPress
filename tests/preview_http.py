#!/usr/bin/env python3
"""Actual loopback preview acceptance, run on native Linux CI."""
import json,os,pathlib,subprocess,urllib.request,tempfile,shutil
root=pathlib.Path(__file__).resolve().parents[1]
cli=os.environ.get('OMAPRESS_CLI',str(root/'target/debug/omapress'))
with tempfile.TemporaryDirectory()as t:
    pub=pathlib.Path(t)/'pub';shutil.copytree(root/'fixtures/fixing-everything',pub,ignore=shutil.ignore_patterns('.omapress','output'))
    p=subprocess.Popen([cli,'preview',str(pub),'--json'],stdout=subprocess.PIPE,stderr=subprocess.PIPE,text=True)
    try:
        line=p.stdout.readline();v=json.loads(line);assert v.get('event')=='preview_ready',v
        with urllib.request.urlopen(v['url'])as r:assert r.status==200 and b'Fixing Everything' in r.read()
        with urllib.request.urlopen(v['url']+'today-in-omarchy/rss.xml')as r:assert b'<rss 'in r.read()
        try:urllib.request.urlopen(v['url']+'%2e%2e/publication.toml');raise AssertionError('Traversal accepted')
        except urllib.error.HTTPError as e:assert e.code==404
        req=urllib.request.Request(v['url'],headers={'Host':'attacker.example'})
        try:urllib.request.urlopen(req);raise AssertionError('Wrong Host accepted')
        except urllib.error.HTTPError as e:assert e.code==404
        print('Loopback site, daily RSS, traversal and Host checks passed.')
    finally:p.terminate();p.wait(timeout=5)
