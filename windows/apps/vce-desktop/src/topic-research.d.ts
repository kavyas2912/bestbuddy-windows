export type ResearchRow={title:string;summary:string;url:string;source:string;topic:string};
export function createTopicResearch(options:{fetchText:(url:string)=>Promise<string>;ingest:(rows:ResearchRow[])=>Promise<number>;allowed:()=>boolean;getState:()=>any;saveState:(state:any)=>void;now?:()=>number}):{refresh:(values:string[],location?:string)=>Promise<{fetched:number;failed:number}>};
export function publicTopic(value:unknown):string|null;
export function publicUrl(value:unknown):URL|null;
export function searchLinks(xml:string):Array<{title:string;url:string}>;
export function extractArticle(html:string,candidate:{title:string;url:string},topic:string):ResearchRow|null;
