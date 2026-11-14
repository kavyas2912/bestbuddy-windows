import {onlineProviders} from './online_registry.js';
export type SetupChoices={provider:string;local_model:string;online_model:string;voice:string};
export function restoreSetup(value:any):SetupChoices{return {provider:typeof value?.provider==='string'?value.provider:'rules',local_model:typeof value?.local_model==='string'?value.local_model:'',online_model:typeof value?.online_model==='string'?value.online_model:'openrouter/free',voice:typeof value?.voice==='string'?value.voice:''};}
export function routingFor(setup:SetupChoices){const selected=onlineProviders.find(p=>p.id===setup.provider);const online=Boolean(selected);return {provider:online||setup.local_model?'auto':'rules',local_model:setup.local_model,online_enabled:online,online_provider:selected?.id||'groq',online_model:selected?(selected.id==='openrouter'?setup.online_model:selected.model):'openai/gpt-oss-20b'};}

export function stageSnapshot(draft:any,step:number,setup:SetupChoices){return {draft:structuredClone(draft),step,setup:restoreSetup(setup)};}
