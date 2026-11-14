import {preparationWait,waitForVoicePreparation,onlinePlanGuide} from './setup_patch';
import {localChoiceUsable,installLocalScan,waitForModelPreparation} from './local_models_ui';
import {speakGuidance,stopGuidance,quietSpeech} from './speech';
import {restoreSetup,routingFor,stageSnapshot} from './onboarding_policy';
import {modelSetupHtml,reviewSetupHtml,decorateRegistration,saveRegistrationVoice,registrationVoice,restoreRegistrationVoice} from './registration_layout';
import {readQuiet,validQuietPeriods} from './settings_policy';
import { invoke } from '@tauri-apps/api/core';
import { currentDayAndTime, deliveryKey, dueReminders, listReminderEvents } from './reminders';

const escape = (s: unknown) => String(s ?? '').replace(/[&<>"']/g, c => ({'&':'&amp;','<':'&lt;','>':'&gt;','"':'&quot;',"'":'&#39;'}[c]!));
const consentLabels: Record<string,string> = {
  profile_storage: 'Save my profile on this device (required to save registration)',
  contacts_storage: 'Store the family, friend and emergency contact details I enter',
  personalization: 'Remember interests and learn my conversational vocabulary',
  microphone: 'Allow Smart Butler to activate the microphone after registration',
  voice_processing: 'Allow local recognition and processing of what I say',
  live_information: 'Allow live news and information searches using my selected location and interests',
  external_services: 'Allow third-party information services to receive these searches (city, region or topics may be sent)',
};
function stored(key: string, fallback: any) { try { return JSON.parse(localStorage.getItem(key) || 'null') ?? fallback; } catch { return fallback; } }
function field(label: string, key: string, value: unknown, type='text') {
  return `<label>${label}<input data-field="${key}" type="${type}" value="${escape(value)}"></label>`;
}
export function setupCompanion(getProfile:()=>any, setProfile:(p:any)=>void, notify:(s:string)=>void, refreshed:()=>Promise<void>) {
  let draft:any; let step=0; let savedDraft=false;
  let draftWrite:Promise<unknown>=Promise.resolve();
  let pendingHfToken=''; let pendingProviderKey=''; let pendingVaultPassword=''; let pendingKeys:Record<string,string>={};
  let chosenProvider='rules'; let chosenLocalModel=''; let chosenOnlineModel='openrouter/free';
  const steps=['Welcome & privacy','Your profile','Family & emergency contacts','Important dates','Smart Butler preferences','Conversation models','Review & save'];
  const panel=document.querySelector<HTMLElement>('#onboarding')!;
  panel.setAttribute('role','dialog'); panel.setAttribute('aria-modal','true'); panel.setAttribute('aria-label','Smart Butler registration');
  const spokenSteps=[
    'Welcome to Smart Butler. This beta is free to test. Your saved profile, contacts, reminders and learning files are encrypted for your Windows account. Online providers use the internet only when you configure and use them. You can review each choice before continuing.',
    'Tell me what name you prefer, and optionally add your birth date, location and language. You may leave optional details blank.',
    'Add family, friends or emergency contacts only with permission. Smart Butler can open communication apps, but cannot place emergency calls or read who is online.',
    'Add birthdays, anniversaries and other important dates. Reminders appear inside Smart Butler; external delivery is not configured.',
    'Choose the conversation style you like and add your own phrases. Smart Butler checks local commands and personal vocabulary first.',
    'Choose an optional compact local model or an online provider. Online conversation sends what you say to that provider. You can skip model setup and return later.',
    'Review your choices. Windows or Android controls microphone permission. Smart Butler is in beta testing; please verify important information independently.'
  ];
  const stopNarration=()=>{stopGuidance();panel.classList.remove('butler-speaking');};
  const narrate=(message:string,explicit=false)=>speakGuidance(message,'onboarding','Smart Butler',explicit);
  const renderSplash=()=>{
    (panel.querySelector<HTMLElement>(':scope>.modal-dialog')||panel).innerHTML=`<div class="registration-splash"><div class="splash-orbit"><img src="/assets/butler-welcome.png" alt="Smart Butler" class="splash-butler-mark"></div><div class="kicker">WELCOME TO SMART BUTLER</div><h2>At your service</h2><p id="splash-words">Hello. I’m Smart Butler. I’ll guide you through a few choices about your profile, voice and privacy.</p><p class="splash-beta">Beta preview · Free to test · Your choices stay editable in Settings</p><div class="registration-actions"><div class="splash-small-actions"><button type="button" id="splash-replay">Hear again</button><button type="button" data-quiet-speech>Quiet</button></div><button type="button" id="splash-begin" class="primary-wide">Get started</button></div></div>`;
    panel.querySelector('#splash-replay')!.addEventListener('click',()=>narrate('Hello. I’m Smart Butler. I’ll guide you through a few choices about your profile, voice and privacy.',true));
    panel.querySelector('#splash-begin')!.addEventListener('click',()=>{stopNarration();render();});
    narrate('Hello. I’m Smart Butler. I’ll guide you through a few choices about your profile, voice and privacy.');
  };
  const render=()=>{
    const c=draft.consents;panel.dataset.registrationStep=String(step);
    let body='';
    if(step===0) body=`<div class="onboard-section"><h3>Private local profile</h3><p>This is device registration; it does not create an online account. Saved profile, contacts, reminders and learning files are encrypted and bound to your Windows user account. Provider keys use the separate password-protected credential vault.</p></div><div class="onboard-section"><h3>Your choices</h3>${Object.entries(consentLabels).map(([k,l])=>`<label class="check"><input type="checkbox" data-consent="${k}" ${c[k]?'checked':''}>${l}</label>`).join('')}<label class="check"><input id="draft-consent" type="checkbox" ${savedDraft?'checked':''}>Save this registration draft on this device so I can resume later</label></div><p class="onboard-note">Voice listening starts only after you complete registration and enable both voice choices. Your device also controls microphone permission. Live widgets may require internet access.</p>`;
    if(step===1) body=field('Preferred name','name',draft.name==='Friend'?'':draft.name)+field('Birth date (optional)','birthdate',draft.birthdate,'date')+`<p id="derived-age">Age is calculated from your birth date.</p>`+field('City','location',draft.location)+field('Region','region',draft.region)+field('Country','country',draft.country)+field('Time zone (IANA name)','timezone',draft.timezone)+field('Preferred language','language',draft.language)+`<p>Installed speech models currently determine available voice languages.</p>`;
    if(step===2) body=`<p>Add only contact details you are allowed to store. Emergency priority 1 is primary; 2 is secondary. Smart Butler does not automatically contact emergency services.</p>${draft.contacts.map((x:any,i:number)=>`<fieldset data-contact="${i}"><legend>${escape(x.name||`Contact ${i+1}`)}</legend>${field('Name','name',x.name)}${field('Relationship','relationship',x.relationship)}${field('Phone (include country code)','phone',x.phone,'tel')}${field('Email','email',x.email,'email')}${field('Signal share link (from Signal app)','signal_link',x.signal_link,'url')}${field('Messenger conversation link (copy from browser)','messenger_link',x.messenger_link,'url')}${field('Slack conversation link','slack_link',x.slack_link,'url')}${field('Discord conversation link','discord_link',x.discord_link,'url')}${field('Zoom meeting link','zoom_link',x.zoom_link,'url')}${field('Teams meeting link','teams_link',x.teams_link,'url')}${field('Birthday','birthday',x.birthday,'date')}${field('Anniversary','anniversary',x.anniversary,'date')}<label>Preferred communication<select data-field="channel">${['whatsapp','messenger','slack','discord','zoom','teams','telegram','signal','phone','sms','email'].map(k=>`<option ${x.channel===k?'selected':''}>${k}</option>`).join('')}</select></label><label>Emergency priority<select data-field="emergency_priority">${[0,1,2].map(n=>`<option value="${n}" ${x.emergency_priority===n?'selected':''}>${n===0?'Not an emergency contact':n}</option>`).join('')}</select></label><button type="button" data-remove-contact="${i}">Remove contact</button></fieldset>`).join('')}<button id="add-contact" type="button" ${c.contacts_storage?'':'disabled'}>Add contact</button>${!c.contacts_storage?'<p>Enable contact storage on the privacy step to add contacts.</p>':''}`;
    if(step===3) body=`<p>Reminders appear inside Smart Butler at your chosen local time. If Smart Butler was closed, it shows reminders missed in the last seven days when reopened. Birthdays and anniversaries from Contacts are included automatically. External delivery is not configured.</p>${draft.reminders.map((x:any,i:number)=>`<fieldset data-reminder="${i}"><legend>Reminder ${i+1}</legend>${field('Title','title',x.title)}${field('Date','date',x.date,'date')}${field('Time in your profile time zone','time',x.time||'09:00','time')}${field('Remind how many days before (0–366)','lead_days',x.lead_days,'number')}<label class="check"><input type="checkbox" data-field="annual" ${x.annual?'checked':''}>Repeat annually</label><label class="check"><input type="checkbox" data-field="enabled" ${x.enabled?'checked':''}>Enabled</label><button type="button" data-remove-reminder="${i}">Remove reminder</button></fieldset>`).join('')}<button id="add-reminder" type="button">Add important date</button>`;
    if(step===4) body=field('Interests (comma separated)','interests',(draft.interests||[]).join(', '))+field('Music and other preferences (comma separated)','preferences',(draft.preferences||[]).join(', '))+field('Dislikes / topics to avoid','dislikes',(draft.dislikes||[]).join(', '))+`<label>Conversation style<select data-field="style">${['friendly','formal','playful'].map(k=>`<option ${draft.style===k?'selected':''}>${k}</option>`).join('')}</select></label><label>Your phrasebook (one command = reply per line)<textarea id="phrasebook" rows="6">${escape(Object.entries(draft.phrasebook||{}).map(([k,v])=>`${k} = ${v}`).join('\n'))}</textarea></label><p>Exact phrasebook matches run locally. Live knowledge has a 24-hour retrieval limit. Personalization controls learned conversational vocabulary.</p>`;
    if(step===5) body=modelSetupHtml('Smart Butler',pendingHfToken);
    if(step===5) body=body.replace('<option value="huggingface">Automatic + Hugging Face online fallback</option>','<option value="huggingface">Automatic + Hugging Face online fallback</option><option value="gemini">Automatic + Google Gemini free-tier fallback</option>')
      +`<div id="registration-guide-popup" class="registration-guide-popup hidden" role="dialog" aria-modal="true" aria-label="Account setup guide"><div class="registration-guide-card"><h3 id="registration-guide-title">Account setup</h3><p id="registration-guide-text"></p><div class="registration-actions"><button id="registration-guide-open" type="button">Open official page ↗</button><button id="registration-guide-close" type="button">Close guide</button></div></div></div>`;
    if(step===6) body=reviewSetupHtml(draft,pendingVaultPassword)+`<section class="onboard-section registration-summary"><h3>Review summary</h3><p>${Object.entries(draft.consents).filter(([key,value])=>key!=='recorded_at'&&value).map(([key])=>escape(consentLabels[key]||key)).join(' · ')||'Optional permissions off'}</p><p>Local model: ${escape(chosenLocalModel||'None · built-in offline responses')}<br>Online: ${escape(['rules','auto','local'].includes(chosenProvider)?'Off':chosenProvider)}<br>Voice: ${escape(registrationVoice()||'Current installed voice')}</p></section>`;
    (panel.querySelector<HTMLElement>(':scope>.modal-dialog')||panel).innerHTML=`<div class="modal-card registration"><div class="registration-guide"><img src="/assets/butler-welcome.png" alt="Smart Butler" class="registration-butler-mark"><div><h2>${steps[step]}</h2><p id="registration-narration" class="sr-only">${escape(spokenSteps[step])}</p><button type="button" id="registration-replay">Hear this step</button><button type="button" data-quiet-speech>Quiet</button></div></div><p>Step ${step+1} of ${steps.length}</p><progress value="${step+1}" max="${steps.length}"></progress><form id="registration-form"><div class="registration-body">${body}</div><p id="registration-error" role="status" aria-live="polite"></p><small>Save & Next keeps an encrypted draft on this device when profile storage is enabled. Tokens and passwords are saved only with your final app password. English remains the fallback language.</small><div class="registration-actions"><button type="button" id="registration-back" ${step===0?'disabled':''}>Back</button><button type="button" id="registration-later">Continue later</button><button type="submit">${step===6?'Save and start':'Save & Next'}</button></div></form></div>`;
    panel.querySelector('#registration-replay')!.addEventListener('click',()=>narrate(spokenSteps[step],true));
    narrate(spokenSteps[step]);
    if(step===5){
      panel.querySelector<HTMLSelectElement>('#registration-model-provider')!.value=chosenProvider;
      const initialLocal=panel.querySelector<HTMLSelectElement>('#registration-local-model')!;if(chosenLocalModel&&![...initialLocal.options].some(o=>o.value===chosenLocalModel))initialLocal.add(new Option(chosenLocalModel+' · saved choice',chosenLocalModel));initialLocal.value=chosenLocalModel;
      panel.querySelector<HTMLInputElement>('#registration-api-key')!.value=pendingProviderKey;
      panel.querySelector<HTMLInputElement>('#registration-vault-password')!.value=pendingVaultPassword;
    }
    if(step===6){panel.querySelector<HTMLSelectElement>('#registration-model-provider')!.value=chosenProvider;}
    const capture=()=>{
      panel.querySelectorAll<HTMLInputElement>('[data-consent]').forEach(el=>c[el.dataset.consent!]=el.checked);
      panel.querySelectorAll<HTMLInputElement|HTMLSelectElement>('[data-field]').forEach(el=>{
        const k=el.dataset.field!;const contact=el.closest<HTMLElement>('[data-contact]');const reminder=el.closest<HTMLElement>('[data-reminder]');
        const dest=contact?draft.contacts[Number(contact.dataset.contact)]:reminder?draft.reminders[Number(reminder.dataset.reminder)]:draft;
        dest[k]=el instanceof HTMLInputElement && el.type==='checkbox'?el.checked:['lead_days','emergency_priority'].includes(k)?Number(el.value):['interests','preferences','dislikes'].includes(k)?el.value.split(',').map(s=>s.trim()).filter(Boolean):el.value.trim();
      });
      const phrases=panel.querySelector<HTMLTextAreaElement>('#phrasebook');if(phrases) {draft.phrasebook={};for(const line of phrases.value.split('\n')){const n=line.indexOf('=');if(n>0)draft.phrasebook[line.slice(0,n).trim().toLowerCase()]=line.slice(n+1).trim();}}
      const age=panel.querySelector<HTMLElement>('#derived-age');
      if(age){const born=draft.birthdate?new Date(`${draft.birthdate}T12:00:00Z`):null;const today=new Date();const before=born && (today.getMonth()+1<born.getUTCMonth()+1 || (today.getMonth()+1===born.getUTCMonth()+1 && today.getDate()<born.getUTCDate()));const years=born?today.getFullYear()-born.getUTCFullYear()-Number(before):null;age.textContent=years!==null && years>=0?`Age: ${years} years (calculated, not stored)`:'Age is calculated from your birth date.';}
      const save=panel.querySelector<HTMLInputElement>('#draft-consent');if(save)savedDraft=save.checked;
      if(step===5){
        chosenProvider=panel.querySelector<HTMLSelectElement>('#registration-model-provider')!.value;
        chosenLocalModel=panel.querySelector<HTMLSelectElement>('#registration-local-model')!.value;
        chosenOnlineModel=panel.querySelector<HTMLSelectElement>('#registration-online-model')?.value||chosenOnlineModel;
        pendingHfToken=panel.querySelector<HTMLInputElement>('#registration-hf-token')!.value;
        pendingProviderKey=panel.querySelector<HTMLInputElement>('#registration-api-key')!.value;
        pendingVaultPassword=panel.querySelector<HTMLInputElement>('#registration-vault-password')!.value;
      }
      if(step===5){const language=panel.querySelector<HTMLSelectElement>('[data-voice-language]')?.value;if(language&&language!=='all')draft.language=language;panel.querySelectorAll<HTMLInputElement>('[data-provider-key]').forEach(input=>{if(input.value.trim())pendingKeys[input.dataset.providerKey!]=input.value.trim();});}
      if(step===6){chosenProvider=panel.querySelector<HTMLSelectElement>('#registration-model-provider')!.value;pendingVaultPassword=panel.querySelector<HTMLInputElement>('#registration-vault-password')!.value;const periods=readQuiet(panel);draft.conversation_routine={...(draft.conversation_routine||{}),quiet_enabled:periods.some(p=>p.enabled),quiet_periods:periods};}
      const saved=stageSnapshot(draft,step,{provider:chosenProvider,local_model:chosenLocalModel,online_model:chosenOnlineModel,voice:registrationVoice()});
      if(savedDraft)draftWrite=draftWrite.catch(()=>{}).then(()=>invoke('profile_draft_save',{draft:saved}));

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
      let latestStatus:any=null;let localRuntimeAvailable=true;
      const localSelect=panel.querySelector<HTMLSelectElement>('#registration-local-model')!;
      const refreshModels=async()=>{capture();const previous=chosenLocalModel;try{const catalog:any=await invoke('llm_local_catalog',{allowNetwork:!!c.external_services});if(!localSelect.isConnected)return;localRuntimeAvailable=catalog.runtime_available!==false;localSelect.replaceChildren(new Option('No additional model · built-in offline responses',''));for(const model of catalog.models||[])localSelect.add(new Option(`${model.name} · ${(Number(model.size_bytes)/1e9).toFixed(2)} GB · ${model.installed?'Installed':model.availability==='missing'?'Repository unavailable':model.availability==='gated'?'Token / terms required':'Download'}`,model.id));for(const model of catalog.local_available||[])if(![...localSelect.options].some(o=>o.value===model.id))localSelect.add(new Option(`${model.name} · Available in ${model.source}${model.size_bytes?' · '+(model.size_bytes/1e9).toFixed(2)+' GB':''}`,model.id));if(previous&&![...localSelect.options].some(o=>o.value===previous))localSelect.add(new Option(previous+' · saved choice (currently unavailable)',previous));localSelect.value=previous;refreshLocalChoice();panel.querySelector('#registration-model-status')!.textContent='Compatible models up to 3 GB, ordered by Hugging Face downloads. Native local generative inference is not enabled in this Android engineering build. Built-in offline responses remain available; online AI is optional.';}catch(error){panel.querySelector('#registration-model-status')!.textContent='Bundled compatible list retained. Live scan unavailable: '+String(error);}};void refreshModels();installLocalScan(localSelect.closest('.model-section,.onboard-section') as HTMLElement||localSelect.parentElement!,localSelect);

      const refreshLocalChoice=()=>{
        const local=panel.querySelector<HTMLOptionElement>('#registration-model-provider option[value="local"]');
        const model=panel.querySelector<HTMLSelectElement>('#registration-local-model')?.value;
        const ready=localChoiceUsable(model||'',latestStatus);
        const prepare=panel.querySelector<HTMLButtonElement>('#registration-prepare-local');if(prepare){const existing=/^(ollama|lmstudio|llamacpp):/.test(model||'');prepare.disabled=true;prepare.title='Native local generation is not enabled in this Android engineering build.';prepare.textContent=existing?'Use existing local service':'Download / prepare model';}
        if(local){local.disabled=!ready;local.textContent=ready?'Local model / service':'Local model (prepare or start service)';}
      };
      refreshLocalChoice();
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
      const guide=(url.includes('huggingface.co/settings')||url.includes('huggingface.co/join'))?['Hugging Face token','Create a free Hugging Face account, open Access Tokens, choose a fine-grained token with the access needed for your model, copy it once, and paste it into the Hugging Face token field. Smart Butler encrypts it after you enter a credential password.']:url.includes('groq.com')?['Groq API key','Sign in to Groq Console, open API Keys, create a key, and paste it into the selected provider key field. Review your account rate limits before use.']:url.includes('openrouter.ai')?['OpenRouter API key','Register with OpenRouter, create an API key, and paste it into the selected provider key field. One key works with the currently available free models in the list.']:url.includes('aistudio.google.com')?['Gemini API key','Sign in to Google AI Studio, create an API key for a free-tier project, and paste it into the selected provider key field. Check the current model and limits before use.']:['Local model information','Review the model page and file size. Smart Butler downloads and verifies the selected compact model in its own private app-data folder.'];
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
      let finishWait:(()=>void)|undefined;
      try {
        if(step===6)finishWait=preparationWait(panel);
        if(step===5||step===6){await waitForModelPreparation(panel);await waitForVoicePreparation();}
        if(step===1){if(!draft.name)draft.name='Friend';new Intl.DateTimeFormat('en',{timeZone:draft.timezone});if(draft.birthdate && (Number.isNaN(Date.parse(draft.birthdate)) || draft.birthdate>new Date().toISOString().slice(0,10)))throw Error('Check the birth date.');}
        if(step===2 && draft.contacts.some((x:any)=>!x.name || (x.emergency_priority>0 && !x.phone && !x.email)))throw Error('Each contact needs a name; emergency contacts also need a phone or email.');
        if(step===3 && draft.reminders.some((x:any)=>!x.title||!x.date||x.lead_days<0||x.lead_days>366||!/^(?:[01]\d|2[0-3]):[0-5]\d$/.test(x.time||'09:00')))throw Error('Check reminder titles, dates, local times and lead days (0–366).');
        if(step===6){

          const selected=panel.querySelector<HTMLSelectElement>('#registration-model-provider')!.value;
          const password=pendingVaultPassword;
          if(password.length<8)throw Error('Enter an app password of at least 8 characters.');
          if(!validQuietPeriods(readQuiet(panel)))throw Error('Each enabled quiet period needs different valid From and To times.');
          if(!c.profile_storage)throw Error('Enable profile storage before saving your setup.');
          await invoke('llm_unlock',{password});
          for(const [provider,apiKey] of Object.entries(pendingKeys))await invoke('llm_store_key',{provider,apiKey});
          const hfToken=pendingHfToken.trim();
          const apiKey=pendingProviderKey.trim()||(selected==='huggingface'?hfToken:'');
          if(hfToken || (selected!=='rules' && selected!=='auto' && selected!=='local')){
            if(!password)throw Error('Enter a credential password to encrypt your keys, or choose Skip model setup.');
            await invoke('llm_unlock',{password});
            if(hfToken)await invoke('llm_store_key',{provider:'huggingface',apiKey:hfToken});
          }
          const settings=routingFor({provider:chosenProvider,local_model:chosenLocalModel,online_model:chosenOnlineModel,voice:registrationVoice()});
          await invoke('llm_configure',{settings,apiKey:apiKey||null});
          // Clear secrets only after the profile commit succeeds; failed saves remain retryable.
          // Voice activation is attempted only after the profile is safely committed.
        }
        if(step<6){if(c.profile_storage){savedDraft=true;capture();await draftWrite;}step++;render();return;}
        if(!c.profile_storage)throw Error('Choose profile storage on the first step before saving personal details, or Continue later to use Smart Butler without saving.');
        draft.name=draft.name||'Friend';draft.preferred_voice=registrationVoice();draft.consents.recorded_at=new Date().toISOString();draft.consent=c.profile_storage;draft.onboarding_complete=true;
        const p=await invoke('profile_save',{profile:draft});setProfile(p);await draftWrite.catch(()=>{});await invoke('profile_draft_clear').catch(err=>notify('Profile saved; draft cleanup will need retry: '+String(err)));void saveRegistrationVoice().catch(err=>notify('Profile saved. Voice could not start: '+String(err)+'. You can retry from Voice settings.'));pendingHfToken='';pendingProviderKey='';pendingVaultPassword='';pendingKeys={};stopNarration();panel.classList.add('hidden');await refreshed().catch(err=>notify('Profile saved; some widgets could not refresh: '+String(err)));notify('Your profile is saved. Smart Butler will listen for its wake word only when both microphone and voice-processing consent are enabled.');
      } catch(err){error.textContent=String(err);}finally{finishWait?.();}
    });
    decorateRegistration(panel,step);
    panel.querySelector('[data-stage="1"]')?.addEventListener('click',()=>onlinePlanGuide());
    panel.querySelector('form')!.addEventListener('save-model-stage',async event=>{try{capture();if(c.profile_storage){savedDraft=true;capture();await draftWrite;}(event as CustomEvent).detail.done();}catch(error){panel.querySelector('#registration-error')!.textContent=String(error);(event as CustomEvent).detail.done(false);}});
    panel.querySelector<HTMLInputElement>('input:not([type="hidden"])')?.focus();
  };
  const open=async(force=false)=>{
    let p=getProfile();
    if(!force){try{p=await invoke('profile_get');setProfile(p);}catch(e){notify(`Profile unavailable: ${e}`);}}
    if(!force && p.onboarding_complete)return;
    const legacyDraft=force?null:stored('butler.cleanr5.registrationDraft',null);localStorage.removeItem('butler.cleanr5.registrationDraft');
    const protectedDraft=force?null:await invoke<any>('profile_draft_get').catch(()=>null);
    const resumed=protectedDraft||legacyDraft;draft=structuredClone(resumed?.draft||p);step=resumed?.step||0;savedDraft=!!resumed;
    if(legacyDraft&&!protectedDraft)draftWrite=invoke('profile_draft_save',{draft:legacyDraft}).catch(()=>{});
    const currentSettings:any=resumed?null:await invoke<any>('llm_status').then(x=>x.settings).catch(()=>null);const setup=restoreSetup(resumed?.setup||(currentSettings?{provider:currentSettings.online_enabled?currentSettings.online_provider:'rules',local_model:currentSettings.local_model,online_model:currentSettings.online_model,voice:draft.preferred_voice}:null));chosenProvider=setup.provider;chosenLocalModel=setup.local_model;chosenOnlineModel=setup.online_model;restoreRegistrationVoice(setup.voice||draft.preferred_voice||'');pendingKeys={};pendingHfToken='';pendingProviderKey='';pendingVaultPassword='';
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
    if(!title.startsWith('Delete local'))document.querySelector('#settings-panel')?.classList.add('hidden');
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
      link.href=url;link.download=`butler-personal-data-${new Date().toISOString().slice(0,10)}.json`;
      document.body.append(link);link.click();link.remove();setTimeout(()=>URL.revokeObjectURL(url),30000);
      notify('Personal data export requested. Choose a safe location for the downloaded JSON file.');
    } catch(e){notify(`Could not export data: ${e}`);}
  });
  document.querySelector('#profile-erase')!.addEventListener('click',()=>{
    display('Delete local Smart Butler data',`<p>This removes the saved profile, contacts, reminders, lexicon, prefetched knowledge and conversation sources, saved music files and Smart Butler log on this device. The action cannot be undone. Export first if you want a copy.</p><label>Type DELETE MY BUTLER DATA<input id="erase-phrase" autocomplete="off" spellcheck="false"></label><button id="erase-confirm" disabled>Confirm wipe</button><button id="erase-cancel" type="button">Cancel</button><p id="erase-error" role="alert"></p>`);
    document.querySelector('#erase-cancel')!.addEventListener('click',()=>document.querySelector('#media-modal')!.classList.add('hidden'));
    const input=document.querySelector<HTMLInputElement>('#erase-phrase')!;
    const button=document.querySelector<HTMLButtonElement>('#erase-confirm')!;
    input.addEventListener('input',()=>button.disabled=input.value!=='DELETE MY BUTLER DATA');
    button.addEventListener('click',async()=>{
      button.disabled=true;
      try {
        await invoke('profile_erase',{confirmation:input.value});
        await new Promise<void>((resolve,reject)=>{const request=indexedDB.deleteDatabase('butler-music-library');request.onsuccess=()=>resolve();request.onerror=()=>reject(request.error);request.onblocked=()=>reject(Error('Close other Smart Butler windows and try again to remove saved music.'));});
        for(const key of Object.keys(localStorage))if(key.startsWith('butler.'))localStorage.removeItem(key);
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
      const delivered=stored('butler.reminders.delivered',{});
      const lastChecked=localStorage.getItem('butler.reminders.lastChecked');
      const due=dueReminders(listReminderEvents(p),now,timezone,lastChecked,new Set(Object.keys(delivered)));
      for(const reminder of due){
        const when=reminder.missed?`missed on ${reminder.dueDate}`:reminder.lead_days?`in ${reminder.lead_days} day(s)`:'today';
        notify(`Reminder: ${reminder.title} — ${when}.`);
        delivered[deliveryKey(reminder)]=day;
      }
      for(const [key,when] of Object.entries(delivered)) if(typeof when!=='string'||when<new Date(now.getTime()-32*86400000).toISOString().slice(0,10))delete delivered[key];
      localStorage.setItem('butler.reminders.delivered',JSON.stringify(delivered));
      localStorage.setItem('butler.reminders.lastChecked',day);
    }catch(e){notify(`Reminder check failed: ${e}`);}
  }};
}
