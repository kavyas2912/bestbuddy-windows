import {invoke} from '@tauri-apps/api/core';
import {setupGuide} from './setup_patch';
export function localServiceModel(id:string){return /^(ollama|lmstudio|llamacpp):[^\r\n]+$/.test(id);}
const prepared=new Set<string>();
export function localChoiceUsable(id:string,status:any){if(!id)return false;if(localServiceModel(id)||prepared.has(id))return true;return Boolean(status?.local_runtime_ready&&(id.startsWith('file:')||(status.local_models||[]).some((m:any)=>m.id===id)));}
export function localPreparationLabel(id:string){return localServiceModel(id)?'Uses installed local service':id.startsWith('file:')?'Prepare local runtime':'Download / prepare selected model';}
const work=new WeakMap<HTMLSelectElement,Promise<void>>();
export async function waitForModelPreparation(root:ParentNode){for(const select of root.querySelectorAll<HTMLSelectElement>('#registration-local-model,#llm-local-model'))await work.get(select);}
export function installLocalScan(host:HTMLElement,select:HTMLSelectElement){
 if(select.dataset.modelPicker)return;select.dataset.modelPicker='true';
 const chooser=document.createElement('details');chooser.className='local-model-chooser';const summary=document.createElement('summary');summary.setAttribute('aria-label','Local model dropdown');const groups=document.createElement('div');groups.className='local-model-groups';chooser.append(summary,groups);select.after(chooser);select.classList.add('model-backing-select');select.tabIndex=-1;
 const tools=document.createElement('div');tools.className='local-scan-tools';const scan=document.createElement('button');scan.type='button';scan.textContent='Scan for on-device local model';const status=document.createElement('small');status.setAttribute('role','status');tools.append(scan,status);host.append(tools);
 let runtime=true,scanSupported=true,busy=false,metadata:any[]=[];
 const paint=()=>{
  summary.textContent=(select.selectedOptions[0]?.textContent||'Choose a local model')+' ▾';groups.replaceChildren();
  const none=document.createElement('button');none.type='button';none.textContent='No additional model · built-in offline';none.onclick=()=>choose('');groups.append(none);
  for(const available of [true,false]){const box=document.createElement('section');box.className='model-choice-box';const heading=document.createElement('h4');heading.textContent=available?'Available models on device':'Download models';const list=document.createElement('ul');
   for(const option of [...select.options].filter(o=>o.value&&((/^(ollama|lmstudio|llamacpp|file):/.test(o.value)||/\bInstalled\b|Available in/i.test(o.textContent||''))===available))){const li=document.createElement('li');const button=document.createElement('button');button.type='button';button.textContent=option.textContent;button.disabled=option.disabled;button.onclick=()=>choose(option.value);li.append(button);list.append(li);}
   if(!list.children.length){const li=document.createElement('li');li.textContent=available?'No installed models found. Use Scan to choose a folder.':'No compatible downloads in this catalogue.';list.append(li);}box.append(heading,list);groups.append(box);
  }
 };
 const prepare=async(id:string)=>{
  if(localServiceModel(id)){status.textContent='Selected installed service. A stopped service must be started before conversation.';return;}
  if(!runtime)throw Error('The local generative runtime is not available in this Android build. Built-in offline conversation remains available.');
  const task=(async()=>{busy=true;status.textContent='Preparing model and runtime automatically…';try{await invoke('llm_prepare_local',{model:id});prepared.add(id);status.textContent='Model passed its preparation test. Save your choices to use it.';select.dispatchEvent(new Event('change',{bubbles:true}));}finally{busy=false;}})();
  work.set(select,task);try{await task;}finally{work.delete(select);}
 };
 const choose=(id:string)=>{
  if(busy){status.textContent='Please wait for the current model preparation to finish.';return;}
  const previous=select.value;chooser.open=false;
  const activate=async()=>{select.value=id;select.dispatchEvent(new Event('change',{bubbles:true}));paint();if(id)await prepare(id);};
  if(!id||localServiceModel(id)||id.startsWith('file:')||/\bInstalled\b/i.test(select.options[[...select.options].findIndex(o=>o.value===id)]?.textContent||'')){void activate().catch(e=>{status.textContent=String(e);select.value=previous;select.dispatchEvent(new Event('change',{bubbles:true}));});return;}
  const m=metadata.find(m=>m.id===id);const url=m?.repo?'https://huggingface.co/'+m.repo:'https://huggingface.co/models?pipeline_tag=text-generation';
  let token:HTMLInputElement,password:HTMLInputElement;
  const dialog=setupGuide('How to download your model','Review the model size and licence. Public models usually need no account or token. For gated models, register, accept the model terms, and create a read or fine-grained token. A token is optional below; enter your app password only if saving a token. Next downloads verified weights, prepares the runtime and tests the model automatically. Conversation activates when you save your choices.',[{name:'Model page',url},{name:'Register',url:'https://huggingface.co/join'},{name:'Token guide',url:'https://huggingface.co/docs/hub/security-tokens'}],'Next · download',async()=>{try{if(token.value.trim()){if(password.value.length<8)throw Error('Use your app password of at least 8 characters to encrypt this token.');await invoke('llm_unlock',{password:password.value});await invoke('llm_store_key',{provider:'huggingface',apiKey:token.value.trim()});const field=document.querySelector<HTMLInputElement>(select.id==='registration-local-model'?'#registration-vault-password':'#llm-vault-password');if(field){field.value=password.value;field.dispatchEvent(new Event('input',{bubbles:true}));}token.value='';password.value='';}await activate();}catch(e){select.value=previous;select.dispatchEvent(new Event('change',{bubbles:true}));paint();throw e;}});
  const fields=document.createElement('div');fields.className='download-access-fields';for(const [title,key]of [['Optional Hugging Face token','token'],['App password · only for token storage','password']]){const label=document.createElement('label');label.textContent=title;const input=document.createElement('input');input.type='password';input.autocomplete='off';label.append(input);fields.append(label);if(key==='token')token=input;else password=input;}dialog.querySelector('.setup-help-actions')!.before(fields);
 };
 const refresh=async(scanFolder?:string)=>{
  const data:any=await invoke('llm_local_catalog',{localOnly:true,scanFolder:scanFolder||null});runtime=data.runtime_available!==false;scanSupported=data.local_scan_supported!==false;metadata=data.models||[];
  const selected=select.value;for(const option of [...select.options])if(/^(ollama|lmstudio|llamacpp|file):/.test(option.value))option.remove();
  for(const m of data.local_available||[])select.add(new Option(`${m.name} · Available in ${m.source}${m.size_bytes?' · '+(m.size_bytes/1e9).toFixed(2)+' GB':''}`,m.id));
  if(selected&&![...select.options].some(o=>o.value===selected))select.add(new Option(selected+' · saved choice',selected));select.value=selected;
  scan.disabled=!scanSupported;scan.title=scanSupported?'Choose a model library folder in Explorer':'Desktop folder scanning is unavailable on Android';status.textContent=scanSupported?`${data.local_available?.length||0} installed models found · ${(data.max_model_bytes||0)/1e9} GB limit.`:'Desktop model-library imports are unavailable in this Android build.';paint();
 };
 scan.onclick=async()=>{scan.disabled=true;try{const folder=await invoke<string|null>('llm_pick_model_folder');if(folder)await refresh(folder);}catch(e){status.textContent=String(e);}finally{scan.disabled=!scanSupported;}};
 select.addEventListener('change',paint);new MutationObserver(paint).observe(select,{childList:true,subtree:true});paint();void refresh().catch(e=>status.textContent=String(e));
}
