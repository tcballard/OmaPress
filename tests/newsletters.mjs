// End-to-end desktop RPC → HTTP service → PostgreSQL capture queue. No email provider.
import assert from 'node:assert/strict';
import { spawn, spawnSync } from 'node:child_process';
import { mkdtempSync, writeFileSync, rmSync, readFileSync } from 'node:fs';
import { tmpdir } from 'node:os';
import { resolve, join } from 'node:path';
import { randomUUID } from 'node:crypto';
import { setTimeout as delay } from 'node:timers/promises';
assert.equal(process.env.NEWSLETTER_E2E_DISPOSABLE_DATABASE, 'true', 'Use only a disposable PostgreSQL DATABASE_URL');
assert.ok(process.env.DATABASE_URL);
const temp=mkdtempSync(join(tmpdir(),'newsletter-e2e-'));
const root=join(temp,'publication'), credentials=join(temp,'credentials.json');
writeFileSync(credentials,'{}',{mode:0o600});
const token='fixture-token-not-a-production-credential-12345';
const env={...process.env,NEWSLETTER_CREDENTIALS_FILE:credentials};
const service=spawn(resolve('target/debug/newsletter-service'),[],{env:{...process.env,NEWSLETTER_BIND:'127.0.0.1:0',NEWSLETTER_PUBLIC_URL:'http://127.0.0.1:8787',NEWSLETTER_ADMIN_TOKEN:token,NEWSLETTER_SIGNING_KEY:'fixture-signing-key-distinct-not-production-12345',NEWSLETTER_TEST_RECIPIENT:'author@example.com',NEWSLETTER_TRANSPORT:'capture',NEWSLETTER_SENDING_ENABLED:'true',NEWSLETTER_SEND_INTERVAL_MS:'100'},stdio:['ignore','ignore','pipe']});
let log='';service.stderr.on('data',v=>{log+=v;});
function rpc(command,args={},ok=true) {
    const p=spawnSync(resolve('target/debug/omapress'),['rpc'],{env,input:JSON.stringify({schema:1,command,path:root,args}),encoding:'utf8',timeout:70_000});
    assert.ifError(p.error);const v=JSON.parse(p.stdout);assert.equal(v.ok,ok,JSON.stringify(v));return ok?v.result:v.error;
}
try {
    for(let i=0;i<100&&!/listening on (127\.0\.0\.1:\d+)/.test(log);i++){assert.equal(service.exitCode,null,log);await delay(100);}
    const address=log.match(/listening on (127\.0\.0\.1:\d+)/)?.[1];assert.ok(address,log);
    const service_url='http://'+address;
    let pub=rpc('init',{name:'Newsletter fixture',author:'Test author',base_url:'https://example.com'});
    const intake=rpc('intake',{title:'Fixture newsletter',summary:'A newsletter summary',body:'## Hello\n\nSome **formatted** text.',series:Object.keys(pub.config.series)[0],expected_source_hash:pub.source_hash});
    const article=intake.article;
    const doc=rpc('read',{article});doc.article.meta.status='ready';doc.article.meta.published_at=new Date().toISOString();
    pub=rpc('save-document',{article,meta:doc.article.meta,body:doc.article.body,expected_source_hash:doc.source_hash});
    rpc('newsletter-connect',{service_url,api_token:token});
    assert.equal(rpc('newsletter-list').service_url,service_url);
    const slug='fixture-'+randomUUID();
    const n=rpc('newsletter-create',{slug,name:'Fixture',from_email:'author@example.com',reply_to:'reply@example.com',postal_address:'A test postal address'}).newsletter.id;
    const args={article,expected_source_hash:pub.source_hash,newsletter_id:n};
    rpc('newsletter-import',{newsletter_id:n,csv:'email\nreader@example.com\n',consent_attested:true,consent_source:'Test fixture confirmed signup'});
    assert.equal(rpc('newsletter-contacts',{newsletter_id:n}).active,1);
    const review=rpc('newsletter-prepare',args);
    assert.equal(review.audience_count,1);assert.ok(review.html.includes('<strong>formatted</strong>'));
    assert.equal(rpc('newsletter-prepare',args).campaign.id,review.campaign.id);
    const request_id=randomUUID();const t={...args,revision:review.campaign.revision,request_id};
    assert.equal(rpc('newsletter-test',t).delivery_id,rpc('newsletter-test',t).delivery_id);
    const send={...args,revision:review.campaign.revision,audience_hash:review.audience_hash,reviewed:true,send_now:true};
    const first=rpc('newsletter-schedule',send);const second=rpc('newsletter-schedule',send);
    assert.equal(first.campaign.scheduled_at,second.campaign.scheduled_at);
    let status;
    for(let i=0;i<100;i++){status=rpc('newsletter-refresh',args);if(status.counts.captured===1)break;await delay(100);}
    assert.equal(status.counts.captured,1);assert.equal(status.campaign.status,'complete');
    const saved=JSON.parse(readFileSync(credentials,'utf8'));assert.equal(saved.api_token,token);
    console.log('Newsletter end-to-end: connection, subscribers, preview, idempotent test/send and captured delivery passed.');
} finally {
    service.kill('SIGTERM');
    const timeout=setTimeout(()=>service.kill('SIGKILL'),35_000);
    if(service.exitCode===null)await new Promise(r=>service.once('exit',r));
    clearTimeout(timeout);
    rmSync(temp,{recursive:true,force:true});
}
