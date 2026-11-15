export class SpeechSession {private epoch=0;muted=false;ticket(){return this.epoch;}valid(ticket:number){return ticket===this.epoch&&!this.muted;}cancel(mute=false){this.epoch++;this.muted=mute;}resume(){this.cancel(false);}}
export class PhraseQueue {private positions=new Map<string,number>();pick(key:string,phrases:readonly string[]){if(!phrases.length)return '';const index=this.positions.get(key)||0;this.positions.set(key,index+1);return phrases[index%phrases.length];}}

export const quietCommand=(text:string)=>/^(?:(?:hey[, ]+)?butler[, ]+)?(?:quiet|be quiet|shut up|no need(?: to (?:read|speak|talk))?|stop(?: the)?(?: briefing| speaking| reading| talking)?)[.!? ]*$/i.test(text.trim());
