// Executed in the isolated world only after a user presses Fill in the popup.
export async function fillDraft(bundle) {
    try {
        if (location.protocol !== 'https:' || !location.hostname.endsWith('.substack.com') || !location.pathname.startsWith('/publish/')) throw Error('Open your publication’s blank Substack editor first.');
        const body = document.querySelector('.post-editor [contenteditable="true"]');
        const title = document.getElementById('post-title');
        const subtitle = document.querySelector('.subtitle.mousetrap');
        if (!body || !title || !subtitle) throw Error('Substack’s editor layout is not recognized. Nothing was inserted.');
        const read = el => typeof el.value === 'string' ? el.value : el.textContent;
        if (read(title).trim() || read(subtitle).trim() || body.textContent.trim() || body.querySelector('img,video,iframe,hr')) throw Error('This draft already contains content. Open a blank draft to avoid overwriting it.');
        if (typeof bundle.title !== 'string' || typeof bundle.subtitle !== 'string' || typeof bundle.html !== 'string') throw Error('Invalid prepared article.');
        const parsed = new DOMParser().parseFromString(bundle.html, 'text/html');
        if (parsed.querySelector('script,iframe,object,embed,form,style,link')) throw Error('Unsupported HTML in the prepared article.');
        for (const el of parsed.body.querySelectorAll('*')) for (const attr of el.attributes) {
            if (attr.name.startsWith('on') || /^(javascript|vbscript):/i.test(attr.value.trim())) throw Error('Unsafe article HTML.');
        }
        const set = (el,text) => {
            if (el instanceof HTMLTextAreaElement || el instanceof HTMLInputElement) {
                const prototype = el instanceof HTMLTextAreaElement ? HTMLTextAreaElement.prototype : HTMLInputElement.prototype;
                Object.getOwnPropertyDescriptor(prototype,'value').set.call(el,text);
            } else { el.textContent=text; }
            el.dispatchEvent(new Event('input',{bubbles:true}));
            el.dispatchEvent(new Event('change',{bubbles:true}));
        };
        set(title,bundle.title); set(subtitle,bundle.subtitle);
        body.focus();
        const selection=window.getSelection(), range=document.createRange();
        range.selectNodeContents(body);selection.removeAllRanges();selection.addRange(range);
        const transfer=new DataTransfer();
        transfer.setData('text/html',bundle.html);
        transfer.setData('text/plain',parsed.body.textContent);
        body.dispatchEvent(new ClipboardEvent('paste',{clipboardData:transfer,bubbles:true,cancelable:true}));
        await new Promise(resolve=>setTimeout(resolve,1000));
        const normalize=s=>s.replace(/\s+/g,'').trim();
        if (read(title)!==bundle.title || read(subtitle)!==bundle.subtitle || normalize(body.textContent)!==normalize(parsed.body.textContent) || body.querySelectorAll('img').length < parsed.querySelectorAll('img').length) throw Error('Substack did not preserve the complete pasted article. Inspect this draft before doing anything else; it was not marked published.');
        return {ok:true,message:'Draft filled. Wait for Substack to save and upload images, then review formatting, audience, and email delivery before publishing.'};
    } catch (error) {return {ok:false,message:error.message};}
}
