/** Rehome existing elements so their native playback, persistence and provider handlers survive. */
export function installMediaLayout(){
 const music=document.querySelector<HTMLElement>('.music-panel');
 const disclosure=(label:string,nodes:(Element|null)[],className:string)=>{
  const details=document.createElement('details');details.className='media-shelf '+className;
  const summary=document.createElement('summary');summary.textContent=label;details.append(summary);
  for(const node of nodes)if(node)details.append(node);return details;
 };
 if(music){
  const audio=music.querySelector<HTMLAudioElement>('#audio')!;
  // Custom controls cover seek, time and volume; avoid a second native transport strip.
  audio.removeAttribute('controls');
  const controls=music.querySelector<HTMLElement>('.music-controls')!;
  const library=music.querySelector<HTMLElement>('.library-controls')!;
  const previous=music.querySelector<HTMLElement>('#music-previous')!;
  const open=music.querySelector<HTMLElement>('#music-open')!;
  controls.prepend(previous);library.prepend(open);open.setAttribute('aria-label','Open audio files');open.title='Open audio files';
  const timeline=document.createElement('div');timeline.className='player-timeline';
  timeline.append(music.querySelector('#music-seek')!.closest('label')!,music.querySelector('#music-time')!);
  const playback=disclosure('Playback options',[
   music.querySelector('#music-shuffle'),music.querySelector('#music-repeat'),
   music.querySelector('#music-volume')!.closest('label'),music.querySelector('#music-mute')
  ],'playback-options');
  const shelves=disclosure('Library & queue',[library],'library-shelf');
  const services=disclosure('Find music online',[music.querySelector('.media-discovery')],'services-shelf');
  music.append(music.querySelector('.music-hero')!,audio,controls,timeline,playback,shelves,services);
  music.querySelector<HTMLElement>('.progress')!.hidden=true; // Existing handler remains valid; the labelled seek control replaces this duplicate strip.
  for(const id of ['music-play','music-next','music-previous']){
   const button=music.querySelector<HTMLButtonElement>('#'+id)!;
   button.setAttribute('aria-label',id==='music-play'?'Play or pause music':id==='music-next'?'Next track':'Previous track');
  }
  const canvas=document.createElement('canvas');canvas.className='audio-spectrum';canvas.width=640;canvas.height=160;
  canvas.setAttribute('role','img');canvas.setAttribute('aria-label','Audio frequency visualization; active during local playback');
  music.querySelector('.music-hero')!.after(canvas);
  installSpectrum(audio,canvas,music);
 }
 const video=document.querySelector<HTMLElement>('.video-panel');
 if(video){
  const stage=video.querySelector<HTMLElement>('#video-stage')!;
  const actions=video.querySelector<HTMLElement>('.video-actions')!;
  actions.append(video.querySelector('#video-stop')!);
  const discover=disclosure('Search & video sources',[
   video.querySelector('.youtube-search'),video.querySelector('.video-discovery'),video.querySelector('#video-tabs')
  ],'video-sources');
  video.append(stage,actions,discover);
 }
}

/** No animation is used as evidence of audio: these bars come from the playing local track. */
function installSpectrum(audio:HTMLAudioElement,canvas:HTMLCanvasElement,panel:HTMLElement){
 let context:AudioContext|undefined,analyser:AnalyserNode|undefined,frame=0,last=0;
 const ink=canvas.getContext('2d');if(!ink)return;
 const reduced=matchMedia('(prefers-reduced-motion: reduce)');
 const bins=new Uint8Array(128);
 const clear=()=>{cancelAnimationFrame(frame);frame=0;ink.clearRect(0,0,canvas.width,canvas.height);};
 const visible=()=>!document.hidden&&panel.getBoundingClientRect().width>0&&panel.getBoundingClientRect().height>0;
 const draw=(time:number)=>{
  frame=0;if(audio.paused||audio.ended||!visible()||reduced.matches||!analyser){clear();return;}
  if(time-last>=50){last=time;analyser.getByteFrequencyData(bins);ink.clearRect(0,0,canvas.width,canvas.height);
   ink.fillStyle=getComputedStyle(panel).getPropertyValue('--text-primary').trim()||getComputedStyle(panel).color;
   for(let i=0;i<32;i++){const height=bins[i*4]/255*canvas.height;ink.fillRect(i*20+3,canvas.height-height,12,height);}
  }
  frame=requestAnimationFrame(draw);
 };
 const resume=()=>{
  if(audio.paused||audio.ended||!visible()||reduced.matches)return;
  try{
   if(!context){
    // Capture a separate stream for analysis; never reroute or mute the actual player output.
    const stream=(audio as HTMLAudioElement & {captureStream?:()=>MediaStream}).captureStream?.();
    if(!stream?.getAudioTracks().length){canvas.hidden=true;return;}
    context=new AudioContext();analyser=context.createAnalyser();analyser.fftSize=256;
    context.createMediaStreamSource(stream).connect(analyser);
   }
   void context.resume().then(()=>{if(!frame)frame=requestAnimationFrame(draw);}).catch(()=>clear());
  }catch{canvas.hidden=true;} // Native playback remains usable when visualization is unsupported.
 };
 audio.addEventListener('play',resume);audio.addEventListener('pause',clear);audio.addEventListener('ended',clear);
 document.addEventListener('visibilitychange',()=>document.hidden?clear():resume());
 reduced.addEventListener('change',()=>reduced.matches?clear():resume());
 new MutationObserver(()=>visible()?resume():clear()).observe(panel,{attributes:true,attributeFilter:['class']});
 window.addEventListener('beforeunload',()=>{clear();void context?.close();},{once:true});
}
