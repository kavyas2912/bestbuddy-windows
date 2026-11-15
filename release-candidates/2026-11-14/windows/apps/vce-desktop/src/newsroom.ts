export type Story={title:string;url:string;source?:string;published?:string;summary?:string;thumbnail?:string;article_status?:string};
export type StoryGroup={lead:Story;coverage:Story[]};
export type StoryFeed={category:string;items:Story[]};
const stop=new Set(['the','a','an','and','of','to','in','on','for','as','at','by','with','from','is','are']);
const labels=new Set(['','news','latest news','breaking news','latest breaking news','news headlines','latest news headlines','world','world news','business','business news','india','india news','national news','regional news','local news','market','markets','market news','stock market news','science','science news','technology','technology news','tech news','sports','sports news','entertainment','entertainment news','home','homepage','top stories','top news','live news','news today','today s news','latest updates','समाचार','ताज़ा खबरें','मुख्य समाचार']);
const sections=new Set(['news','world','india','business','markets','market','sports','science','technology','tech','entertainment','latest','index.html','index.htm','home','national','regional','local','opinion','videos','video','photos','gallery','about','contact','subscribe','login','newsletter','live','europe','asia','africa','us','uk','canada','australia','japan','finance','economy','politics','weather','health','education','lifestyle','culture','rss','feed']);
export const cleanTitle=(value:string)=>value.toLocaleLowerCase().replace(/[^\p{L}\p{N} ]/gu,' ').replace(/\s+/g,' ').trim();
export function headlineTitle(story:Story):string {
 let title=story.title.trim();const source=story.source?.trim();
 if(source)for(const separator of [' - ',' | ',' – ',' — ']){const suffix=separator+source;if(title.toLocaleLowerCase().endsWith(suffix.toLocaleLowerCase())){title=title.slice(0,-suffix.length).trim();break;}}
 return title;
}
/** Preserve identity parameters; remove tracking and equivalent AMP presentation only. */
export function canonicalStoryUrl(value:string):string {
 try{const url=new URL(value);if(!['http:','https:'].includes(url.protocol)||url.username||url.password)return '';
  url.protocol='https:';url.hostname=url.hostname.replace(/^www\./i,'');url.hash='';
  for(const key of [...url.searchParams.keys()])if(/^(?:utm_.+|fbclid|gclid|dclid|msclkid|mc_cid|mc_eid|ocid|cmpid|guccounter)$/i.test(key)||(/^amp$/i.test(key)&&url.searchParams.get(key)==='1')||(/^output$/i.test(key)&&url.searchParams.get(key)==='amp'))url.searchParams.delete(key);
  url.pathname=url.pathname.replace(/\/amp\/?$/i,'').replace(/\/$/,'')||'/';url.searchParams.sort();return url.toString();
 }catch{return '';}
}
export function isArticleHeadline(story:Story):boolean {
 if(typeof story.title!=='string'||typeof story.url!=='string'||['index','unavailable','not-article'].includes(story.article_status||''))return false;
 const title=headlineTitle(story),key=cleanTitle(title);if(labels.has(key)||key===cleanTitle(story.source||''))return false;
 const parts=title.split(/\s[-–—|]\s|[|]/).map(cleanTitle);if(parts.every(part=>labels.has(part)||part===cleanTitle(story.source||'')))return false;
 const canonical=canonicalStoryUrl(story.url);if(!canonical)return false;const url=new URL(canonical),path=url.pathname.replace(/^\/+|\/+$/g,'');
 if(!path)return [...url.searchParams].some(([key,value])=>['p','article','article_id','storyid'].includes(key.toLowerCase())&&!!value);
 // Explicit archive, author, search and taxonomy paths are listings, not stories.
 if(/^(?:news\/)?(?:tags?|topics?|categor(?:y|ies)|sections?|search|authors?|archives?|playlists?)(?:\/|$)/i.test(path))return false;
 const segments=path.split('/').map(part=>{try{return decodeURIComponent(part).toLocaleLowerCase();}catch{return part.toLocaleLowerCase();}});
 return !segments.every(segment=>sections.has(segment)||labels.has(cleanTitle(segment)));
}
function tokens(title:string){return new Set(cleanTitle(title).split(' ').filter(word=>!stop.has(word)));}
export function sameStory(a:Story,b:Story):boolean{
 const first=canonicalStoryUrl(a.url),second=canonicalStoryUrl(b.url);if(first&&first===second)return true;
 if(cleanTitle(headlineTitle(a))===cleanTitle(headlineTitle(b)))return true;
 const x=tokens(headlineTitle(a)),y=tokens(headlineTitle(b));
 const numbers=(set:Set<string>)=>[...set].filter(word=>/^\d+$/.test(word)).sort().join(',');
 if(numbers(x)!==numbers(y))return false;
 for(const word of ['not','no','denies','rejects','wins','loses','rises','falls'])if(x.has(word)!==y.has(word))return false;
 const shared=[...x].filter(word=>y.has(word)).length;
 return shared>=5&&shared/(new Set([...x,...y]).size)>=0.8;
}
export function groupStories(items:Story[]):StoryGroup[]{
 const groups:StoryGroup[]=[];const urls=new Map<string,number>(),titles=new Map<string,number>(),words=new Map<string,Set<number>>();
 for(const item of items){
  if(!isArticleHeadline(item))continue;
  const url=canonicalStoryUrl(item.url),key=cleanTitle(headlineTitle(item)),terms=tokens(headlineTitle(item));let match=urls.get(url)??titles.get(key);
  if(match===undefined){const candidates=new Map<number,number>();for(const word of terms)for(const index of words.get(word)||[])candidates.set(index,(candidates.get(index)||0)+1);match=[...candidates].filter(([,count])=>count>=5).map(([index])=>index).sort((a,b)=>a-b).find(index=>sameStory(groups[index].lead,item));}
  if(match!==undefined){const group=groups[match];if(!group.coverage.some(other=>canonicalStoryUrl(other.url)===url))group.coverage.push(item);urls.set(url,match);titles.set(key,match);}
  else{const index=groups.length;groups.push({lead:item,coverage:[item]});urls.set(url,index);titles.set(key,index);for(const word of terms){if(!words.has(word))words.set(word,new Set());words.get(word)!.add(index);}}
 }
 return groups;
}
/** Deterministic ownership across the supplied snapshot, never first-visited wins. */
export function partitionStories(feeds:StoryFeed[]):Map<string,StoryGroup[]> {
 const priority=['business','business-national','business-world','market-news','corporate-actions','science','ai-tech','sports','entertainment','local','regional','national','world','political','viral','breaking'];
 const order=(category:string)=>{const index=priority.indexOf(category);return index<0?priority.length:index;};
 const sorted=[...feeds].sort((a,b)=>order(a.category)-order(b.category)||a.category.localeCompare(b.category));
 const result=new Map(feeds.map(feed=>[feed.category,[] as StoryGroup[]]));
 const all=groupStories(sorted.flatMap(feed=>feed.items));
 const identities=new Map<string,Set<string>>();for(const feed of sorted)identities.set(feed.category,new Set(feed.items.filter(isArticleHeadline).map(item=>canonicalStoryUrl(item.url))));
 for(const group of all){const owner=sorted.find(feed=>group.coverage.some(item=>identities.get(feed.category)!.has(canonicalStoryUrl(item.url))));if(owner)result.get(owner.category)!.push(group);}
 return result;
}
export function quoteAge(timestamp:number|undefined,now=Date.now()):string{
 if(!timestamp||!Number.isFinite(timestamp))return 'Quote time unavailable';const age=Math.max(0,Math.floor((now-timestamp*1000)/60000));return age<1?'Updated within a minute':age<60?`${age} min old`:age<1440?`${Math.floor(age/60)} h old`:`${Math.floor(age/1440)} d old`;
}
