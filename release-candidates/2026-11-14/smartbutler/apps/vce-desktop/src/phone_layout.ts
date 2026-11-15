import {quietSpeech} from './speech';

/** Phone shell reuses the existing controls and handlers, including saved settings. */
export function installPhoneLayout(brand:string){
 const root=document.querySelector<HTMLElement>('#app'),shell=root?.querySelector<HTMLElement>('.app-shell');if(!root||!shell)return;
 const viewport=matchMedia('(max-width:700px), (pointer:coarse)');let active=false,current='chatter';
 const moves:Array<{element:HTMLElement;marker:Comment}>=[];
 const move=(element:HTMLElement|null,destination:HTMLElement)=>{if(!element)return;const marker=document.createComment('phone-layout-original-position');element.before(marker);moves.push({element,marker});destination.append(element);};
 const createDialog=(id:string,title:string)=>{const modal=document.createElement('div');modal.id=id;modal.className='modal hidden phone-options-modal';const card=document.createElement('section');card.className='modal-card';const header=document.createElement('header');header.className='dialog-header';const heading=document.createElement('h2');heading.textContent=title;const close=document.createElement('button');close.type='button';close.className='modal-close';close.textContent='×';close.setAttribute('aria-label','Close '+title);close.onclick=()=>modal.classList.add('hidden');header.append(heading,close);const body=document.createElement('div');body.className='phone-options-body';card.append(header,body);modal.append(card);root.append(modal);return {modal,body};};
 const options=createDialog('phone-widget-options','Options'),more=createDialog('phone-sections','All sections');
 const bar=document.createElement('header');bar.className='phone-appbar';
 const logo=document.createElement('img');logo.alt=brand;logo.src=brand==='Smart Butler'?'/assets/butler-welcome.png':'/assets/butler-welcome.png';
 const title=document.createElement('h1');title.textContent='Chat';
 const action=document.createElement('button');action.type='button';action.className='phone-context-action';
 const quiet=document.createElement('button');quiet.type='button';quiet.textContent='Quiet';quiet.onclick=quietSpeech;
 const settings=document.createElement('button');settings.type='button';settings.className='phone-settings-action';settings.textContent='⚙';settings.setAttribute('aria-label','Settings');settings.onclick=()=>document.querySelector<HTMLButtonElement>('#settings')?.click();
 bar.append(logo,title,action,quiet,settings);shell.prepend(bar);
 const nav=document.createElement('nav');nav.className='phone-primary-nav';nav.setAttribute('aria-label','App sections');shell.append(nav);
 const panels:Record<string,string>={chatter:'Chat',news:'News',market:'Markets',calendar:'Daybook',weather:'Weather',music:'Music',video:'Video'};
 const choose=(name:string)=>{document.querySelector<HTMLButtonElement>(`.desktop-widget-nav [data-desktop-panel="${name}"]`)?.click();sync(name);};
 const addButton=(host:HTMLElement,label:string,handler:()=>void)=>{const button=document.createElement('button');button.type='button';button.textContent=label;button.onclick=handler;host.append(button);return button;};
 const afterClose=(callback:()=>void)=>{if(more.modal.classList.contains('show'))more.modal.addEventListener('hidden.bs.modal',callback,{once:true});else queueMicrotask(callback);more.modal.classList.add('hidden');};
 for(const [name,label] of Object.entries(panels)){
  if(!document.querySelector(`.dashboard>.${name}-panel`))continue;
  addButton(more.body,label,()=>afterClose(()=>choose(name)));
 }
 addButton(more.body,'Settings',()=>afterClose(()=>settings.click()));
 addButton(more.body,'Voice setup and permissions',()=>afterClose(()=>{const card=document.querySelector<HTMLElement>('.voice-setup-card');if(card){card.hidden=false;card.classList.remove('minimised');}else settings.click();}));
 const navButtons=new Map<string,HTMLButtonElement>();
 for(const [name,label] of [['chatter','Chat'],['news','News'],['market','Markets'],['calendar','Daybook']])navButtons.set(name,addButton(nav,label,()=>choose(name)));
 addButton(nav,'Settings',()=>settings.click());
 for(const [name,label] of Object.entries(panels)){if(['chatter','news','market','calendar'].includes(name)||!root.querySelector('.'+name+'-panel'))continue;navButtons.set(name,addButton(nav,label,()=>choose(name)));}
 const groups=new Map<string,HTMLElement>();
 const listbar=document.createElement('div');listbar.className='phone-watchlist-bar';
 const dateToggle=document.createElement('button');dateToggle.type='button';dateToggle.className='phone-month-toggle';dateToggle.textContent='Month ▾';dateToggle.onclick=()=>document.querySelector<HTMLButtonElement>('#calendar-toggle')?.click();
 const ideas=createDialog('phone-chat-ideas','Conversation ideas');
 function sync(name?:string){
  if(name)current=name;else{const selected=root!.querySelector<HTMLElement>('.dashboard>.panel.desktop-active');const match=Object.keys(panels).find(key=>selected?.classList.contains(key+'-panel'));if(match)current=match;}
  title.textContent=panels[current]||'Chat';
  for(const [key,button] of navButtons){button.classList.toggle('active',key===current);if(key===current)button.setAttribute('aria-current','page');else button.removeAttribute('aria-current');}
  action.textContent=current==='chatter'?'Briefing':current==='news'?'Sources':current==='calendar'?'Add':'Options';
  action.onclick=()=>{
   if(current==='chatter'){root!.querySelector<HTMLButtonElement>('#daily-briefing-chat,#briefing-now')?.click();return;}
   if(current==='calendar'){root!.querySelector<HTMLButtonElement>('#daybook-add')?.click();return;}
   for(const [key,group] of groups)group.hidden=key!==current;
   options.modal.querySelector('h2')!.textContent=(current==='news'?'News sources':panels[current]+' options');options.modal.classList.remove('hidden');
  };
 }
 function activate(){if(active)return;active=true;document.documentElement.classList.add('phone-mode');
  for(const key of Object.keys(panels)){
   const panel=root!.querySelector<HTMLElement>(`.dashboard>.${key}-panel`);if(!panel)continue;
   const group=document.createElement('section');group.className='phone-option-group';group.hidden=true;options.body.append(group);groups.set(key,group);
   if(key!=='chatter')move(panel.querySelector<HTMLElement>('.panel-tools'),group);
   if(key==='market'){
    const controls=panel.querySelector<HTMLElement>('.market-control-bar');const shortcuts=controls?.querySelector<HTMLElement>('.watchlist-shortcuts');
    panel.querySelector('.market-tabs')?.after(listbar);move(shortcuts||null,listbar);move(controls,group);
   }
  }
  // Header Talk reuses the original handler through the baseline adapter.
  
  const shelf=root!.querySelector<HTMLElement>('.persona-shelf');if(shelf){move(shelf,ideas.body);const trigger=addButton(root!.querySelector<HTMLElement>('.chatter-panel')!,'Ideas',()=>ideas.modal.classList.remove('hidden'));trigger.className='phone-ideas-trigger';root!.querySelector('.chat-compose')?.before(trigger);}
  const calendar=root!.querySelector<HTMLElement>('.calendar-panel');calendar?.querySelector('.daybook-layout,.daybook-summary')?.before(dateToggle);
  const month=root!.querySelector<HTMLButtonElement>('#calendar-toggle');if(month?.getAttribute('aria-expanded')!=='true')month?.click();
  sync();
 }
 function deactivate(){if(!active)return;active=false;document.documentElement.classList.remove('phone-mode');for(const {element,marker} of moves.reverse()){marker.replaceWith(element);}moves.length=0;for(const group of groups.values())group.remove();groups.clear();listbar.remove();dateToggle.remove();root!.querySelector('.phone-ideas-trigger')?.remove();for(const dialog of [options,more,ideas])dialog.modal.classList.add('hidden');}
 new MutationObserver(()=>{if(active)sync();}).observe(root.querySelector('.dashboard')!,{subtree:false,childList:false,attributes:true});
 for(const panel of root.querySelectorAll('.dashboard>.panel'))new MutationObserver(()=>{if(active)sync();}).observe(panel,{attributes:true,attributeFilter:['class']});
 const apply=()=>viewport.matches?activate():deactivate();viewport.addEventListener('change',apply);apply();
}
