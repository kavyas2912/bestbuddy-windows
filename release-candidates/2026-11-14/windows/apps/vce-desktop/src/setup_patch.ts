import {invoke} from '@tauri-apps/api/core';
import {speakGuidance} from './speech';

export const preparationMessage='Please wait. Let me make preparations to serve you. I am saving your encrypted data locally, preparing my voice and activating intelligent conversation with your chosen model. Once I am ready, you will hear from me.';
export function preparationWait(panel:HTMLElement){
 const box=document.createElement('div');box.className='preparation-wait';box.setAttribute('role','status');box.setAttribute('aria-live','polite');box.innerHTML='<span class="preparation-spinner" aria-hidden="true"></span><p></p>';box.querySelector('p')!.textContent=preparationMessage;
 const form=panel.querySelector<HTMLElement>('form');if(form)form.inert=true;panel.setAttribute('aria-busy','true');panel.append(box);
 return ()=>{box.remove();if(form)form.inert=false;panel.removeAttribute('aria-busy');};
}

/** In-app modal uses the current theme and restores focus to its initiating control. */
export function setupGuide(title:string,text:string,links:{name:string,url:string}[],nextLabel='Done',next?:()=>Promise<void>){
 const prior=document.activeElement as HTMLElement|null;
 const dialog=document.createElement('dialog');dialog.className='setup-help-dialog';dialog.setAttribute('aria-label',title);
 const heading=document.createElement('h3');heading.textContent=title;const words=document.createElement('p');words.textContent=text;
 const status=document.createElement('p');status.setAttribute('role','status');const actions=document.createElement('div');actions.className='setup-help-actions';
 const close=document.createElement('button');close.type='button';close.textContent='Cancel';close.onclick=()=>dialog.close();
 for(const link of links){const button=document.createElement('button');button.type='button';button.textContent=link.name+' ↗';button.onclick=()=>void invoke('open_external',{url:link.url}).catch(e=>status.textContent=String(e));actions.append(button);}
 let busy=false;dialog.addEventListener('cancel',event=>{if(busy)event.preventDefault();});
 const proceed=document.createElement('button');proceed.type='button';proceed.textContent=nextLabel;proceed.onclick=async()=>{busy=true;proceed.disabled=true;close.disabled=true;try{await next?.();dialog.close();}catch(e){status.textContent=String(e);}finally{busy=false;proceed.disabled=false;close.disabled=false;}};
 actions.append(close,proceed);dialog.append(heading,words,status,actions);document.body.append(dialog);dialog.addEventListener('close',()=>{dialog.remove();if(prior?.isConnected)prior.focus();});dialog.showModal();return dialog;
}
const planMessage='Online AI plans may have limited free requests, temporary credits, regional restrictions or paid subscriptions. Limits can change. Check your selected provider’s current pricing and API access. This app does not buy a subscription for you. Your requests are sent to that provider only when online conversation is enabled.';
export function onlinePlanGuide(){setupGuide('Online AI plans',planMessage,[{name:'OpenRouter plans',url:'https://openrouter.ai/docs/faq'},{name:'Gemini pricing',url:'https://ai.google.dev/gemini-api/docs/pricing'}]);void speakGuidance(planMessage,'online-plan','Assistant');}

let voiceWork:Promise<void>=Promise.resolve();
export function waitForVoicePreparation(){return voiceWork;}
export async function englishVoicePicker(host:HTMLElement,onChoice:(id:string)=>void,initial=''){
 const result=document.createElement('p');result.setAttribute('role','status');
 host.replaceChildren();const title=document.createElement('h3');title.textContent='Voice engine';const engine=document.createElement('label');engine.className='setup-radio';engine.innerHTML='<input type="radio" checked name="speech-engine" value="piper"> Piper · offline English speech';
 const note=document.createElement('small');note.textContent='Piper is the supported engine in this release. Choose an English speaker below.';
 const list=document.createElement('div');list.className='speaker-radio-list';host.append(title,engine,note,list,result);
 try{
  const data:any=await invoke('piper_voices');const all=[...(data.available||[]),...(data.installed||[]).filter((v:any)=>!(data.available||[]).some((x:any)=>x.id===v.id))];
  const voices=all.filter((v:any)=>/^en(?:[-_]|glish|$)/i.test(v.language||'')&&!v.id.includes('pratham'));
  for(const voice of voices){const row=document.createElement('label');row.className='setup-radio';const radio=document.createElement('input');radio.type='radio';radio.name='speaker-'+(host.id||host.dataset.modelStage||'settings');radio.value=voice.id;radio.checked=voice.id===(initial||data.selected);const label=document.createElement('span');label.textContent=`${voice.name} · ${voice.gender||'voice'}${voice.installed?' · ready':''}`;row.append(radio,label);list.append(row);
   radio.onchange=()=>{const id=voice.id;result.textContent='Preparing your selected voice…';onChoice('');
    voiceWork=voiceWork.catch(()=>{}).then(async()=>{if(!voice.installed){await invoke('piper_prepare_voice',{id});voice.installed=true;}if(radio.checked){onChoice(id);result.textContent='Voice ready. It will activate when you save your choices.';}}).catch(e=>{result.textContent='Voice preparation failed; the existing voice is retained. '+String(e);radio.checked=false;});
   };
  }
  if(!voices.length)result.textContent='The English voice catalogue is unavailable. The current fallback voice is retained.';
 }catch(e){result.textContent='Voice catalogue unavailable: '+String(e);}
}
