import {sameStory,isReadableStory,type Story} from './newsroom.js';
export type CategoryNews={category:string;items:Story[]};
export const categoryOrder=['world','national','local','breaking','regional','political','business-world','business-national','business','science','ai-tech','sports','entertainment','viral'];
export const categoryLimit=(category:string)=>['world','national','local'].includes(category)?5:2;
export function headlineText(story:Story):string{return (story.source&&story.title.endsWith(' - '+story.source)?story.title.slice(0,-story.source.length-3):story.title).replace(/\s+/g,' ').trim();}
export function briefingSections(categories:CategoryNews[]){const used:Story[]=[];return categoryOrder.map(category=>{const items:Story[]=[];const candidates=categories.filter(row=>row.category===category).flatMap(row=>row.items).filter(item=>item.title&&item.url&&isReadableStory(item)&&!/^(?:home|latest news|breaking news|news headlines|world news|business news)(?:\s*[-|:]\s*(?:latest news|home|news|headlines))?$/i.test(headlineText(item))).sort((a,b)=>(Date.parse(b.published||'')||0)-(Date.parse(a.published||'')||0));for(const item of candidates){if(used.some(other=>sameStory(other,item)))continue;items.push(item);used.push(item);if(items.length>=categoryLimit(category))break;}return {category,items};}).filter(row=>row.items.length);}
export const briefingSelection=(categories:CategoryNews[])=>briefingSections(categories).flatMap(row=>row.items);
export function inTimeWindow(now:string,start:string,end:string):boolean {if(!/^\d{2}:\d{2}$/.test(start)||!/^\d{2}:\d{2}$/.test(end)||start===end)return false;return start<end?now>=start&&now<end:now>=start||now<end;}
export const listeningGaps=[5500,11000,22000,45000,90000];
