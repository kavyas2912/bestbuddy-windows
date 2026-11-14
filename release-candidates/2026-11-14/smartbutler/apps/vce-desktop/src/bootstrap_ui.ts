import Modal from 'bootstrap/js/dist/modal';

/** Bridge existing widget actions to Bootstrap components without changing their business logic. */
export function installBootstrapUI(){
 const root=document.querySelector<HTMLElement>('#app');if(!root)return;
 const dialogs=new Map<HTMLElement,{instance:Modal;open:boolean;returnTo:HTMLElement|null}>();
 function upgrade(parent:ParentNode){
  parent.querySelectorAll<HTMLButtonElement>('button:not(.btn)').forEach(el=>el.classList.add('btn','btn-sm','btn-secondary'));
  parent.querySelectorAll<HTMLInputElement>('input:not([type=checkbox]):not([type=radio]):not([type=range]):not(.form-control),textarea:not(.form-control)').forEach(el=>el.classList.add('form-control','form-control-sm'));
  parent.querySelectorAll<HTMLSelectElement>('select:not(.form-select)').forEach(el=>el.classList.add('form-select','form-select-sm'));
  parent.querySelectorAll<HTMLInputElement>('input[type=checkbox],input[type=radio]').forEach(el=>el.classList.add('form-check-input'));
  parent.querySelectorAll<HTMLInputElement>('input[type=range]').forEach(el=>el.classList.add('form-range'));
  parent.querySelectorAll<HTMLElement>('.settings-page-tabs,.review-tabs,.model-stage-tabs,.workspace-tabs').forEach(el=>{el.classList.add('nav','nav-pills');el.querySelectorAll('button').forEach(b=>b.classList.add('nav-link'));});
  parent.querySelectorAll<HTMLElement>('.modal').forEach(el=>{
   if(dialogs.has(el)){el.querySelector<HTMLElement>(':scope>.modal-dialog>.modal-card,:scope>.modal-dialog>.registration-splash')?.classList.add('modal-content');return;}
   const content=el.querySelector<HTMLElement>(':scope>.modal-card');if(!content)return;
   const frame=document.createElement('div');frame.className='modal-dialog modal-dialog-centered modal-dialog-scrollable '+(el.id==='settings-panel'?'modal-xl':'modal-lg');
   content.before(frame);frame.append(content);content.classList.add('modal-content');
   for(const attribute of ['aria-label','aria-labelledby']){const value=content.getAttribute(attribute);if(value)el.setAttribute(attribute,value);}
   content.removeAttribute('role');content.removeAttribute('aria-modal');
   el.tabIndex=-1;el.setAttribute('aria-modal','true');el.setAttribute('role','dialog');
   const header=content.querySelector<HTMLElement>(':scope>.dialog-header,:scope>.settings-header');
   if(header){header.classList.add('modal-header');const close=content.querySelector<HTMLButtonElement>('.modal-close');if(close){header.append(close);close.classList.add('bootstrap-dialog-close');}}
   const report=content.querySelector<HTMLElement>(':scope>#market-detail');if(report)report.classList.add('modal-body');
   const state={instance:new Modal(el,{backdrop:true,keyboard:true,focus:true}),open:false,returnTo:null as HTMLElement|null};dialogs.set(el,state);
   el.addEventListener('hide.bs.modal',()=>{
    // Keep existing cancellation/cleanup handlers active for Escape and backdrop closes.
    if(!el.classList.contains('hidden')){const close=el.querySelector<HTMLButtonElement>('.modal-close,#watchlists-close');close?.click();el.classList.add('hidden');}
   });
   el.addEventListener('hidden.bs.modal',()=>{state.open=false;el.classList.add('hidden');state.returnTo?.focus();});
  });
 }
 function sync(){for(const [el,state] of dialogs){const open=!el.classList.contains('hidden');if(open===state.open)continue;state.open=open;if(open){state.returnTo=document.activeElement instanceof HTMLElement?document.activeElement:null;state.instance.show();}else state.instance.hide();}}
 upgrade(root);sync();
 let pending=false;
 new MutationObserver(records=>{
  let added=false,visibility=false;
  for(const record of records){if(record.type==='childList'&&record.addedNodes.length)added=true;if(record.type==='attributes'&&(record.target as Element).classList.contains('modal'))visibility=true;}
  if(!added&&!visibility||pending)return;pending=true;queueMicrotask(()=>{pending=false;if(added)upgrade(root);sync();});
 }).observe(root,{subtree:true,childList:true,attributes:true,attributeFilter:['class']});
}
