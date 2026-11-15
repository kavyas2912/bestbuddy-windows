/** Original controls retain native routing, including index components and corporate actions. */
export function installNewsNavigation(){
 const tabs=document.querySelector<HTMLElement>('#news-tabs');if(!tabs)return;
 const labels:Record<string,string>={breaking:'Breaking',national:'National',world:'World',local:'Local',business:'Economy',science:'SciTech',sports:'Sports & culture'};
 for(const b of tabs.querySelectorAll<HTMLButtonElement>('[data-cat]')){const label=labels[b.dataset.cat!];if(label)b.textContent=label;else {b.hidden=true;b.style.setProperty('display','none','important');}}
 const market=document.querySelector('.market-tabs');const names:Record<string,string>={indices:'Indices','market-news':'Market News','business-news':'Business News'};
 for(const b of market?.querySelectorAll<HTMLButtonElement>('[data-market-view]')||[]){const name=names[b.dataset.marketView!];if(name)b.textContent=name;else{b.hidden=true;b.style.setProperty('display','none','important');}}
}
