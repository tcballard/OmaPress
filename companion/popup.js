import {fillDraft} from './editor.js';
const $=id=>document.getElementById(id);
let tab;
async function native(command,args={}) {
    const response=await chrome.runtime.sendNativeMessage('com.pressroom.companion',{command,...args});
    if (!response?.ok) throw Error(response?.error || 'Pressroom companion returned an invalid response.');
    return response.result;
}
const selected=()=>{const id=$('articles').value;if(!id)throw Error('Prepare an article in Pressroom first.');return id;};
async function action(task) {
    for(const id of ['fill','confirm','remove'])$(id).disabled=true;
    try {await task();} catch(error){$('status').textContent=error.message;}
    finally {enable();}
}
function enable() {
    const id=$('articles').value;
    let url;try{url=new URL(tab?.url);}catch{return;}
    const host=url.protocol==='https:'&&(url.hostname==='substack.com'||url.hostname.endsWith('.substack.com'));
    $('fill').disabled=!id||!host||!url.pathname.startsWith('/publish/');
    $('confirm').disabled=!id||url.protocol!=='https:'||!url.pathname.startsWith('/p/');
    $('remove').disabled=!id;
}
async function refresh() {
    const {items}=await native('list');
    $('articles').replaceChildren(...items.map(item=>new Option(item.title,item.id)));
    $('status').textContent=items.length?'Choose the matching article before filling or recording a page.':'Use Prepare Substack in Pressroom to add an article.';
    enable();
}
$('articles').onchange=enable;
$('fill').onclick=()=>action(async()=>{
    const id=selected(),chunks=[];let offset=0,total,hash;
    do {
        const part=await native('read',{id,offset});
        if (part.total>32*1024*1024 || part.next<=offset || part.next>part.total || (hash&&hash!==part.hash)) throw Error('Invalid or changed browser outbox.');
        const bytes=Uint8Array.from(atob(part.data),c=>c.charCodeAt(0));
        if(bytes.length!==part.next-offset)throw Error('Invalid outbox chunk.');
        chunks.push(bytes);offset=part.next;total=part.total;hash=part.hash;
    }while(offset<total);
    const bytes=new Uint8Array(total);let position=0;for(const part of chunks){bytes.set(part,position);position+=part.length;}
    const digest=[...new Uint8Array(await crypto.subtle.digest('SHA-256',bytes))].map(b=>b.toString(16).padStart(2,'0')).join('');
    if(digest!==hash)throw Error('Prepared article integrity check failed.');
    const bundle=JSON.parse(new TextDecoder('utf-8',{fatal:true}).decode(bytes));
    if(bundle.schema!==1||bundle.id!==id)throw Error('Unsupported prepared article.');
    const result=await chrome.scripting.executeScript({target:{tabId:tab.id},func:fillDraft,args:[bundle]});
    if(!result[0]?.result?.ok)throw Error(result[0]?.result?.message||'Could not fill this editor.');
    $('status').textContent=result[0].result.message;
});
$('confirm').onclick=()=>action(async()=>{
    // Explicit user confirmation, never inferred from a successful paste.
    const current=await chrome.tabs.get(tab.id);
    const url=new URL(current.url);
    if(url.protocol!=='https:'||!url.pathname.startsWith('/p/'))throw Error('Open the published article page before recording its URL.');
    await native('confirm',{id:selected(),url:current.url});
    $('status').textContent='Published URL recorded as confirmed by you. Refresh the article’s destination status in Pressroom.';
});
$('remove').onclick=()=>action(async()=>{await native('remove',{id:selected()});await refresh();});
try {
    [tab]=await chrome.tabs.query({active:true,currentWindow:true});
    $('destination').textContent=tab?.url?`Current page: ${new URL(tab.url).hostname}`:'Open your Substack editor.';
    await refresh();
}catch(error){$('status').textContent=`${error.message}\nInstall the native companion host as described in Pressroom’s Connections help.`;}
