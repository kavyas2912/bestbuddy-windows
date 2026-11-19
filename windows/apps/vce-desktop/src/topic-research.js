// Shared bounded topic discovery. Retrieved text is reference data, never instructions.
export function publicTopic(value){
 const topic=String(value||'').normalize('NFKC').trim();
 if(topic.length<2||topic.length>64||topic.split(/\s+/).length>7||!/^\p{L}[\p{L}\s+'’&-]+$/u.test(topic))return null;
 if(/\b(my|wife|husband|daughter|son|family|password|address|phone|diagnosis|prescription|account|private)\b/i.test(topic))return null;
 return topic;
}
export function publicUrl(value){
 try{const url=new URL(value);const host=url.hostname.toLowerCase();
  return url.protocol==='https:'&&!url.username&&!url.password&&host.includes('.')&&
   !/^[\d.]+$/.test(host)&&!host.includes(':')&&!/\.(local|localhost|internal|test|invalid)$/.test(host)?url:null;
 }catch{return null}
}
const key=text=>{let hash=2166136261;for(const ch of text.toLowerCase())hash=Math.imul(hash^ch.codePointAt(0),16777619);return (hash>>>0).toString(16)};
const tidy=text=>String(text||'').replace(/\s+/g,' ').trim();
export function searchLinks(xml){
 const document=new DOMParser().parseFromString(xml,'application/xml');
 if(document.querySelector('parsererror'))return [];
 return [...document.querySelectorAll('item')].flatMap(item=>{
  const url=publicUrl(item.querySelector('link')?.textContent||'');
  return url?[{title:tidy(item.querySelector('title')?.textContent),url:url.href}]:[];
 }).slice(0,4);
}
export function extractArticle(html,candidate,topic){
 const url=publicUrl(candidate.url);if(!url)return null;
 const document=new DOMParser().parseFromString(html,'text/html');
 document.querySelectorAll('script,style,nav,header,footer,aside,form,dialog,[hidden],[aria-hidden="true"]').forEach(node=>node.remove());
 const container=document.querySelector('article')||document.querySelector('main')||document.body;
 const title=tidy(document.querySelector('h1')?.textContent||document.querySelector('title')?.textContent||candidate.title).slice(0,180);
 const words=topic.toLowerCase().split(/\s+/).filter(word=>word.length>=2);
 const paragraphs=[...container.querySelectorAll('p')].map(node=>tidy(node.textContent))
  .filter(text=>text.length>=70&&!/^(?:sign in|subscribe|accept cookies|all rights reserved|advertisement)/i.test(text));
 const relevant=paragraphs.filter(text=>words.some(word=>(title+' '+text).toLowerCase().includes(word)));
 const summary=relevant.join(' ').slice(0,900);
 if(!title||summary.length<80)return null;
 const published=document.querySelector('meta[property="article:published_time"]')?.getAttribute('content')||document.querySelector('time[datetime]')?.getAttribute('datetime')||'';
 const date=Number.isFinite(Date.parse(published))?new Date(published).toISOString().slice(0,10):null;
 return {title,summary,url:url.href,source:url.hostname.replace(/^www\./,'')+(date?' · published '+date:' · publication date unknown'),topic};
}
export function createTopicResearch({fetchText,ingest,allowed,getState,saveState,now=()=>Date.now()}){
 let running=false;
 return {async refresh(values,location=''){
  if(running||!allowed())return {fetched:0,failed:0};running=true;
  let fetched=0,failed=0;const state=getState()||{};state.requests=(state.requests||[]).filter(time=>now()-time<3600000);state.topics||={};
  const permitted=()=>allowed()&&state.requests.length<24;
  const request=async url=>{if(!permitted())throw Error('Research paused');state.requests.push(now());saveState(state);return fetchText(url)};
  try{
   const topics=[...new Set(values.map(publicTopic).filter(Boolean))].slice(0,16)
    .sort((a,b)=>(state.topics[key(a)]||0)-(state.topics[key(b)]||0));
   for(const topic of topics.slice(0,2)){
    const id=key(topic);if(now()-(state.topics[id]||0)<3600000||!permitted())continue;
    let rows=[];
    try{
     const query=new URLSearchParams({format:'rss',q:topic+(publicTopic(location)?' '+location:'')});
     let links=[];try{links=searchLinks(await request('https://www.bing.com/search?'+query))}catch{failed++}
     for(const candidate of links.slice(0,2)){
      if(!permitted())break;
      try{const row=extractArticle(await request(candidate.url),candidate,topic);if(row)rows.push(row)}catch{failed++}
     }
     if(!rows.length&&permitted()){
      const query=new URLSearchParams({action:'query',format:'json',generator:'search',gsrsearch:topic,gsrlimit:'2',prop:'extracts|info',exintro:'1',explaintext:'1',inprop:'url',origin:'*'});
      const result=JSON.parse(await request('https://en.wikipedia.org/w/api.php?'+query));
      rows=Object.values(result.query?.pages||{}).flatMap(page=>{
       const url=publicUrl(page.fullurl||''),summary=tidy(page.extract).slice(0,900);
       return url&&summary.length>=80?[{title:tidy(page.title),summary,url:url.href,source:'Wikipedia · CC BY-SA',topic}]:[];
      });
     }
     if(rows.length&&allowed()){const stored=await ingest(rows.slice(0,4));fetched+=stored;state.topics[id]=stored>0?now():now()-3000000}
     else state.topics[id]=now()-3000000; // Ten-minute retry delay; preserve earlier native cache.
    }catch{failed++;state.topics[id]=now()-3000000}
    saveState(state);
   }
   return {fetched,failed};
  }finally{running=false}
 }};
}
