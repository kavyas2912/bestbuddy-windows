import {invoke} from '@tauri-apps/api/core';
import {groupStories,headlineTitle} from './newsroom';
let request=0;
export async function renderWeatherReports(location:string){
 const forecast=document.querySelector<HTMLElement>('#forecast');if(!forecast)return;
 let host=document.querySelector<HTMLElement>('#weather-reports');if(!host){host=document.createElement('section');host.id='weather-reports';host.className='weather-reports';forecast.after(host);}
 const ticket=++request;host.textContent='Checking local weather reports…';
 try{
  const data:any=await invoke('weather_reports',{location});if(ticket!==request)return;host.replaceChildren();
  for(const [title,key]of [['Weather warnings · news reports','warnings'],['Local weather news','items']]){
   const section=document.createElement('section');const heading=document.createElement('h4');heading.textContent=title;section.append(heading);
   const groups=groupStories(data[key]||[]).slice(0,6);
   if(!groups.length){const note=document.createElement('p');note.textContent=key==='warnings'?'No recent warning reports were returned. This does not confirm that no official warnings are active.':'No recent local weather articles were returned.';section.append(note);}
   for(const group of groups){const button=document.createElement('button');button.type='button';button.className='weather-report-story';button.textContent=headlineTitle(group.lead);button.onclick=()=>void invoke('open_external',{url:group.lead.url}).catch(e=>{button.title=String(e);});section.append(button);}
   if(key==='warnings'&&data.official_url){const link=document.createElement('button');link.type='button';link.className='official-weather-link';link.textContent='Official weather warnings ↗';link.onclick=()=>void invoke('open_external',{url:data.official_url});section.append(link);}
   host.append(section);
  }
 }catch(e){if(ticket===request)host.textContent='Weather warning reports and news are unavailable: '+String(e);}
}
