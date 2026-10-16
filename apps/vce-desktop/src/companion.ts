import { invoke } from '@tauri-apps/api/core';
import { currentDayAndTime, deliveryKey, dueReminders, listReminderEvents } from './reminders';

const escape = (s: unknown) => String(s ?? '').replace(/[&<>"']/g, c => ({'&':'&amp;','<':'&lt;','>':'&gt;','"':'&quot;',"'":'&#39;'}[c]!));
const consentLabels: Record<string,string> = {
  profile_storage: 'Save my profile on this device (required to save registration)',
  contacts_storage: 'Store the family, friend and emergency contact details I enter',
  personalization: 'Remember interests and learn my conversational vocabulary',
  local_learning: 'Learn vocabulary and source-backed knowledge from the public sources I enable',
  microphone: 'Allow microphone listening after registration (OS permission is also required)',
  voice_processing: 'Allow local speech recognition and voice processing on this device',
};
function stored(key: string, fallback: any) { try { return JSON.parse(localStorage.getItem(key) || 'null') ?? fallback; } catch { return fallback; } }
function field(label: string, key: string, value: unknown, type='text') {
  return `<label>${label}<input data-field="${key}" type="${type}" value="${escape(value)}"></label>`;
}
export function setupCompanion(getProfile:()=>any, setProfile:(p:any)=>void, notify:(s:string)=>void, refreshed:()=>Promise<void>) {
  let draft:any; let step=0; let savedDraft=false;
  let draftWrite:Promise<unknown>=Promise.resolve();
  let pendingHfToken=''; let pendingProviderKey=''; let pendingVaultPassword='';
  let chosenProvider='auto'; let chosenLocalModel='qwen3-0.6b'; let chosenOnlineModel='openrouter/free';
  const steps=['Welcome & privacy','Your profile','Family & emergency contacts','Important dates','BestBuddy preferences','Conversation models','Review & voice'];
  const panel=document.querySelector<HTMLElement>('#onboarding')!;
  panel.setAttribute('role','dialog'); panel.setAttribute('aria-modal','true'); panel.setAttribute('aria-label','BestBuddy registration');
  const spokenSteps=[
    'Welcome to BestBuddy. This beta is free to test. Your saved profile, contacts, reminders and learning files are encrypted for your Windows account. Online providers use the internet only when you configure and use them. You can review each choice before continuing.',
    'Tell me what name you prefer, and optionally add your birth date, location and language. You may leave optional details blank.',
    'Add family, friends or emergency contacts only with permission. BestBuddy can open communication apps, but cannot place emergency calls or read who is online.',
    'Add birthdays, anniversaries and other important dates. Reminders appear inside BestBuddy; external delivery is not configured.',
    'Choose the conversation style you like and add your own phrases. BestBuddy checks local commands and personal vocabulary first.',
    'Choose an optional compact local model or an online provider. Online conversation sends what you say to that provider. You can skip model setup and return later.',
    'Review your choices. Windows or Android controls microphone permission. BestBuddy is in beta testing; please verify important information independently.'
  ];
  const stopNarration=()=>{try{speechSynthesis.cancel();}catch{}panel.classList.remove('buddy-speaking');};
  const narrate=(message:string)=>{
    stopNarration();
    if(!('speechSynthesis' in window))return;
    const utterance=new SpeechSynthesisUtterance(message);
    utterance.rate=0.94;utterance.pitch=1.08;
    utterance.onstart=()=>panel.classList.add('buddy-speaking');
    utterance.onend=utterance.onerror=()=>panel.classList.remove('buddy-speaking');
    try{speechSynthesis.speak(utterance);}catch{}
  };
  const renderSplash=()=>{
    panel.innerHTML=`<div class="registration-splash"><div class="splash-orbit"><img src="/assets/macaw-buddy.png" alt="BestBuddy the macaw" class="splash-macaw"></div><div class="kicker">WELCOME TO BESTBUDDY</div><h2>Your always-there companion</h2><p id="splash-words">Hello. I’m BestBuddy. I’ll guide you through a few choices about your profile, voice and privacy.</p><p class="splash-beta">Beta preview · Free to test · Your choices stay editable in Settings</p><div class="registration-actions"><button type="button" id="splash-replay">Hear welcome again</button><button type="button" id="splash-begin" class="primary-wide">Get started</button></div></div>`;
    panel.querySelector('#splash-replay')!.addEventListener('click',()=>narrate('Hello. I’m BestBuddy. I’ll guide you through a few choices about your profile, voice and privacy.'));
    panel.querySelector('#splash-begin')!.addEventListener('click',()=>{stopNarration();render();});
    narrate('Hello. I’m BestBuddy. I’ll guide you through a few choices about your profile, voice and privacy.');
  };
  const render=()=>{
    const c=draft.consents;
    let body='';
    if(step===0) body=`<div class="onboard-section"><h3>Private local profile</h3><p>This is device registration; it does not create an online account. Saved profile, contacts, reminders and learning files are encrypted and bound to your Windows user account. Provider keys use the separate password-protected credential vault.</p></div><div class="onboard-section"><h3>Your choices</h3>${Object.entries(consentLabels).map(([k,l])=>`<label class="check"><input type="checkbox" data-consent="${k}" ${c[k]?'checked':''}>${l}</label>`).join('')}<label class="check"><input id="draft-consent" type="checkbox" ${savedDraft?'checked':''}>Save this registration draft on this device so I can resume later</label></div><p class="onboard-note">You can continue with text and narrated guidance. Microphone listening starts only after registration, your voice choices and OS permission; public widgets contact their listed data sources.</p>`;
    if(step===1) body=field('Preferred name','name',draft.name==='Friend'?'':draft.name)+field('Birth date (optional)','birthdate',draft.birthdate,'date')+`<p id="derived-age">Age is calculated from your birth date.</p>`+field('City','location',draft.location)+field('Region','region',draft.region)+field('Country','country',draft.country)+field('Time zone (IANA name)','timezone',draft.timezone)+field('Preferred language','language',draft.language)+`<p>Installed speech models currently determine available voice languages.</p>`;
    if(step===2) body=`<p>Add only contact details you are allowed to store. Emergency priority 1 is primary; 2 is secondary. BestBuddy does not automatically contact emergency services.</p>${draft.contacts.map((x:any,i:number)=>`<fieldset data-contact="${i}"><legend>${escape(x.name||`Contact ${i+1}`)}</legend>${field('Name','name',x.name)}${field('Relationship','relationship',x.relationship)}${field('Phone (include country code)','phone',x.phone,'tel')}${field('Email','email',x.email,'email')}${field('Signal share link (from Signal app)','signal_link',x.signal_link,'url')}${field('Messenger conversation link (copy from browser)','messenger_link',x.messenger_link,'url')}${field('Slack conversation link','slack_link',x.slack_link,'url')}${field('Discord conversation link','discord_link',x.discord_link,'url')}${field('Zoom meeting link','zoom_link',x.zoom_link,'url')}${field('Teams meeting link','teams_link',x.teams_link,'url')}${field('Birthday','birthday',x.birthday,'date')}${field('Anniversary','anniversary',x.anniversary,'date')}<label>Preferred communication<select data-field="channel">${['whatsapp','messenger','slack','discord','zoom','teams','telegram','signal','phone','sms','email'].map(k=>`<option ${x.channel===k?'selected':''}>${k}</option>`).join('')}</select></label><label>Emergency priority<select data-field="emergency_priority">${[0,1,2].map(n=>`<option value="${n}" ${x.emergency_priority===n?'selected':''}>${n===0?'Not an emergency contact':n}</option>`).join('')}</select></label><button type="button" data-remove-contact="${i}">Remove contact</button></fieldset>`).join('')}<button id="add-contact" type="button" ${c.contacts_storage?'':'disabled'}>Add contact</button>${!c.contacts_storage?'<p>Enable contact storage on the privacy step to add contacts.</p>':''}`;
    if(step===3) body=`<p>Reminders appear inside BestBuddy at your chosen local time. If BestBuddy was closed, it shows reminders missed in the last seven days when reopened. Birthdays and anniversaries from Contacts are included automatically. External delivery is not configured.</p>${draft.reminders.map((x:any,i:number)=>`<fieldset data-reminder="${i}"><legend>Reminder ${i+1}</legend>${field('Title','title',x.title)}${field('Date','date',x.date,'date')}${field('Time in your profile time zone','time',x.time||'09:00','time')}${field('Remind how many days before (0–366)','lead_days',x.lead_days,'number')}<label class="check"><input type="checkbox" data-field="annual" ${x.annual?'checked':''}>Repeat annually</label><label class="check"><input type="checkbox" data-field="enabled" ${x.enabled?'checked':''}>Enabled</label><button type="button" data-remove-reminder="${i}">Remove reminder</button></fieldset>`).join('')}<button id="add-reminder" type="button">Add important date</button>`;
    if(step===4) body=field('Interests (comma separated)','interests',(draft.interests||[]).join(', '))+field('Music and other preferences (comma separated)','preferences',(draft.preferences||[]).join(', '))+`<label>Conversation style<select data-field="style">${['friendly','formal','playful'].map(k=>`<option ${draft.style===k?'selected':''}>${k}</option>`).join('')}</select></label><label>Your phrasebook (one command = reply per line)<textarea id="phrasebook" rows="6">${escape(Object.entries(draft.phrasebook||{}).map(([k,v])=>`${k} = ${v}`).join('\n'))}</textarea></label><p>Exact phrasebook matches run locally. Live knowledge has a 24-hour retrieval limit. Personalization controls learned conversational vocabulary.</p>`;
    if(step===5) body=`<p class="onboard-note">BestBuddy checks commands, phrasebook, vocabulary and prefetched sources first. Model setup is optional and remains editable in Settings.</p><div class="onboard-section model-onboard-section"><h3>Hugging Face access</h3><div class="onboard-inline"><label>Hugging Face token<input id="registration-hf-token" type="password" autocomplete="off" value="${escape(pendingHfToken)}" placeholder="Optional · encrypted in key vault"></label><button type="button" data-model-guide="https://huggingface.co/settings/tokens">Get token and guide ↗</button></div></div><div class="onboard-section model-onboard-section"><h3>Local compact model</h3><div class="onboard-inline"><label>Model<select id="registration-local-model"><option value="qwen3-0.6b">Qwen3 0.6B · about 0.6 GB</option><option value="qwen3-1.7b">Qwen3 1.7B · about 1.3 GB</option></select></label><button id="registration-prepare-local" type="button">Prepare model</button></div><p id="registration-model-status" role="status">Optional model files are downloaded only into BestBuddy's private app-data folder.</p></div><div class="onboard-section model-onboard-section"><h3>Composer and online fallback</h3><label>Conversation option<select id="registration-model-provider"><option value="auto">Automatic local-first routing</option><option value="rules">Built-in sources only</option><option value="local" disabled>Local GGUF model only (checking assets)</option><option value="groq">Automatic + Groq online fallback</option><option value="openrouter">Automatic + OpenRouter online fallback</option><option value="huggingface">Automatic + Hugging Face online fallback</option></select></label><label>Current free OpenRouter model<select id="registration-online-model"><option value="openrouter/free">Free model router</option></select></label><p id="registration-free-model-status" role="status">Checking live free-model choices…</p></div><div class="onboard-section model-onboard-section"><h3>Encrypted provider key</h3><div class="onboard-inline"><label>Vault password<input id="registration-vault-password" type="password" autocomplete="new-password" placeholder="At least 12 characters"></label><label>Selected provider API key<input id="registration-api-key" type="password" autocomplete="off" placeholder="Leave blank for local-only setup"></label></div><div class="compact-links"><button type="button" data-model-guide="https://console.groq.com/keys">Groq ↗</button><button type="button" data-model-guide="https://openrouter.ai/keys">OpenRouter ↗</button><button type="button" data-model-guide="https://aistudio.google.com/app/apikey">Gemini ↗</button><button type="button" data-model-guide="https://huggingface.co/ggml-org/Qwen3-1.7B-GGUF">Local model details ↗</button></div></div><button id="registration-skip-model" type="button">Skip model setup</button>`;
    if(step===5) body=body.replace('<option value="huggingface">Automatic + Hugging Face online fallback</option>','<option value="huggingface">Automatic + Hugging Face online fallback</option><option value="gemini">Automatic + Google Gemini free-tier fallback</option>')
      +`<div id="registration-guide-popup" class="registration-guide-popup hidden" role="dialog" aria-modal="true" aria-label="Account setup guide"><div class="registration-guide-card"><h3 id="registration-guide-title">Account setup</h3><p id="registration-guide-text"></p><div class="registration-actions"><button id="registration-guide-open" type="button">Open official page ↗</button><button id="registration-guide-close" type="button">Close guide</button></div></div></div>`;
    if(step===6) body=`<h3>${escape(draft.name)}, here is your setup</h3><p>${escape(draft.location)} · ${escape(draft.timezone)}<br>${draft.contacts.length} contacts · ${draft.reminders.length} important dates</p><ul>${Object.entries(consentLabels).map(([k,l])=>`<li>${c[k]?'Allowed':'Off'}: ${l}</li>`).join('')}</ul><h3>Communication connections</h3><p>Messenger, WhatsApp, Slack, Discord, Zoom and Teams open in their own websites. Add contact links in your profile to open a specific conversation or meeting; you review and act there. BestBuddy does not read online presence or control personal video calls. Phone, SMS and email use installed handlers.</p><p>BestBuddy listens for its wake word whenever the desktop app is open. Talk begins a direct request without the wake word.</p>`;
    panel.innerHTML=`<div class="modal-card registration"><div class="registration-guide"><img src="/assets/macaw-buddy.png" alt="BestBuddy the macaw" class="registration-macaw"><div><h2>${steps[step]}</h2><p id="registration-narration">${escape(spokenSteps[step])}</p><button type="button" id="registration-replay">Hear this step</button></div></div><p>Step ${step+1} of ${steps.length}</p><progress value="${step+1}" max="${steps.length}"></progress><form id="registration-form">${body}<p id="registration-error" role="alert"></p><div class="registration-actions"><button type="button" id="registration-back" ${step===0?'disabled':''}>Back</button><button type="button" id="registration-later">Continue later</button><button type="submit">${step===6?'Save & meet BestBuddy':'Continue'}</button></div></form></div>`;
    panel.querySelector('#registration-replay')!.addEventListener('click',()=>narrate(spokenSteps[step]));
    narrate(spokenSteps[step]);
    if(step===5){
      panel.querySelector<HTMLSelectElement>('#registration-model-provider')!.value=chosenProvider;
      panel.querySelector<HTMLSelectElement>('#registration-local-model')!.value=chosenLocalModel;
      panel.querySelector<HTMLInputElement>('#registration-api-key')!.value=pendingProviderKey;
      panel.querySelector<HTMLInputElement>('#registration-vault-password')!.value=pendingVaultPassword;
    }
    const capture=()=>{
      panel.querySelectorAll<HTMLInputElement>('[data-consent]').forEach(el=>c[el.dataset.consent!]=el.checked);
      panel.querySelectorAll<HTMLInputElement|HTMLSelectElement>('[data-field]').forEach(el=>{
        const k=el.dataset.field!;const contact=el.closest<HTMLElement>('[data-contact]');const reminder=el.closest<HTMLElement>('[data-reminder]');
        const dest=contact?draft.contacts[Number(contact.dataset.contact)]:reminder?draft.reminders[Number(reminder.dataset.reminder)]:draft;
        dest[k]=el instanceof HTMLInputElement && el.type==='checkbox'?el.checked:['lead_days','emergency_priority'].includes(k)?Number(el.value):['interests','preferences'].includes(k)?el.value.split(',').map(s=>s.trim()).filter(Boolean):el.value.trim();
      });
      const phrases=panel.querySelector<HTMLTextAreaElement>('#phrasebook');if(phrases) {draft.phrasebook={};for(const line of phrases.value.split('\n')){const n=line.indexOf('=');if(n>0)draft.phrasebook[line.slice(0,n).trim().toLowerCase()]=line.slice(n+1).trim();}}
      const age=panel.querySelector<HTMLElement>('#derived-age');
      if(age){const born=draft.birthdate?new Date(`${draft.birthdate}T12:00:00Z`):null;const today=new Date();const before=born && (today.getMonth()+1<born.getUTCMonth()+1 || (today.getMonth()+1===born.getUTCMonth()+1 && today.getDate()<born.getUTCDate()));const years=born?today.getFullYear()-born.getUTCFullYear()-Number(before):null;age.textContent=years!==null && years>=0?`Age: ${years} years (calculated, not stored)`:'Age is calculated from your birth date.';}
      const save=panel.querySelector<HTMLInputElement>('#draft-consent');if(save)savedDraft=save.checked;
      if(step===5){
        chosenProvider=panel.querySelector<HTMLSelectElement>('#registration-model-provider')!.value;
        chosenLocalModel=panel.querySelector<HTMLSelectElement>('#registration-local-model')!.value;
        chosenOnlineModel=panel.querySelector<HTMLSelectElement>('#registration-online-model')!.value;
        pendingHfToken=panel.querySelector<HTMLInputElement>('#registration-hf-token')!.value;
        pendingProviderKey=panel.querySelector<HTMLInputElement>('#registration-api-key')!.value;
        pendingVaultPassword=panel.querySelector<HTMLInputElement>('#registration-vault-password')!.value;
      }
      const saved={draft:structuredClone(draft),step};
      draftWrite=draftWrite.then(()=>savedDraft && c.profile_storage && (c.contacts_storage || !saved.draft.contacts.length)
        ?invoke('profile_draft_save',{draft:saved}):invoke('profile_draft_clear')).catch(()=>{});
    };
    panel.querySelector('form')!.addEventListener('input',capture);
    panel.querySelector('form')!.addEventListener('change',capture);
    panel.querySelector('#registration-back')!.addEventListener('click',()=>{capture();step--;render();});
    panel.querySelector('#registration-later')!.addEventListener('click',()=>{capture();stopNarration();panel.classList.add('hidden');});
    panel.querySelector('#registration-skip-model')?.addEventListener('click',async()=>{
      capture();
      try{
        if(pendingHfToken){
          await invoke('llm_unlock',{password:pendingVaultPassword});
          await invoke('llm_store_key',{provider:'huggingface',apiKey:pendingHfToken});
          pendingHfToken='';pendingVaultPassword='';
        }
        step=6;render();
      }catch(error){panel.querySelector('#registration-error')!.textContent=`Could not save Hugging Face token: ${String(error)}`;}
    });
    if(step===5){
      let latestStatus:any=null;
      const refreshLocalChoice=()=>{
        const local=panel.querySelector<HTMLOptionElement>('#registration-model-provider option[value="local"]');
        const model=panel.querySelector<HTMLSelectElement>('#registration-local-model')?.value;
        const ready=latestStatus?.local_runtime_ready && (latestStatus?.local_models||[]).some((entry:any)=>entry.id===model);
        if(local){local.disabled=!ready;local.textContent=ready?'Local GGUF model (BestBuddy assets ready)':'Local GGUF model (download needed)';}
      };
      panel.querySelector('#registration-local-model')?.addEventListener('change',refreshLocalChoice);
      void invoke<any>('llm_status').then(status=>{latestStatus=status;refreshLocalChoice();}).catch(()=>{});
      void invoke<any[]>('llm_free_models').then(models=>{
        const select=panel.querySelector<HTMLSelectElement>('#registration-online-model');
        const status=panel.querySelector<HTMLElement>('#registration-free-model-status');
        if(!select||!status)return;
        const shown=models.slice(0,60);
        select.innerHTML='<option value="openrouter/free">Free model router</option>'+shown.map(model=>`<option value="${escape(model.id)}">${escape(model.name)}</option>`).join('');
        select.value=Array.from(select.options).some(option=>option.value===chosenOnlineModel)?chosenOnlineModel:'openrouter/free';
        status.textContent=`${models.length} current free text models; ${shown.length} listed. Model availability is checked live.`;
      }).catch(error=>{const status=panel.querySelector<HTMLElement>('#registration-free-model-status');if(status)status.textContent=`Could not load the live free-model list: ${String(error)}`;});
    }
    panel.querySelector('#registration-prepare-local')?.addEventListener('click',async()=>{
      const button=panel.querySelector<HTMLButtonElement>('#registration-prepare-local')!;
      const model=panel.querySelector<HTMLSelectElement>('#registration-local-model')!.value;
      const status=panel.querySelector<HTMLElement>('#registration-model-status')!;
      button.disabled=true;status.textContent=`Preparing ${model}; checking reusable files first…`;
      const polling=setInterval(()=>void invoke<any>('llm_status').then(value=>{status.textContent=value.local_preparation_status||status.textContent;}).catch(()=>{}),2000);
      try{await invoke('llm_prepare_local',{model});status.textContent=`${model} passed a local conversation test.`;const option=panel.querySelector<HTMLOptionElement>('#registration-model-provider option[value="local"]');if(option)option.disabled=false;}
      catch(error){status.textContent=`Preparation failed: ${String(error)}`;}
      finally{clearInterval(polling);button.disabled=false;}
    });
    panel.querySelectorAll<HTMLButtonElement>('[data-model-guide]').forEach(button=>button.addEventListener('click',()=>{
      const url=button.dataset.modelGuide!;
      const popup=panel.querySelector<HTMLElement>('#registration-guide-popup');
      if(!popup){void invoke('open_external',{url}).catch(e=>{panel.querySelector('#registration-error')!.textContent=String(e);});return;}
      const guide=url.includes('huggingface.co/settings')?['Hugging Face token','Create a free Hugging Face account, open Access Tokens, choose a fine-grained token with the access needed for your model, copy it once, and paste it into the Hugging Face token field. BestBuddy encrypts it after you enter a credential password.']:url.includes('groq.com')?['Groq API key','Sign in to Groq Console, open API Keys, create a key, and paste it into the selected provider key field. Review your account rate limits before use.']:url.includes('openrouter.ai')?['OpenRouter API key','Register with OpenRouter, create an API key, and paste it into the selected provider key field. One key works with the currently available free models in the list.']:url.includes('aistudio.google.com')?['Gemini API key','Sign in to Google AI Studio, create an API key for a free-tier project, and paste it into the selected provider key field. Check the current model and limits before use.']:['Local model information','Review the model page and file size. BestBuddy downloads and verifies the selected compact model in its own private app-data folder.'];
      popup.querySelector<HTMLElement>('#registration-guide-title')!.textContent=guide[0];
      popup.querySelector<HTMLElement>('#registration-guide-text')!.textContent=guide[1];
      popup.classList.remove('hidden');
      popup.querySelector<HTMLButtonElement>('#registration-guide-open')!.onclick=()=>void invoke('open_external',{url}).catch(e=>{panel.querySelector('#registration-error')!.textContent=String(e);});
      popup.querySelector<HTMLButtonElement>('#registration-guide-close')!.onclick=()=>popup.classList.add('hidden');
    }));
    panel.querySelector('#add-contact')?.addEventListener('click',()=>{capture();draft.contacts.push({id:crypto.randomUUID(),name:'',relationship:'',phone:'',email:'',signal_link:'',messenger_link:'',slack_link:'',discord_link:'',zoom_link:'',teams_link:'',birthday:'',anniversary:'',channel:'whatsapp',emergency_priority:0});render();});
    panel.querySelector('#add-reminder')?.addEventListener('click',()=>{capture();draft.reminders.push({id:crypto.randomUUID(),title:'',date:'',time:'09:00',annual:true,lead_days:0,enabled:true,channel:'in_app',contact_id:''});render();});
    panel.querySelectorAll<HTMLElement>('[data-remove-contact]').forEach(el=>el.onclick=()=>{capture();draft.contacts.splice(Number(el.dataset.removeContact),1);render();});
    panel.querySelectorAll<HTMLElement>('[data-remove-reminder]').forEach(el=>el.onclick=()=>{capture();draft.reminders.splice(Number(el.dataset.removeReminder),1);render();});
    panel.querySelector('form')!.addEventListener('submit',async e=>{
      e.preventDefault();capture();const error=panel.querySelector('#registration-error')!;
      try {
        if(step===1){if(!draft.name)throw Error('Please enter your preferred name.');new Intl.DateTimeFormat('en',{timeZone:draft.timezone});if(draft.birthdate && (Number.isNaN(Date.parse(draft.birthdate)) || draft.birthdate>new Date().toISOString().slice(0,10)))throw Error('Check the birth date.');}
        if(step===2 && draft.contacts.some((x:any)=>!x.name || (x.emergency_priority>0 && !x.phone && !x.email)))throw Error('Each contact needs a name; emergency contacts also need a phone or email.');
        if(step===3 && draft.reminders.some((x:any)=>!x.title||!x.date||x.lead_days<0||x.lead_days>366||!/^(?:[01]\d|2[0-3]):[0-5]\d$/.test(x.time||'09:00')))throw Error('Check reminder titles, dates, local times and lead days (0–366).');
        if(step===5){
          const selected=panel.querySelector<HTMLSelectElement>('#registration-model-provider')!.value;
          const password=pendingVaultPassword;
          const hfToken=pendingHfToken.trim();
          const apiKey=pendingProviderKey.trim()||(selected==='huggingface'?hfToken:'');
          if(hfToken || (selected!=='rules' && selected!=='auto' && selected!=='local')){
            if(!password)throw Error('Enter a credential password to encrypt your keys, or choose Skip model setup.');
            await invoke('llm_unlock',{password});
            if(hfToken)await invoke('llm_store_key',{provider:'huggingface',apiKey:hfToken});
          }
          if(selected==='rules' || selected==='auto'){
            await invoke('llm_configure',{settings:{provider:selected,online_provider:'groq',online_model:'openai/gpt-oss-20b',local_model:chosenLocalModel},apiKey:null});
          } else if(selected==='local'){
            await invoke('llm_configure',{settings:{provider:'local',online_provider:'groq',online_model:'openai/gpt-oss-20b',local_model:chosenLocalModel},apiKey:null});
          } else {
            if(!apiKey)throw Error('Enter the provider key, or choose Skip model setup.');
            const model=({groq:'openai/gpt-oss-20b',openrouter:chosenOnlineModel,huggingface:'openai/gpt-oss-120b:fastest',gemini:'gemini-2.5-flash-lite'} as Record<string,string>)[selected];
            await invoke('llm_configure',{settings:{provider:'auto',online_provider:selected,online_model:model,local_model:chosenLocalModel},apiKey});
          }
          pendingHfToken='';pendingProviderKey='';pendingVaultPassword='';
          step++;render();return;
        }
        if(step<6){step++;render();return;}
        if(!c.profile_storage)throw Error('Choose profile storage on the first step before saving personal details, or Continue later to use BestBuddy without saving.');
        draft.consents.recorded_at=new Date().toISOString();draft.consent=c.profile_storage;draft.onboarding_complete=true;
        const p=await invoke('profile_save',{profile:draft});setProfile(p);await draftWrite;await invoke('profile_draft_clear');stopNarration();panel.classList.add('hidden');await refreshed();if(c.microphone&&c.voice_processing){try{await invoke('voice_permission_request');notify('Your profile is saved. BestBuddy is preparing wake-word listening.');}catch(e){notify(`Your profile is saved. Voice permission: ${String(e)}`);}}else notify('Your profile is saved. Microphone listening is off; text and speaker playback remain available.');
      } catch(err){error.textContent=String(err);}
    });
    panel.querySelector<HTMLInputElement>('input')?.focus();
  };
  const open=async(force=false)=>{
    let p=getProfile();
    if(!force){try{p=await invoke('profile_get');setProfile(p);}catch(e){notify(`Profile unavailable: ${e}`);}}
    if(!force && p.onboarding_complete)return;
    const legacyDraft=force?null:stored('buddy.registrationDraft',null);localStorage.removeItem('buddy.registrationDraft');
    const protectedDraft=force?null:await invoke<any>('profile_draft_get').catch(()=>null);
    const resumed=protectedDraft||legacyDraft;draft=structuredClone(resumed?.draft||p);step=resumed?.step||0;savedDraft=!!resumed;
    if(legacyDraft&&!protectedDraft)draftWrite=invoke('profile_draft_save',{draft:legacyDraft}).catch(()=>{});
    draft.consents={...Object.fromEntries(Object.keys(consentLabels).map(k=>[k,false])),...draft.consents};draft.contacts||=[];draft.reminders||=[];draft.timezone||=Intl.DateTimeFormat().resolvedOptions().timeZone;draft.language||='English';draft.style||='friendly';
    panel.classList.remove('hidden');if(!force && !resumed)renderSplash();else render();
  };
  document.querySelector<HTMLElement>('.settings-nav-actions')!.insertAdjacentHTML('beforeend','<button id="contacts" class="action-btn ghost" type="button">Contacts</button><button id="profile-export" class="action-btn ghost" type="button">Export my data</button><button id="profile-erase" class="action-btn ghost" type="button">Delete my local data</button>');
  document.querySelector<HTMLElement>('#learning-settings-actions')!.insertAdjacentHTML('beforeend','<button id="memory-view" class="action-btn ghost" type="button">Knowledge & sources</button>');
  const report=(message:string)=>{const output=document.querySelector<HTMLElement>('#settings-action-result');if(output)output.textContent=message;else notify(message);};
  const display=(title:string,body:string)=>{
    const detail=title==='Your contacts'?'#profile-settings-detail':title==='Knowledge & conversation sources'?'#learning-settings-detail':null;
    if(detail){
      const target=document.querySelector<HTMLElement>(detail)!;
      target.innerHTML=`<h4>${escape(title)}</h4>${body}`;
      target.scrollIntoView({block:'nearest',behavior:'smooth'});
      return;
    }
    document.querySelector('#settings-panel')?.classList.add('hidden');
    document.querySelector('#media-modal')!.classList.remove('hidden');
    document.querySelector('#media-content')!.innerHTML=`<h2>${escape(title)}</h2>${body}`;
  };
  document.querySelector('#memory-view')!.addEventListener('click',async()=>{try{
    const [knowledge,lexicon,conversation]=await Promise.all([invoke('knowledge'),invoke('lexicon'),invoke<any[]>('conversation_items')]);
    const cues=Array.isArray(conversation)?conversation:[];
    const episodes=cues.map((cue:any,index:number)=>`<div class="contact-choice"><strong>${escape(cue.title||'Untitled')}</strong><div>${escape(cue.source||'Publisher')}</div><p>${escape(cue.synopsis||'')}</p><button class="cue-source" data-cue-index="${index}">Open publisher page ↗</button></div>`).join('')||'<p>Run Prefetch now to collect podcast and debate references.</p>';
    display('Knowledge & conversation sources',`<h3>Podcast and debate references (${cues.length})</h3>${episodes}<details><summary>News references</summary><pre>${escape(JSON.stringify(knowledge,null,2))}</pre></details><details><summary>Personal lexicon</summary><pre>${escape(JSON.stringify(lexicon,null,2))}</pre></details>`);
    document.querySelectorAll<HTMLButtonElement>('.cue-source').forEach(button=>button.addEventListener('click',()=>{const cue=cues[Number(button.dataset.cueIndex)];if(cue?.url)void invoke('open_external',{url:cue.url}).catch(e=>notify(String(e)));}));
  }catch(e){notify(String(e));}});
  document.querySelector('#profile-export')!.addEventListener('click',async()=>{
    try {
      const data=await invoke('profile_export');
      const blob=new Blob([JSON.stringify(data,null,2)],{type:'application/json'});
      const url=URL.createObjectURL(blob);const link=document.createElement('a');
      link.href=url;link.download=`buddy-personal-data-${new Date().toISOString().slice(0,10)}.json`;
      document.body.append(link);link.click();link.remove();setTimeout(()=>URL.revokeObjectURL(url),30000);
      notify('Personal data export requested. Choose a safe location for the downloaded JSON file.');
    } catch(e){notify(`Could not export data: ${e}`);}
  });
  document.querySelector('#profile-erase')!.addEventListener('click',()=>{
    display('Delete local BestBuddy data',`<p>This removes the saved profile, contacts, reminders, lexicon, prefetched knowledge and conversation sources, saved music files and BestBuddy log on this device. The action cannot be undone. Export first if you want a copy.</p><label>Type DELETE MY BUDDY DATA<input id="erase-phrase" autocomplete="off" spellcheck="false"></label><button id="erase-confirm" disabled>Delete local data</button><p id="erase-error" role="alert"></p>`);
    const input=document.querySelector<HTMLInputElement>('#erase-phrase')!;
    const button=document.querySelector<HTMLButtonElement>('#erase-confirm')!;
    input.addEventListener('input',()=>button.disabled=input.value!=='DELETE MY BUDDY DATA');
    button.addEventListener('click',async()=>{
      button.disabled=true;
      try {
        await invoke('profile_erase',{confirmation:input.value});
        await new Promise<void>((resolve,reject)=>{const request=indexedDB.deleteDatabase('buddy-music-library');request.onsuccess=()=>resolve();request.onerror=()=>reject(request.error);request.onblocked=()=>reject(Error('Close other BestBuddy windows and try again to remove saved music.'));});
        for(const key of Object.keys(localStorage))if(key.startsWith('buddy.'))localStorage.removeItem(key);
        location.reload();
      }catch(e){document.querySelector('#erase-error')!.textContent=String(e);button.disabled=false;}
    });
    input.focus();
  });
  document.querySelector('#contacts')!.addEventListener('click',()=>{
    const p=getProfile();display('Your contacts',`<p>Choose a contact and app. Review and send there. Presence, messages and calls remain inside the selected app.</p>${(p.contacts||[]).map((c:any,i:number)=>`<div class="contact-choice"><strong>${escape(c.name)} · ${escape(c.relationship)}</strong><div>${['whatsapp','messenger','slack','discord','zoom','teams','telegram','signal','phone','sms','email'].filter(channel=>['messenger','slack','discord','zoom','teams','signal'].includes(channel)?!!c[`${channel}_link`]:channel==='email'?!!c.email:!!c.phone).map(channel=>`<button class="contact-action" data-contact-id="${i}" data-channel="${channel}">${channel===c.channel?'★ ':''}${escape(channel)}</button>`).join('')}</div></div>`).join('')||'<p>Add contacts in your profile first.</p>'}`);
    document.querySelectorAll<HTMLElement>('.contact-action').forEach(b=>b.onclick=async()=>{try{const c=p.contacts[Number(b.dataset.contactId)];await invoke('contact_open',{contactId:c.id,channel:b.dataset.channel});}catch(e){notify(String(e));}});
  });
  return {open,checkReminders:()=>{
    const p=getProfile();if(!p.onboarding_complete)return;
    try {
      const now=new Date();const timezone=p.timezone||Intl.DateTimeFormat().resolvedOptions().timeZone;
      const {day}=currentDayAndTime(now,timezone);
      const delivered=stored('buddy.reminders.delivered',{});
      const lastChecked=localStorage.getItem('buddy.reminders.lastChecked');
      const due=dueReminders(listReminderEvents(p),now,timezone,lastChecked,new Set(Object.keys(delivered)));
      for(const reminder of due){
        const when=reminder.missed?`missed on ${reminder.dueDate}`:reminder.lead_days?`in ${reminder.lead_days} day(s)`:'today';
        notify(`Reminder: ${reminder.title} — ${when}.`);
        delivered[deliveryKey(reminder)]=day;
      }
      for(const [key,when] of Object.entries(delivered)) if(typeof when!=='string'||when<new Date(now.getTime()-32*86400000).toISOString().slice(0,10))delete delivered[key];
      localStorage.setItem('buddy.reminders.delivered',JSON.stringify(delivered));
      localStorage.setItem('buddy.reminders.lastChecked',day);
    }catch(e){notify(`Reminder check failed: ${e}`);}
  }};
}
