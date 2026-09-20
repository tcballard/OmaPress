#!/usr/bin/env python3
"""Run the real CLI and real Git against a local bare relay.
Only transport executables are replaced; production publication rules run intact.
No GitHub account, network or real publication is touched.
"""
import json, os, pathlib, shutil, subprocess, tempfile, unittest
ROOT=pathlib.Path(__file__).resolve().parents[1]
CLI=pathlib.Path(os.environ.get('OMAPRESS_CLI',ROOT/'target/debug/omapress')).resolve()
GIT=shutil.which('git')
class DeploymentTests(unittest.TestCase):
    def setUp(self):
        self.temp=tempfile.TemporaryDirectory();self.root=pathlib.Path(self.temp.name)
        self.bin=self.root/'bin';self.bin.mkdir();self.pub=self.root/'publication';self.remote=self.root/'remote.git'
        subprocess.run([GIT,'init','--bare','--quiet',str(self.remote)],check=True)
        self.env=os.environ|{'PATH':str(self.bin)+os.pathsep+os.environ['PATH'],'FIXTURE_REMOTE':str(self.remote),'REAL_GIT':GIT,'FIXTURE_CONTROL':str(self.root)}
        self.script('git',r'''
import os,sys,subprocess,pathlib
args=sys.argv[1:];root=pathlib.Path(os.environ['FIXTURE_CONTROL'])
args=[os.environ['FIXTURE_REMOTE'] if a=='https://github.com/fixture/site.git' else a for a in args]
if 'push' in args:
    with (root/'pushes').open('a') as f:f.write('push\n')
    if (root/'fail-push').exists():sys.exit(7)
# Real Git still performs the exact force-with-lease comparison.
args=['-c','protocol.file.allow=always']+args
for i,a in enumerate(args):
    if a=='protocol.file.allow=never':args[i]='protocol.file.allow=always'
sys.exit(subprocess.call([os.environ['REAL_GIT']]+args))
''')
        self.script('gh',r'''
import json,sys
args=sys.argv[1:]
if args[:2]==['auth','status']:print('Logged in');sys.exit(0)
if args==['api','user']:print(json.dumps({'login':'fixture'}));sys.exit(0)
if args and args[0]=='api' and args[-1].endswith('/pages'):print(json.dumps({'source':{'branch':'gh-pages','path':'/'}}));sys.exit(0)
print('{}')
''')
        self.script('curl',r'''
import os,sys,pathlib,subprocess
root=pathlib.Path(os.environ['FIXTURE_CONTROL']);args=sys.argv[1:]
if (root/'offline').exists():print('Fixture offline',file=sys.stderr);sys.exit(22)
url=args[-1];prefix='https://fixture.example/'
if not url.startswith(prefix):sys.exit(23)
path=url[len(prefix):]
data=subprocess.check_output([os.environ['REAL_GIT'],'--git-dir',os.environ['FIXTURE_REMOTE'],'show','gh-pages:'+path])
if '--output' in args:pathlib.Path(args[args.index('--output')+1]).write_bytes(data)
else:sys.stdout.buffer.write(data)
''')
        self.rpc('init',{'name':'Fixture','author':'Test Author','base_url':'https://fixture.example'})
        cfg=(self.pub/'publication.toml').read_text().replace('repository = ""','repository = "fixture/site"')
        self.rpc('save-config',{'text':cfg,'expected_source_hash':self.inspect()['source_hash']})
        created=self.rpc('new',{'series':'today-in-omarchy','expected_source_hash':self.inspect()['source_hash']})
        self.article=created['article'];doc=self.rpc('read',{'article':self.article})['article'];self.meta=doc['meta']
        self.meta.update(title='A reviewed story',summary='A useful summary',status='ready',published_at='2026-01-02T21:00:00+00:00')
        self.save('Original body.')
    def tearDown(self):self.temp.cleanup()
    def script(self,name,body):
        p=self.bin/name;p.write_text('#!/usr/bin/env python3\n'+body);p.chmod(0o755)
    def rpc(self,command,args=None,ok=True):
        p=subprocess.run([str(CLI),'rpc'],input=json.dumps({'schema':1,'command':command,'path':str(self.pub),'args':args or {}}),text=True,capture_output=True,env=self.env,timeout=30)
        result=json.loads(p.stdout)
        if ok:self.assertTrue(result['ok'],result)
        return result.get('result') if ok else result
    def inspect(self):return self.rpc('inspect')
    def save(self,body):return self.rpc('save-document',{'article':self.article,'meta':self.meta,'body':body,'expected_source_hash':self.inspect()['source_hash']})
    def plan(self):return self.rpc('publish-plan',{'expected_source_hash':self.inspect()['source_hash']})
    def publish(self,plan=None):
        p=plan or self.plan();return self.rpc('publish',{'expected_source_hash':p['source_hash'],'expected_remote_head':p['expected_remote_head']})
    def test_verified_publish_correction_and_rollback(self):
        source=(self.pub/self.article).read_bytes();first=self.publish();self.assertEqual(first['status'],'published');self.assertEqual((self.pub/self.article).read_bytes(),source)
        state=self.inspect();self.assertEqual(state['articles'][0]['display_status'],'published');self.save('Corrected body.');second=self.publish();self.assertEqual(second['status'],'published')
        p=self.plan();rolled=self.rpc('rollback',{'deployment_id':first['deployment']['id'],'expected_source_hash':p['source_hash'],'expected_remote_head':p['expected_remote_head']});self.assertEqual(rolled['status'],'published')
        self.assertIn('Corrected body.',(self.pub/self.article).read_text());out=subprocess.check_output([GIT,'--git-dir',str(self.remote),'show','gh-pages:rss.xml'],text=True);self.assertIn('Original body.',out);self.assertNotIn('Corrected body.',out)
        self.assertEqual(len(self.inspect()['state']['identities']),1)
    def test_unknown_public_verification_rechecks_without_pushing(self):
        (self.root/'offline').touch();v=self.publish();self.assertEqual(v['status'],'unknown');self.assertEqual(self.inspect()['articles'][0]['display_status'],'ready');pushes=(self.root/'pushes').read_text()
        (self.root/'offline').unlink();v=self.rpc('recheck');self.assertEqual(v['status'],'published');self.assertEqual((self.root/'pushes').read_text(),pushes)
    def test_failed_push_can_be_resolved_without_blind_retry(self):
        (self.root/'fail-push').touch();self.assertEqual(self.publish()['status'],'unknown');self.assertEqual(self.rpc('recheck')['status'],'not_published');self.assertIsNone(self.inspect()['state']['pending'])
    def test_stale_review_never_pushes(self):
        p=self.plan();self.save('Changed after review.');v=self.rpc('publish',{'expected_source_hash':p['source_hash'],'expected_remote_head':p['expected_remote_head']},ok=False);self.assertFalse(v['ok']);self.assertFalse((self.root/'pushes').exists())
    def test_dirty_deployment_worktree_is_preserved(self):
        self.publish();work=self.pub/'.omapress/deploy-worktree';(work/'unrelated.txt').write_text('Preserve this');v=self.rpc('publish',{'expected_source_hash':self.plan()['source_hash'],'expected_remote_head':self.plan()['expected_remote_head']},ok=False);self.assertFalse(v['ok']);self.assertEqual((work/'unrelated.txt').read_text(),'Preserve this')
    def test_unexpected_remote_movement_blocks_publish(self):
        self.publish();p=self.plan();work=self.pub/'.omapress/deploy-worktree'
        subprocess.run([GIT,'-c','user.name=Fixture','-c','user.email=test@example.com','commit','--allow-empty','-m','External update'],cwd=work,check=True,capture_output=True)
        subprocess.run([GIT,'push',str(self.remote),'HEAD:gh-pages'],cwd=work,check=True,capture_output=True)
        v=self.rpc('publish',{'expected_source_hash':p['source_hash'],'expected_remote_head':p['expected_remote_head']},ok=False);self.assertFalse(v['ok']);self.assertIn('Remote branch moved',v['error'])
if __name__=='__main__':unittest.main(verbosity=2)
