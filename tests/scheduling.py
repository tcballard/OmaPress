#!/usr/bin/env python3
"""Real RPC queue tests; inherited provider doubles perform no network publication."""
from distribution import DistributionTests
import datetime as dt
import json
import uuid

class SchedulingTests(DistributionTests):
    def add(self, **kw):
        args=dict(self.args,id=str(uuid.uuid4()),at=(dt.datetime.now(dt.timezone.utc)+dt.timedelta(hours=2)).isoformat(),timezone='Europe/London',targets=['x'])
        args.update(kw)
        return args
    def due(self, status=None):
        p=self.pub/'.omapress/queue.json';q=json.loads(p.read_text())
        q['jobs'][0]['at']=(dt.datetime.now(dt.timezone.utc)-dt.timedelta(seconds=5)).isoformat()
        if status:q['jobs'][0]['status']=status
        p.write_text(json.dumps(q))
    def test_intake_plain_markdown_roundtrip(self):
        v=self.rpc('intake',title='Pasted from ChatGPT',summary='Summary',body='# Hello\n\n**World** 🌍',series='today-in-omarchy',expected_source_hash=self.args['expected_source_hash'])
        article=self.rpc('read',article=v['article'])['article']
        self.assertEqual(article['body'],'# Hello\n\n**World** 🌍')
        self.assertEqual(article['meta']['status'],'draft')
    def test_queue_idempotency_due_and_no_replay(self):
        a=self.add();self.rpc('queue-add',**a);self.rpc('queue-add',**a)
        self.assertEqual(len(self.rpc('queue-list')['jobs']),1)
        self.assertEqual(self.rpc('queue-run')['jobs'][0]['status'],'queued')
        self.assertFalse(self.calls.exists())
        self.due();v=self.rpc('queue-run');self.assertEqual(v['jobs'][0]['status'],'completed',v)
        calls=self.calls.read_text();self.rpc('queue-run');self.assertEqual(self.calls.read_text(),calls)
    def test_reconcile_never_sends_and_requires_receipts(self):
        a=self.add();self.rpc('queue-add',**a);self.due('running');self.rpc('queue-run')
        self.rpc('queue-reconcile',id=a['id'],expect=False)
        self.assertFalse(self.calls.exists())
        self.rpc('distribution-confirm',**self.args,target='x',url='https://x.com/test/status/123')
        self.assertEqual(self.rpc('queue-reconcile',id=a['id'])['jobs'][0]['status'],'completed')
        self.assertFalse(self.calls.exists())
    def test_source_change_blocks_without_network(self):
        self.rpc('queue-add',**self.add());self.due()
        with (self.pub/self.args['article']).open('a') as f:f.write('\nEdited after review')
        self.assertEqual(self.rpc('queue-run')['jobs'][0]['status'],'blocked')
        self.assertFalse(self.calls.exists())
    def test_interrupted_worker_never_retries(self):
        self.rpc('queue-add',**self.add());self.due('running')
        self.assertEqual(self.rpc('queue-run')['jobs'][0]['status'],'needs_review')
        self.assertFalse(self.calls.exists())
    def test_failed_provider_stays_for_review(self):
        self.env['PRESSROOM_TEST_FAILURE']='1'
        self.rpc('queue-add',**self.add());self.due()
        self.assertEqual(self.rpc('queue-run')['jobs'][0]['status'],'needs_review')
        before=self.calls.read_text();self.rpc('queue-run');self.assertEqual(before,self.calls.read_text())
    def test_cancel_duplicate_timezone_and_substack(self):
        a=self.add();self.rpc('queue-add',**a)
        self.assertIn('already',self.rpc('queue-add',expect=False,**self.add()))
        self.rpc('queue-cancel',id=a['id'])
        self.assertIn('Substack',self.rpc('queue-add',expect=False,**self.add(targets=['substack'])))
        self.assertIn('ambiguous',self.rpc('queue-add',expect=False,**self.add(local_time='2030-10-27 01:30')))
        self.assertIn('does not exist',self.rpc('queue-add',expect=False,**self.add(local_time='2030-03-31 01:30')))
        self.rpc('queue-add',**self.add(local_time='2030-10-27 12:00'))
    def test_remote_roundtrip_upload_update_and_guard(self):
        # The SSH double forwards the exact private-stdin RPC to a distinct worker root.
        self.tool('ssh',f"import subprocess,sys\nr=subprocess.run([{str(__import__('distribution').ENGINE)!r},'rpc'],input=sys.stdin.buffer.read(),capture_output=True)\nsys.stdout.buffer.write(r.stdout)")
        remote=self.root/'worker'
        def call(command,args={}):
            return self.rpc('remote-queue',host='test-worker',path=str(remote),command=command,args=args)
        first=call('queue-upload',dict(expected_source_hash=self.args['expected_source_hash']))
        self.assertEqual(first['source_hash'],self.args['expected_source_hash'])
        call('queue-add',self.add())
        self.assertEqual(call('queue-list')['jobs'][0]['status'],'queued')
        with (self.pub/self.args['article']).open('a') as f:f.write('\nA newer version')
        new=self.rpc('inspect')['source_hash']
        error=self.rpc('remote-queue',expect=False,host='test-worker',path=str(remote),command='queue-upload',args=dict(expected_source_hash=new,expected_remote_source_hash=first['source_hash']))
        self.assertIn('pending jobs',error)
        call('queue-cancel',dict(id=call('queue-list')['jobs'][0]['id']))
        call('queue-upload',dict(expected_source_hash=new,expected_remote_source_hash=first['source_hash']))
        self.assertEqual(call('queue-list')['source_hash'],new)
        self.assertEqual(call('queue-list')['jobs'][0]['status'],'cancelled')
        self.assertIn('SSH host',self.rpc('remote-queue',expect=False,host='-oProxyCommand=bad',path='/tmp/pub',command='queue-list'))
    def test_corrupt_and_newer_queue_rejected(self):
        p=self.pub/'.omapress/queue.json';p.parent.mkdir(exist_ok=True);p.write_text('{')
        self.assertIn('corrupt',self.rpc('queue-list',expect=False))
        p.write_text('{"schema":2,"jobs":[]}')
        self.assertIn('schema',self.rpc('queue-list',expect=False))
    def test_headless_credentials_permissions(self):
        p=self.root/'x.json';p.write_text('{"access_token":"test-user-token"}');p.chmod(0o600)
        self.env['PRESSROOM_X_CREDENTIALS_FILE']=str(p)
        self.rpc('x-status')
        p.chmod(0o644)
        self.assertIn('owner-only',self.rpc('x-status',expect=False))

if __name__=='__main__':
    import unittest
    suite=unittest.TestSuite(SchedulingTests(name) for name in SchedulingTests.__dict__ if name.startswith('test_'))
    result=unittest.TextTestRunner(verbosity=2).run(suite)
    raise SystemExit(not result.wasSuccessful())
