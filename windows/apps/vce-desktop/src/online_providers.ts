import {invoke} from '@tauri-apps/api/core';
import {onlineProviders} from './online_registry';
const esc=(s:string)=>s.replace(/[&<>"']/g,c=>({'&':'&amp;','<':'&lt;','>':'&gt;','"':'&quot;',"'":'&#39;'}[c]!));
export function installOnlineProviders(){
 const shell=document.querySelector('#settings-panel');if(!shell)return;
 const activeProvider=shell.querySelector<HTMLSelectElement>('#llm-online-provider')!;
 const activeModel=shell.querySelector<HTMLInputElement>('#llm-online-model')!;
 for(const provider of onlineProviders){
  if(![...activeProvider.options].some(o=>o.value===provider.id))activeProvider.add(new Option(provider.name,provider.id));
  let rows=[...shell.querySelectorAll<HTMLElement>(`[data-provider-row="${provider.id}"]`)];
  if(!rows.length){const row=document.createElement('div');row.className='online-model-row';row.dataset.providerRow=provider.id;row.innerHTML=`<input type="radio" name="online-active" aria-label="Use ${esc(provider.name)}"><strong>${esc(provider.name)}</strong><select aria-label="${esc(provider.name)} model"></select><input class="row-api-key" type="password" autocomplete="off" aria-label="${esc(provider.name)} API key" placeholder="API key"><div><button type="button" class="text-link llm-row-key">Save key</button></div>`;shell.querySelector('[data-model-part="online"],.online-models')?.append(row);rows=[row];}
  for(const row of rows){
   if(row.dataset.fixedModel)continue;
   const select=row.querySelector<HTMLSelectElement>('select');if(select&&provider.id!=='openrouter'){select.replaceChildren(...provider.models.map(id=>new Option(id,id)));}
   const label=document.createElement('small');label.className='provider-plan';label.textContent=provider.note+' · checked 23 Oct 2026';row.append(label);
   const controls=document.createElement('div');controls.className='provider-links';const refresh=document.createElement('button');refresh.type='button';refresh.textContent='↻';refresh.setAttribute('aria-label','Refresh '+provider.name+' models');controls.append(refresh);
   for(const [text,url] of [['Account ↗',provider.url],['Plan / limits ↗',provider.guide]]){const button=document.createElement('button');button.type='button';button.textContent=text;button.onclick=()=>void invoke('open_external',{url}).catch(()=>{});controls.append(button);}row.append(controls);
   refresh.onclick=async()=>{if(!select)return;refresh.disabled=true;const selected=select.value;try{const items:any[]=await invoke('llm_provider_models',{provider:provider.id});select.replaceChildren(...items.map(m=>new Option(m.name+' · '+m.id,m.id)));if([...select.options].some(o=>o.value===selected))select.value=selected;label.textContent=provider.note+' · catalog checked '+new Date().toLocaleString();}catch(error){label.textContent=provider.note+' · '+String(error);}finally{refresh.disabled=false;}};
   row.addEventListener('change',()=>{if(!row.querySelector<HTMLInputElement>('input[type=radio]')?.checked)return;activeProvider.value=provider.id;if(select)activeModel.value=select.value;});
  }
 }
 // Quota failover already lives in the native bridge; this states its actual scope.
 const hint=document.createElement('small');hint.className='provider-fallback-note';hint.textContent='Quota/service errors switch to another configured free/evaluation provider. Funded HF and Cerebras accounts are used only when explicitly selected. No credit top-ups. Provider plan and spending limits still apply.';shell.querySelector('[data-model-part="online"],.online-models')?.append(hint);
}
