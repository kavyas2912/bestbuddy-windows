import {renderWeatherReports} from './weather_reports';
import {localChoiceUsable,localServiceModel,localPreparationLabel} from './local_models_ui';
import {onlineProviders} from './online_registry';
import {installBaselineUI} from './baseline_ui';
import {installOnlineProviders} from './online_providers';
import {installPhoneLayout} from './phone_layout';
import {installNewsNavigation} from './news_navigation';
import 'bootstrap/dist/css/bootstrap.min.css';
import { installBootstrapUI } from './bootstrap_ui';
import { initializeTheme, bindThemePicker } from "./theme";
import "./theme.css";
import {quietCommand} from './speech_policy';
import {speechSession,quietSpeech,resumeSpeech,installSpeechControls} from './speech';
import {setupRefinedSettings} from './settings_layout';
import {quietAt} from './settings_policy';
import {setupMobileVoice} from './mobile_voice';
import bundledExchanges from '../../../plugins/market/market_registry.json';
import {reviewWatchlistCsv,normalizeWatchEntry,mergeWatchlist,type WatchEntry,type CsvReview} from './watchlists';
import {dailyIndicators,pricePoints,tradingViewSymbol} from './stockdetail';
import {briefingSelection,headlineText,inTimeWindow,listeningGaps} from './briefing';
import {articleExcerpt,isReadableStory,headlineTitle,groupStories,partitionStories,isArticleHeadline,cleanTitle,quoteAge, type Story} from './newsroom';
import { setupCompanion } from './companion';
import { setupDisplay } from './display';
import { currentDayAndTime, listReminderEvents } from './reminders';
import { invoke as nativeInvoke } from '@tauri-apps/api/core';
import { newsScope, quietDuration } from './discovery';
let quietUntil = Number(localStorage.getItem('butler.cleanr5.quietUntil')) || 0;
const isQuiet = () => Date.now() < quietUntil;
function invoke<T=unknown>(command:string,args?:Record<string,unknown>):Promise<T>{
  if(!('__TAURI_INTERNALS__' in window))return Promise.reject(new Error('Open the installed Smart Butler app to use this feature.'));
  if(command==='voice_speak' && (isQuiet()||speechSession.muted)) return Promise.resolve(undefined as T);
  const result=nativeInvoke<T>(command,args);
  if(command==='voice_speak')void result.then(()=>{if(!briefingInProgress&&!isQuiet())startListening();}).catch(()=>{});
  return result;
}
function handleQuiet(text:string):boolean {
 if(/^(?:hey[,]?\s+)?(?:butler[,! ]+)?(?:quiet|be quiet|shut up|silence|no need(?: to (?:read|speak|talk))?|stop (?:talking|speaking|reading)|that(?:’|')?s enough)[.! ]*$/i.test(text.trim())){stopNewsReading();briefingCancelled=true;cancelListening();quietUntil=8_640_000_000_000_000;localStorage.setItem('butler.cleanr5.quietUntil',String(quietUntil));void nativeInvoke('voice_hush').catch(()=>{});void nativeInvoke('voice_quiet_until',{until:quietUntil}).catch(()=>{});voiceTurn++;pushMessage('Certainly. Quiet until you say “resume speaking” or press Speak.','butler');setVoiceTop('Quiet');return true;}
 if(/^(?:butler[, ]+)?(?:resume speaking|you can talk now|end quiet mode)[.!]?$/i.test(text.trim())){
  resumeSpeech();quietUntil=0;void nativeInvoke('voice_quiet_until',{until:0}).catch(()=>{});localStorage.removeItem('butler.cleanr5.quietUntil');pushMessage('Certainly. I am ready to help again.','butler');return true;
 }
 const duration=quietDuration(text);if(duration===null)return false;briefingCancelled=true;
 cancelListening();quietUntil=Date.now()+duration;void nativeInvoke('voice_quiet_until',{until:quietUntil});localStorage.setItem('butler.cleanr5.quietUntil',String(quietUntil));voiceTurn++;
 pushMessage(`Quiet until ${new Date(quietUntil).toLocaleTimeString()}. Say “resume speaking” to end quiet mode.`,'butler');
 setButlerState('sleep');setVoiceTop('Quiet mode');return true;
}
type CuratedSource={id:string;name:string;status?:string;items_returned?:number};
type SourceCatalog={scope:string;sources:CuratedSource[];city?:string;region?:string;country?:string};
let curatedNews:Record<string,SourceCatalog>={};let curatedMarket:SourceCatalog|undefined;let catalogKey='';let catalogExpires=0;let curatedExchanges:any[]=bundledExchanges;
function curatedOptions(catalog:SourceCatalog|undefined,automatic='all'){return `<option value="${automatic}">Auto · your area</option>`+(catalog?.sources||[]).map(source=>`<option value="${esc(source.id)}" ${source.status==='unavailable'?'disabled':''}>${esc(source.name)}${source.status==='unavailable'?' · unavailable':source.status==='empty'?' · no recent matches':''}</option>`).join('');}
function sourceOptions(category:string){return curatedOptions(curatedNews[category]);}
function marketSourceOptions(){return curatedOptions(curatedMarket,'market');}
function currentSource(category:string){const source=localStorage.getItem(`butler.cleanr5.curatedNewsSource.${profile.country}.${profile.region}.${profile.location}.${category}`)||'all';return curatedNews[category]?.sources.some(row=>row.id===source&&row.status!=='unavailable')?source:'all';}
async function loadSourceCatalog(exchange?:string){
 const key=JSON.stringify([profile.location,profile.region,profile.country,exchange||document.querySelector<HTMLSelectElement>('#market-exchange')?.value||'all']);
 if(catalogKey===key&&Date.now()<catalogExpires)return;
 const result:any=await invoke('source_catalog',{exchange:exchange||document.querySelector<HTMLSelectElement>('#market-exchange')?.value||'all'});
 curatedNews=result.news||{};curatedMarket=result.market;catalogKey=key;catalogExpires=Date.now()+300000;curatedExchanges=result.exchanges||[];syncWatchlistChoice();
 const selector=document.querySelector<HTMLSelectElement>('#market-exchange');if(selector){const selected=selector.value;selector.innerHTML='<option value="all">Home market · automatic</option>'+(result.exchanges||[]).map((market:any)=>`<option value="${esc(market.id)}">${esc(market.country)} · ${esc(market.name)}</option>`).join('');if(Array.from(selector.options).some(option=>option.value===selected))selector.value=selected;}
}
let newsSpeechGeneration=0;let newsReading=false;
function stopNewsReading(){newsSpeechGeneration++;if(newsReading)void nativeInvoke('voice_hush').catch(()=>{});newsReading=false;}
async function readCategoryHeadlines(items:NewsItem[]){
 stopNewsReading();if(isQuiet()||routineQuiet()||!document.querySelector('#widget-news')?.classList.contains('desktop-active'))return;
 const generation=newsSpeechGeneration;await refreshStatus();if(!lastVoice?.tts_ready||generation!==newsSpeechGeneration||isQuiet()||!document.querySelector('#widget-news')?.classList.contains('desktop-active'))return;
 newsReading=true;
 try{for(const group of groupStories(items).slice(0,10)){if(generation!==newsSpeechGeneration||isQuiet())break;await invoke('voice_speak',{text:headlineText(group.lead)+'.',request:'Read selected news category',capability:'news.read_visible'});}}
 finally{if(generation===newsSpeechGeneration){newsReading=false;if(!isQuiet())startListening();}}
}

import './style.css';
import './mobile.css';
import './compact-ui.css';
import './phone-ui.css';

type Profile = { [key: string]: any; name: string; location: string; country: string; region: string; interests: string[]; preferences: string[]; consent: boolean };
type NewsItem = { section?:string; title: string; url: string; summary?: string; published?: string; source?: string; thumbnail?: string; video_url?: string | null };
type MarketItem = { kind?:string; market_status?:string; stale?:boolean; close?: string | number; quote?: any; symbol: string; name?: string; region?: string; price?: number; change?: number; change_percent?: number; currency?: string; source?: string; timestamp?: number; error?: string };
type VoiceDiag = { state: string; always_on: boolean; wake_mode: string; wake_word: string; microphone_ready: boolean; microphone?: string; speaker_ready: boolean; speaker?: string; asr_ready: boolean; tts_ready: boolean; vad_ready: boolean; kws_ready: boolean; denoise_ready: boolean; punctuation_ready: boolean; last_transcript?: string; last_voice_ms?: number; barge_in_count?: number; wake_count?: number; kws_decode_count?: number; kws_last_decode_ms?: number; dropped_audio_buffers?: number; input_level?: number; raw_input_level?: number; input_rate?: number; input_channels?: number; input_frames?: number; last_turn_peak?: number; last_turn_frames?: number; last_turn_hypotheses?: number; vad_active?: boolean; asr_partial?: string; rejected_fragments?: number; last_turn_outcome?: string; pause_to_finish_ms?: number; max_listening_ms?: number; last_error?: string };
type AppStatus = { build?: string; profile: Profile; voice: VoiceDiag; learning: any; log_path: string };
type VoiceEvent = { kind: string; text?: string; keyword?: string; asr_ms?: number; at_ms: number };

initializeTheme();

const app = document.querySelector<HTMLDivElement>('#app')!;
let profile: Profile = { name: 'Friend', location: '', country: '', region: '', interests: [], preferences: [], consent: false };
let activeNews = 'national';
let newsItems: NewsItem[] = [];
let newsRequest=0;
let marketRequest=0;
const recentChatter:string[]=[];
let localNewsItems: NewsItem[] = [];
let regionalNewsItems: NewsItem[] = [];
let nationalNewsItems: NewsItem[] = [];
let localEventItems: NewsItem[] = [];
let latestWeather: any = null;
let coverageRequest = 0;
let marketItems: MarketItem[] = [];
let selectedMarketIndex:MarketItem|null=null;
let selectedMarketCompany:{name:string,symbol:string}|null=null;
let weatherTimezone: string | undefined;
let cityList:string[] = [];
let selectedWeatherCity = '';
let marketNewsItems:NewsItem[]=[];
let homeCoordinates:{latitude:number;longitude:number}|null=null;
let idleTimer = 0;
let responding = false;
let pollingVoice = false;
let voiceTurn = 0;
const refreshing = new Set<string>();
let companion: ReturnType<typeof setupCompanion>;
let lastVoice: VoiceDiag | null = null;
let lastInteractionAt = Date.now();
let lastChatterAt = Date.now();
let launchBriefingStarted = false;let startupBriefingWaitingSince=0;
let chatterTopic = 0;
let launchWarmupCompleted = false;
let launchVoiceReady = false;
let launchContextPromise:Promise<unknown>|null = null;
let proactiveChatter = localStorage.getItem('butler.cleanr5.proactiveChatter') !== 'off';
let calendarCursor = new Date();
let selectedCalendarDate = new Date();
let briefingOfferPending = false;
let briefingCancelled=false;

const esc = (s: unknown) => String(s ?? '').replace(/[&<>"]/g, m => ({ '&': '&amp;', '<': '&lt;', '>': '&gt;', '"': '&quot;' }[m]!));
const icon = (name:string)=>{
 const paths:Record<string,string>={
 spark:'<path d="M5 4h14v12H9l-4 4V4Z"/><path d="M9 8h6M9 12h4"/>',
 news:'<rect x="3" y="4" width="18" height="16" rx="2"/><path d="M7 8h4v4H7zM14 8h3M14 12h3M7 16h10"/>',
 market:'<path d="M3 3v18h18M6 16l5-5 4 2 6-8M16 5h5v5"/>',
 weather:'<circle cx="9" cy="8" r="3"/><path d="M9 1v1M2 8h1M4 3l1 1M14 3l-1 1M8 19h11a3 3 0 0 0 0-6 5 5 0 0 0-9-1 4 4 0 0 0-2 7Z"/>',
 calendar:'<rect x="3" y="5" width="18" height="16" rx="2"/><path d="M7 3v4M17 3v4M3 10h18M7 14h3M14 14h3M7 18h3"/>',
 mic:'<rect x="9" y="2" width="6" height="13" rx="3"/><path d="M5 10v2a7 7 0 0 0 14 0v-2M12 19v3M8 22h8"/>',
 settings:'<path d="m9 3 1-2h4l1 2 3 2 2 0 2 4-2 2v3l2 2-2 4h-3l-2 2h-4l-1-2-3-1H3l-2-4 2-2v-3L1 8l2-4h3Z"/><circle cx="12" cy="12" r="3"/>',
 refresh:'<path d="M20 7a9 9 0 1 0 1 9M20 2v6h-6"/>',
 plus:'<path d="M12 4v16M4 12h16"/>',chevron:'<path d="m5 3 15 9-15 9 3-9-3-9ZM8 12h12"/>',
 link:'<path d="M14 3h7v7M21 3 10 14M10 3H3v18h18v-7"/>',
 globe:'<circle cx="12" cy="12" r="10"/><ellipse cx="12" cy="12" rx="4" ry="10"/><path d="M2 12h20"/>',
 music:'<path d="M3 9h4l5-5v16l-5-5H3V9ZM16 8a6 6 0 0 1 0 8M19 5a10 10 0 0 1 0 14"/>'};
 return `<svg class="pictorial-icon" viewBox="0 0 24 24" aria-hidden="true" fill="none" stroke="currentColor" stroke-width="1.8" stroke-linecap="round" stroke-linejoin="round">${paths[name]||paths.spark}</svg>`;
};

function render() {
  app.innerHTML = `
  <section class="launch-splash" id="launch-splash" aria-label="Smart Butler is starting" aria-live="polite">
    <div class="launch-sky"><span></span><span></span><span></span></div>
    <div class="launch-butler-mark-wrap"><img src="/assets/butler-welcome.png" class="launch-butler-mark" alt="Orangutan butler welcoming you"/></div>
    <div class="launch-copy"><div class="kicker">WELCOME TO</div><h1>Smart Butler</h1><p id="launch-message">Hello! I’m warming up my voice and gathering your day.</p><div class="launch-progress" aria-hidden="true"><i></i><i></i><i></i></div></div>
  </section>
  <div class="app-shell">
    <header class="topbar">
      <div class="brand">
        <div class="butler-mark-wrap" id="butler-mascot"><img src="/assets/butler-welcome.png" class="butler-mark" alt="Orangutan butler welcoming you"/></div>
        <div class="brand-copy"><div class="brand-name"><img class="butler-wordmark" src="/assets/smart-butler-wordmark.svg" alt="Smart Butler"/></div><div class="brand-sub">Considered. Current. At your service.</div></div>
      </div>
      <div class="header-greeting"><h1 id="greeting">Hello, ${esc(profile.name)}.</h1><p id="hero-line">I’m here with you.</p></div>
      <div class="header-actions">
        <div class="header-pill" id="engine-pill"><span class="status-dot"></span><span>STATUS</span></div>
        <div class="header-pill" id="mic-pill">${icon('mic')}<span>Microphone</span></div>
        <div class="header-pill" id="speaker-pill">◖<span>SPEAKER</span></div>
        <div class="voice-mini"><span class="small-state" id="voice-top-state">Voice off</span><button id="talk" class="talk-button">${icon('mic')}<b>Speak</b></button></div>
        <button class="round-icon" id="settings" title="Settings">${icon('settings')}</button>
      </div>
    </header>

    <section class="desktop-context-bar" aria-label="Location, weather and voice summary">
      <div class="context-location"><span>${icon('globe')}</span><b id="header-location">${esc(profile.location)}</b><small id="header-region">${esc([profile.region,profile.country].filter(Boolean).join(', '))}</small></div>
      <div class="context-weather"><span id="header-weather-icon">☀</span><b id="header-weather-temp">—°</b><div class="header-weather-copy"><small id="header-weather-summary">Checking weather…</small><span id="header-weather-forecast">Today’s forecast is loading…</span></div></div>
      <div class="context-voice"><span class="status-dot"></span><b>Voice</b><small id="header-voice-state">Preparing always-on listening…</small></div>
    </section>

    <main class="workspace">
      <nav class="desktop-widget-nav" role="tablist" aria-label="Smart Butler workspaces">
        <div class="widget-nav-heading"><span>Dashboard</span><small>Choose a view</small></div>
        <button class="active" data-desktop-panel="chatter" id="tab-chatter" role="tab" aria-controls="widget-chatter" aria-selected="true" tabindex="0" aria-current="page">${icon('spark')}<span><b>Chat</b><small>Your assistant</small></span></button>
        <button data-desktop-panel="news" id="tab-news" role="tab" aria-controls="widget-news" aria-selected="false" tabindex="-1">${icon('news')}<span><b>News</b><small>Headlines and sources</small></span></button>
        <button data-desktop-panel="market" id="tab-market" role="tab" aria-controls="widget-market" aria-selected="false" tabindex="-1">${icon('market')}<span><b>Market Watch</b><small>Indices and companies</small></span></button>
        <button data-desktop-panel="weather" id="tab-weather" role="tab" aria-controls="widget-weather" aria-selected="false" tabindex="-1">${icon('weather')}<span><b>Weather</b><small>Current and forecast</small></span></button>
        <button data-desktop-panel="calendar" id="tab-calendar" role="tab" aria-controls="widget-calendar" aria-selected="false" tabindex="-1">${icon('calendar')}<span><b>Daybook</b><small>Calendar and Panchang</small></span></button>
      </nav>
      <div class="butler-entrance" id="butler-entrance" aria-hidden="true"><img id="entrance-pose" src="/assets/butler-welcome.png" alt=""/><span id="entrance-caption">At your service.</span></div><section class="dashboard">
        <article id="widget-weather" role="tabpanel" aria-labelledby="tab-weather" class="panel weather-panel"><div class="workspace-intro"><div><span class="eyebrow">THE OUTLOOK</span><h1>A fine day to be prepared.</h1><p>Conditions, the coming days and your local time.</p></div><img src="/assets/butler-resting.png" alt=""/></div>
          <div class="panel-head"><div class="panel-title"><span class="panel-icon weather">${icon('weather')}</span><div><h2>Weather</h2><small id="weather-location">${esc(profile.location)}</small></div></div><div class="panel-tools"><span class="live-label" id="weather-live">NOT FETCHED</span><button class="refresh" id="weather-refresh" title="Refresh weather" aria-label="Refresh weather">${icon('refresh')}</button></div></div>
          <div class="weather-city-bar"><div id="weather-city-buttons" class="weather-city-buttons"></div><button id="add-city" class="weather-city-add" type="button">${icon('plus')} Add city</button><small>Home stays tied to your profile location · up to five extra cities</small></div>
          <div class="weather-overview"><div class="current-weather"><div class="weather-graphic" id="weather-graphic">☀</div><div class="weather-measure"><div class="current-temp" id="weather-temp">—°</div><div class="current-condition" id="weather-condition">Fetching current weather…</div><div class="weather-facts" id="weather-facts"></div></div></div><div class="weather-top"><div class="analog-clock" id="analog"><i class="hour"></i><i class="minute"></i><i class="second"></i></div><div class="weather-timecopy"><div class="weather-clockline"><span class="big-time" id="weather-time">—</span><span class="mini-day" id="weather-day">—</span></div><div class="big-date" id="weather-date">—</div></div></div></div>
          <div class="forecast-row" id="forecast"></div>
        </article>

        <article id="widget-calendar" role="tabpanel" aria-labelledby="tab-calendar" class="panel calendar-panel">
          <div class="panel-head"><div class="panel-title"><span class="panel-icon calendar-icon">${icon('calendar')}</span><div><h2>Calendar &amp; daybook</h2><small>Schedule, Hindu calendar context and this day in history</small></div></div><div class="panel-tools"><button class="action-btn" id="daybook-add" type="button">${icon('plus')} Add schedule item</button><button class="panel-toggle" id="calendar-toggle" title="Show or hide month" aria-expanded="true">▴</button></div></div>
          <div class="daybook-layout"><aside class="daybook-left" aria-label="Calendar and your schedule"><div class="calendar-month" id="calendar-month"><div class="calendar-nav"><button id="calendar-prev" type="button" aria-label="Previous month">‹</button><strong id="calendar-month-title"></strong><button id="calendar-next" type="button" aria-label="Next month">›</button></div><div class="calendar-weekdays"><span>Sun</span><span>Mon</span><span>Tue</span><span>Wed</span><span>Thu</span><span>Fri</span><span>Sat</span></div><div class="calendar-grid" id="calendar-grid"></div></div><div class="daybook-summary" id="daybook-summary"></div></aside><section class="daybook-events" aria-label="Local, regional and national events"><div class="day-context" id="day-context"><p>Choose a date for local events, regional and national context, science history and notable events.</p></div></section></div>
        </article>

        <article id="widget-news" role="tabpanel" aria-labelledby="tab-news" class="panel news-panel"><div class="workspace-intro news-heading"><div><span class="eyebrow">THE READING ROOM</span><small id="news-scope">Your area</small></div><div class="panel-tools"><label class="source-choice"><select id="news-source" aria-label="News source for this category">${sourceOptions(activeNews)}</select></label><span class="live-label" id="news-live">READY</span><button class="refresh" id="news-refresh" title="Refresh news">${icon('refresh')}</button></div></div>
          <div class="tabs" id="news-tabs"><button class="active" data-cat="breaking">BREAKING</button><button data-cat="world">WORLD</button><button data-cat="national">NATIONAL</button><button data-cat="regional">REGION</button><button data-cat="local">LOCAL</button><button data-cat="political">POLITICAL</button><button data-cat="business">BUSINESS</button><button data-cat="business-world">WORLD BUSINESS</button><button data-cat="business-national">NATIONAL BUSINESS</button><button data-cat="science">SCIENCE</button><button data-cat="ai-tech">AI &amp; TECH</button><button data-cat="sports">SPORTS</button><button data-cat="entertainment">ENTERTAINMENT</button><button data-cat="viral">VIRAL NEWS</button></div>
          <div class="news-ticker" id="news-ticker" aria-label="Current category news ticker"></div>
          <div class="news-list" id="news-list"><p class="empty-state">Headlines appear here when live information is enabled. Choose a category or refresh to check its sources.</p></div>
        </article>

        <article id="widget-chatter" role="tabpanel" aria-labelledby="tab-chatter" class="panel chatter-panel mobile-active desktop-active">
          <div class="panel-head"><div class="panel-title"><span class="butler-badge"><img src="/assets/butler-welcome.png" alt=""/></span><div><h2>Smart Butler</h2><small>A little wit. A lot of help.</small></div></div><div class="panel-tools"><button class="action-btn ghost" id="briefing-now" type="button">Daily briefing</button><span class="live-label alive">READY</span></div></div>
          <div class="news-ticker" id="chat-ticker" aria-label="Latest news updates"></div><div class="briefing-controls"><span id="briefing-progress" role="status">Ask for a briefing, a useful thought, or a little mischief.</span><button id="briefing-stop" class="action-btn ghost" type="button" data-quiet-speech>Quiet</button></div>
          <div class="chat-scroll" id="chat-scroll"><div class="chat-messages" id="chat-messages"></div></div>
          <label class="chatter-switch"><input id="proactive-chatter" type="checkbox" ${proactiveChatter?'checked':''}/> Smart Butler may start timely companion check-ins</label>
          <div class="chat-compose"><input id="chat-input" autocomplete="off" placeholder="Talk to Smart Butler…"/><button id="send" aria-label="Send message" title="Send message">${icon('chevron')}</button></div>
        </article>



        <article id="widget-market" role="tabpanel" aria-labelledby="tab-market" class="panel market-panel"><div class="workspace-intro"><div><span class="eyebrow">THE MARKET DESK</span><h1>Keep an eye on the world.</h1><p>Your exchanges, latest available quotes and the stories behind them.</p></div><img src="/assets/butler-market.png" alt=""/></div>
          <div class="panel-head"><div class="panel-title"><span class="panel-icon market">${icon('market')}</span><div><h2>Market Watch</h2><small>Major indices · refreshable board</small></div></div><div class="panel-tools"><span class="live-label" id="market-live">NOT FETCHED</span><button class="refresh" id="market-refresh" title="Refresh markets">${icon('refresh')}</button></div></div>
          <div class="market-tabs" role="tablist"><button class="active" data-market-view="indices">Indices</button><button data-market-view="components">Components</button><button data-market-view="market-news">Market news</button><button data-market-view="business-world">World business</button><button data-market-view="business-national">National business</button><button data-market-view="corporate-actions">Corporate actions</button><button data-market-view="company-news">Selected company</button><button data-market-view="business-news">Business News</button></div>
          <div class="market-control-bar"><div class="watchlist-shortcuts"><span id="watchlist-quick" class="watchlist-quick"></span><button class="action-btn ghost" id="watchlists-open" type="button" aria-label="Edit watchlists">Edit</button></div><label>Exchange<select id="market-exchange"><option value="all">Home market · automatic</option>${bundledExchanges.map(market=>`<option value="${esc(market.id)}">${esc(market.country)} · ${esc(market.name)}</option>`).join('')}</select></label><label class="source-choice">Market news source<select id="market-news-source">${marketSourceOptions()}</select></label><label class="ticker-choice-label">Ticker<select id="ticker-choice"><option value="home">Home-market basket</option>${savedWatchlists().map(list=>`<option value="${esc(list.id)}">${esc(list.name)}</option>`).join('')}</select></label><label class="ticker-choice-label">Refresh<select id="price-refresh"><option value="5">5 minutes</option><option value="6" selected>6 minutes</option><option value="7">7 minutes</option></select></label></div><div class="ticker-tape" id="ticker-tape"><div class="price-ticker-heading"><b>MARKET WATCH</b><span>Yahoo Finance · choose an exchange to view latest available quotes</span></div></div>
          <aside id="watchlist-popover" class="watchlist-popover hidden" role="dialog" aria-label="Watchlist prices"><button id="watchlist-popover-close" class="action-btn ghost" type="button">Close</button><h3 id="watchlist-popover-title"></h3><div id="watchlist-popover-rows"></div></aside><div class="market-grid" id="market-grid"><p class="empty-state">Select an exchange and refresh to load its major indices. Quote timestamps and source labels appear with each price.</p></div>
          <div class="market-browser hidden" id="market-browser"></div>
          <div class="market-news-ticker" id="market-news-ticker" aria-label="Business and market news ticker"></div>
        </article>


      </section>
    </main>



    <footer class="statusbar"><span id="footer-status">Smart Butler · Android engineering candidate 1.0.11 · NEWS-20261027</span><span id="footer-log">Orangutan edition · personal data in Settings</span></footer>
  </div>

  <div id="watchlist-modal" class="modal hidden"><section class="modal-card watchlist-editor" role="dialog" aria-modal="true" aria-label="Your watchlists"><button id="watchlists-close" type="button" class="action-btn ghost">Close</button><h3>Your watchlists</h3><p>Keep stocks or indices in three named lists. Quotes are delayed where the provider requires it.</p><div id="watchlist-tabs"></div><label>List name<input id="watchlist-name" maxlength="80"></label><div id="watchlist-entries"></div><div class="watchlist-add"><label>Exchange<select id="watchlist-exchange"></select></label><label>Symbol<input id="watchlist-symbol" maxlength="32" placeholder="RELIANCE, AAPL or ^NSEI"></label><label>Display name<input id="watchlist-label" maxlength="120" placeholder="Optional"></label><button id="watchlist-add" type="button" class="action-btn">Add symbol</button></div><section class="watchlist-import"><h4>Import symbols from CSV</h4><p>Use Symbol, Name and Exchange columns, or one symbol per line. Exchange is optional; the exchange selected above is used when omitted. Files stay on this device.</p><label>Choose CSV<input id="watchlist-csv" type="file" accept=".csv,text/csv,text/plain"></label><label>Import mode<select id="watchlist-import-mode"><option value="append">Add to this list</option><option value="replace">Replace this list in draft</option></select></label><div id="watchlist-import-review" aria-live="polite"></div><button id="watchlist-import-apply" type="button" class="action-btn" disabled>Import into draft</button></section><button id="watchlist-save" type="button" class="action-btn">Save watchlists</button><p id="watchlist-status" role="status"></p><p>For India, an unsuffixed stock uses NSE (.NS); use .BO for BSE. Full provider symbols are accepted.</p></section></div>
      <div class="modal hidden" id="media-modal"><div class="modal-card"><button id="media-close" class="modal-close">×</button><div id="media-content"></div></div></div>
  <div class="modal hidden" id="market-modal"><div class="modal-card market-detail-card" role="dialog" aria-modal="true" aria-labelledby="market-dialog-title"><header class="dialog-header"><h2 id="market-dialog-title">Market detail</h2><button id="market-close" class="modal-close" aria-label="Close market detail">×</button></header><div id="market-detail"></div></div></div>
  <div class="modal hidden" id="schedule-modal"><form class="modal-card schedule-card" id="schedule-form"><button id="schedule-close" class="modal-close" type="button" aria-label="Close schedule editor">×</button><h2>Add schedule item</h2><p>Add a one-time or annual reminder to your encrypted local profile.</p><label>Title<input id="schedule-title" required maxlength="120"></label><div class="schedule-fields"><label>Date<input id="schedule-date" type="date" required></label><label>Time<input id="schedule-time" type="time" value="09:00" required></label></div><label class="check"><input id="schedule-annual" type="checkbox"> Repeat annually</label><label>Reminder lead time<select id="schedule-lead"><option value="0">On the day</option><option value="1">1 day before</option><option value="7">7 days before</option><option value="30">30 days before</option></select></label><div id="schedule-error" role="alert"></div><button class="primary-wide" type="submit">Save schedule item</button></form></div>
  <div class="modal hidden" id="settings-panel"><div class="modal-card settings-card" role="dialog" aria-modal="true" aria-labelledby="settings-title"><button id="settings-close" class="modal-close" aria-label="Close settings">×</button><div class="settings-header"><div><h2 id="settings-title">Smart Butler settings</h2><p>Choose a section to adjust Smart Butler. Privacy and deletion have their own page.</p></div></div><div id="settings-action-result" class="settings-action-result" role="status" aria-live="polite"></div>
    <div class="settings-shell"><nav class="settings-nav" aria-label="Settings sections">
      <button class="setting-tile active" data-settings-view="profile" type="button">Profile & people</button>
      <button class="setting-tile" data-settings-view="privacy" type="button">Privacy &amp; security</button><button class="setting-tile" data-settings-view="status" type="button">Device checks</button>
      <button class="setting-tile" data-settings-view="voice" type="button">Voice & speech</button>
      <button class="setting-tile" data-settings-view="assistant" type="button">AI replies</button>
      <button class="setting-tile" data-settings-view="learning" type="button">Offline knowledge</button>
      <button class="setting-tile" id="open-logs" data-settings-view="logs" type="button">Logs</button>
    </nav><div class="settings-content">
      <section class="settings-view" data-settings-page="profile"><h3>Profile & people</h3><p>Review the details Smart Butler uses to address you and remember important people and dates.</p><label class="theme-picker">Appearance<select id="app-theme"><option value="signature">Signature navy & teal</option><option value="darkly">Bootstrap dark</option><option value="flatly">Bootstrap light</option></select></label><div id="settings-profile-summary" class="settings-summary"></div><div class="settings-page-actions settings-nav-actions"><button id="edit-profile" class="action-btn" type="button">Edit profile and consents</button><button id="profile-ai-keys" class="action-btn ghost" type="button">AI keys and model choices</button></div><div id="profile-settings-detail" class="settings-detail" aria-live="polite"></div></section>
      <section class="settings-view hidden" data-settings-page="privacy"><h3>Privacy &amp; security</h3><p>Protect your saved keys, export a copy, or remove your personal data from this device.</p>        <div class="model-section vault-section"><div class="model-section-title"><div><span class="section-number">1</span><div><h4>User data and key vault</h4><p>Profile, contacts, reminders and learning files are encrypted on this device. Provider keys use a separate password-protected AES-256 vault.</p></div></div></div><div class="model-inline"><label class="model-field"><span>Vault password</span><input id="llm-vault-password" type="password" autocomplete="new-password" placeholder="At least 8 characters"></label><button id="llm-unlock" class="action-btn" type="button">Create / unlock vault</button></div><small>The password is never saved and cannot be recovered. Unlock once after each app launch.</small></div><div class="danger-zone"><h4>Delete all personal data</h4><p>This is permanent. Export a copy before deleting. You will be asked to type a confirmation.</p><div id="privacy-actions"></div></div></section>
      <section class="settings-view hidden" data-settings-page="status"><h3>Status and diagnostics</h3><div class="device-report" id="device-report"></div></section>
      <section class="settings-view hidden" data-settings-page="voice"><h3>Voice & speech</h3><div class="voice-tuning"><h4>Quiet time</h4><p>Say “keep quiet for fifteen minutes” or choose a duration. Say “resume speaking” to end it early.</p><label>Minutes<input id="quiet-minutes" type="number" min="1" max="1440" value="15"></label><button id="quiet-start" class="action-btn">Start quiet time</button><button id="quiet-end" class="action-btn ghost">Resume speaking</button></div><div class="settings-page-actions"><button id="speaker-test" class="action-btn ghost" type="button">${icon('music')} Test speaker</button></div><div class="voice-tuning"><h3>Your daily rhythm</h3><p>When may I chat, and when should I leave you in peace, sir?</p><label><input id="routine-quiet-enabled" type="checkbox"> Daily quiet hours</label><label>Quiet from<input id="routine-quiet-start" type="time" value="13:00"></label><label>Until<input id="routine-quiet-end" type="time" value="15:00"></label><label>Relaxation from<input id="routine-relax-start" type="time" value="18:00"></label><label>Until<input id="routine-relax-end" type="time" value="21:00"></label><small>Optional chatter stays in these hours. With personalization enabled, only hourly interaction counts are learned on this device. Wake-word alert remains available between brief reply windows.</small><button id="routine-save" class="action-btn">Save daily rhythm</button><p id="routine-status" role="status"></p><h3>Conversation timing</h3><label>Pause before Smart Butler responds <output id="tune-pause-value">0.6 s</output><input id="tune-pause" type="range" min="400" max="1800" step="100" value="600"></label><label>Maximum listening time <output id="tune-limit-value">18 s</output><input id="tune-limit" type="range" min="6000" max="30000" step="1000" value="18000"></label><small>A longer pause helps when you speak slowly; a shorter pause makes replies start sooner.</small></div>
        <div class="voice-tuning"><h4>Names &amp; pronunciation</h4><p>Teach Smart Butler how to say a name or word. This stays in your personal lexicon on this device.</p><label>Word or name<input id="pronunciation-term" maxlength="64" placeholder="Kamal"/></label><label>Say it like<input id="pronunciation-spoken" maxlength="120" placeholder="Ka maal"/></label><small>Leave “Say it like” empty to remove a saved pronunciation. Personalization must be enabled in Profile.</small><button id="pronunciation-save" class="action-btn" type="button">Save pronunciation</button><p id="pronunciation-status" role="status"></p></div><div class="device-report"><b>Smart Butler's speaking voices</b><label class="voice-auto-label"><input id="piper-auto" type="checkbox"> Adapt pacing to the conversation</label><small>Smart Butler keeps one consistent voice, with brisker jokes and briefings and gentler pacing for stories and support. A warm bass emphasis is applied to speech. The installed voice determines how deep it sounds.</small><label>Ready to use<select id="piper-voice"></select></label><button id="piper-select" class="action-btn ghost" type="button">Keep this voice</button><div id="piper-voice-report">Checking installed voices…</div><p>Optional voices are downloaded only when you choose one, verified before installation, and kept in Smart Butler's writable data folder.</p><div id="piper-voice-catalog" class="voice-catalog"></div></div>
        <div class="voice-test device-report"><b>Voice signal test</b><small>Click Record microphone, then say “Read the news” clearly. Smart Butler records five seconds through the device microphone path. Listen locally, or feed the WAV through ASR → Smart Butler → speaker.</small>
          <div class="voice-test-actions"><button id="mic-record" class="action-btn" type="button">Record microphone</button><button id="mic-save" class="action-btn ghost" type="button" disabled>Save WAV</button><button id="mic-replay" class="action-btn ghost" type="button" disabled>Feed recording to Smart Butler</button></div>
          <audio id="mic-playback" controls hidden></audio>
          <label class="voice-file-label">Or choose a WAV file <input id="voice-wav-file" type="file" accept=".wav,audio/wav,audio/x-wav"/></label><button id="voice-wav-run" class="action-btn ghost" type="button">Feed selected WAV to Smart Butler</button>
          <div id="voice-test-result" role="status">No voice signal test run yet.</div>
        </div></section>
      <section class="settings-view hidden conversation-settings" data-settings-page="assistant"><div class="settings-view-heading"><div><h3>Conversation models</h3><p>Choose local or online AI for longer replies. News, weather and app commands use their own sources.</p></div><div id="llm-report" class="model-status">Checking setup…</div></div>

        <div class="model-section hf-section"><div class="model-section-title"><div><span class="section-number">2</span><div><h4>Hugging Face access</h4><p>Used for optional model downloads and Hugging Face online inference.</p></div></div></div><div class="provider-key-line"><label class="model-field"><span>Hugging Face token</span><input id="llm-hf-token" type="password" autocomplete="off" placeholder="hf_…"></label><button class="action-btn llm-key-save" data-key-provider="huggingface" data-key-input="llm-hf-token" type="button">Save token</button><button class="action-btn ghost llm-direct-guide" data-url="https://huggingface.co/settings/tokens" type="button">Get token ↗</button><button class="action-btn ghost llm-direct-guide" data-url="https://huggingface.co/docs/hub/security-tokens" type="button">Guide ↗</button></div></div>
        <div class="model-section"><div class="model-section-title"><div><span class="section-number">3</span><div><h4>Compact local model</h4><p>Smart Butler downloads verified model files into its own private app-data folder. It can also use running local services or compatible GGUF files found in standard model caches.</p></div></div></div><div class="model-inline local-model-line"><label class="model-field"><span>Local model</span><select id="llm-local-model"><option value="">No additional model · built-in offline</option><option value="qwen3-4b-q4">Qwen3-4B-Q4_K_M · 2.50 GB</option><option value="qwen3-0.6b">Qwen3 0.6B Q8 · 0.64 GB</option><option value="phi3.5-mini-q4">Phi-3.5-mini-instruct-Q4_K_M · 2.39 GB</option><option value="smollm2-1.7b-q4">SmolLM2-1.7B-Instruct-Q4_K_M · 1.06 GB</option><option value="smollm2-1.7b-q8">SmolLM2-1.7B-Instruct-Q8_0 · 1.82 GB</option></select></label><button id="llm-prepare-local" class="action-btn" type="button">Prepare model</button></div><div class="compact-links"><button class="text-link llm-guide" data-url="https://huggingface.co/Qwen/Qwen3-4B-GGUF">Qwen3 4B details ↗</button><button class="text-link llm-guide" data-url="https://huggingface.co/Qwen/Qwen3-8B-GGUF">Qwen3 8B details ↗</button></div></div>
        <div class="model-section"><div class="model-section-title"><div><span class="section-number">4</span><div><h4>Response composer</h4><p>Automatic uses built-in sources and your selected local model. Online fallback is optional.</p></div></div></div><div class="composer-box"><label class="model-field"><span>Composer mode</span><select id="llm-provider"><option value="auto">Automatic · sources → local</option><option value="rules">Built-in sources only</option><option value="local" disabled>Local GGUF only · prepare first</option><option value="online">Online provider only</option></select></label><label class="check"><input type="checkbox" id="llm-online-enabled">Allow configured online fallback</label><label class="model-field"><span>Active online provider</span><select id="llm-online-provider"><option value="openrouter">OpenRouter</option><option value="groq">Groq</option><option value="gemini">Google Gemini</option><option value="huggingface">Hugging Face</option></select></label><label class="model-field"><span>Active model</span><input id="llm-online-model" list="online-model-suggestions" autocomplete="off" placeholder="Provider model ID"><datalist id="online-model-suggestions"><option value="openrouter/free"><option value="openai/gpt-oss-20b"><option value="openai/gpt-oss-120b"><option value="qwen/qwen3.8-27b"><option value="gemini-3.8-flash"></datalist></label><button id="llm-save" class="action-btn" type="button">Save choices</button></div></div>
        <div class="model-section online-models"><div class="model-section-title"><div><span class="section-number">5</span><div><h4>Online model choices</h4><p>Select a model, enter that provider's key, then use the row. Limits and availability can change; official account and limit links are beside each key.</p></div></div><button id="llm-refresh-free" class="action-btn ghost" type="button">Refresh free list</button></div>
          <div class="online-model-head"><span>Use</span><span>Provider</span><span>Model</span><span>Encrypted API key</span><span>Account and help</span></div>
          <div class="online-model-row" data-provider-row="openrouter" data-fixed-model="openrouter/free"><input type="radio" name="online-active" aria-label="Use OpenRouter automatic free router"><strong>OpenRouter</strong><select disabled><option>Automatic free router</option></select><input class="row-api-key" type="password" autocomplete="off" placeholder="OpenRouter key"><div><button class="text-link llm-row-key" type="button">Save key</button><button class="text-link llm-direct-guide" data-url="https://openrouter.ai/keys" type="button">Get key ↗</button><button class="text-link llm-direct-guide" data-url="https://openrouter.ai/docs/quickstart" type="button">Guide ↗</button></div></div>
          <div class="online-model-row" data-provider-row="openrouter"><input type="radio" name="online-active" aria-label="Use a specific free OpenRouter model"><strong>OpenRouter</strong><select id="llm-free-model"><option value="openrouter/free">Loading current free models…</option></select><input class="row-api-key" type="password" autocomplete="off" placeholder="Same OpenRouter key"><div><button class="text-link llm-row-key" type="button">Save key</button><button class="text-link llm-direct-guide" data-url="https://openrouter.ai/models?fmt=cards&max_price=0" type="button">Models ↗</button><button class="text-link llm-direct-guide" data-url="https://openrouter.ai/docs/faq" type="button">Limits ↗</button></div></div>
          <div class="online-model-row" data-provider-row="groq"><input type="radio" name="online-active" aria-label="Use Groq"><strong>Groq</strong><select><option value="openai/gpt-oss-20b">GPT-OSS 20B</option><option value="openai/gpt-oss-120b">GPT-OSS 120B</option><option value="qwen/qwen3.8-27b">Qwen3.8 27B</option></select><input class="row-api-key" type="password" autocomplete="off" placeholder="Groq key"><div><button class="text-link llm-row-key" type="button">Save key</button><button class="text-link llm-direct-guide" data-url="https://console.groq.com/keys" type="button">Get key ↗</button><button class="text-link llm-direct-guide" data-url="https://console.groq.com/docs/rate-limits" type="button">Limits ↗</button></div></div>
          <div class="online-model-row" data-provider-row="gemini"><input type="radio" name="online-active" aria-label="Use Google Gemini"><strong>Gemini</strong><select><option value="gemini-3.8-flash">Gemini 3.8 Flash</option><option value="gemini-3.5-flash-lite">Gemini 3.5 Flash-Lite</option><option value="gemini-2.5-flash-lite">Gemini 2.5 Flash-Lite</option></select><input class="row-api-key" type="password" autocomplete="off" placeholder="Gemini key"><div><button class="text-link llm-row-key" type="button">Save key</button><button class="text-link llm-direct-guide" data-url="https://aistudio.google.com/app/apikey" type="button">Get key ↗</button><button class="text-link llm-direct-guide" data-url="https://ai.google.dev/gemini-api/docs/pricing" type="button">Limits ↗</button></div></div>
          <div class="online-model-row" data-provider-row="huggingface"><input type="radio" name="online-active" aria-label="Use Hugging Face inference"><strong>Hugging Face</strong><select><option value="openai/gpt-oss-120b:fastest">GPT-OSS 120B</option><option value="openai/gpt-oss-20b:fastest">GPT-OSS 20B</option></select><input type="text" disabled value="Uses token above" aria-label="Uses Hugging Face token above"><div><button class="text-link llm-direct-guide" data-url="https://huggingface.co/playground" type="button">Playground ↗</button><button class="text-link llm-direct-guide" data-url="https://huggingface.co/docs/inference-providers/pricing" type="button">Credits ↗</button></div></div>
          <div id="llm-free-model-status" class="row-status" role="status">Refresh to verify the current OpenRouter free catalog.</div>
        </div>
        <div class="model-section usage-section"><div class="model-section-title"><div><h4>Usage and saved credentials</h4><p>Smart Butler falls back after a provider rate-limit or availability error.</p></div></div><div class="model-inline"><button id="llm-refresh-usage" class="action-btn ghost" type="button">Check reported usage</button><button id="llm-remove-key" class="action-btn ghost" type="button">Remove active provider key</button><div id="llm-usage-report" role="status">No usage check run.</div></div></div>
        <p class="model-privacy-note">Online mode sends your request and relevant public references to the active provider. Stored contacts, birth dates and the private local draft are not supplied as reference context. Current facts continue to use Smart Butler's public news, weather and market sources.</p>
      </section>
      <section class="settings-view hidden" data-settings-page="logs"><div class="settings-view-heading"><h3>Smart Butler log</h3><div><button id="refresh-logs" class="action-btn ghost">Refresh</button><button id="open-log-folder" class="action-btn ghost">Open folder ↗</button></div></div><pre id="settings-log" tabindex="0" aria-label="Recent Smart Butler log entries">Choose Logs to load recent entries.</pre></section>
      <section class="settings-view hidden" data-settings-page="learning"><h3>Learning & prefetch</h3><div class="settings-page-actions"><button id="prefetch" class="action-btn" type="button">${icon('refresh')} Prefetch now</button></div><div class="device-report" id="learning-report"></div><div class="settings-page-actions" id="learning-settings-actions"></div><div id="learning-settings-detail" class="settings-detail" aria-live="polite"></div></section>

    </div></div></div></div>
  <div class="modal hidden" id="onboarding"><div class="modal-card onboarding-card"><div class="onboard-butler-mark"><img src="/assets/butler-welcome.png" alt="Smart Butler"/></div><div class="kicker">LET’S GET ACQUAINTED</div><h2>What should I call you?</h2><p>Smart Butler works best when it knows who it is talking with.</p><input id="profile-name" placeholder="Your preferred name"/><input id="profile-location" placeholder="City"/><input id="profile-region" placeholder="Region / state"/><input id="profile-interests" placeholder="Interests, separated by commas"/><label class="consent"><input id="profile-consent" type="checkbox"/> Save these details on this device so Smart Butler can personalize conversations.</label><button id="save-profile" class="primary-wide">Meet Smart Butler</button></div></div>
  `;
}

function setButlerState(state: 'idle'|'listen'|'talk'|'happy'|'sleep') {
  const p = document.querySelector('#butler-mascot');
  if (p) {p.setAttribute('data-state', state);const image=p.querySelector<HTMLImageElement>('img');if(image){image.src=`/assets/butler-${state==='listen'?'listening':state==='sleep'?'resting':'welcome'}.png`;image.alt=state==='listen'?'Your orangutan butler is listening':state==='sleep'?'Your orangutan butler is resting':'Your orangutan butler at your service';}}
  const hero = document.querySelector('#voice-caption');
  if (hero) hero.textContent = state === 'listen' ? 'Voice status' : state === 'talk' ? 'Smart Butler is talking…' : state === 'happy' ? 'That made Smart Butler smile.' : state === 'sleep' ? 'Resting, but still here.' : 'Always ready when you are.';
}

function pushMessage(text: string, who: 'user'|'butler' = 'butler', meta = '') {
  const box = document.querySelector<HTMLDivElement>('#chat-messages');
  if (!box) return;
  const row = document.createElement('div'); row.className = `message ${who}`;
  row.innerHTML = `<div class="bubble">${esc(text)}${meta ? `<small>${esc(meta)}</small>` : ''}</div>`;
  box.appendChild(row);
  const scroll = document.querySelector('#chat-scroll') as HTMLElement | null;
  if (scroll) scroll.scrollTop = scroll.scrollHeight;
}

function showPersonaSource(response: any){
  const source=response?.plugin?.data;
  if(response?.plugin?.plugin_id!=='butler-persona' || !source?.source_url || !String(source.source_url).startsWith('https://'))return;
  const box=document.querySelector<HTMLDivElement>('#chat-messages');if(!box)return;
  const row=document.createElement('div');row.className='persona-source';
  const link=document.createElement('button');link.type='button';link.textContent=`Source: ${source.source_name||'Read more'} ↗`;
  link.addEventListener('click',()=>void invoke('open_external',{url:source.source_url}).catch(error=>pushMessage(`Could not open source: ${String(error)}`)));
  row.append(link);box.append(row);
}

function setRefreshing(id: string, on: boolean) { const el = document.querySelector<HTMLElement>(`#${id}`); if (el) el.classList.toggle('spinning', on); }

function weatherIcon(code: number) {
  if (code === 0) return '☀'; if ([1,2].includes(code)) return '⛅'; if (code === 3) return '☁'; if ([45,48].includes(code)) return '🌫'; if ([51,53,55,61,63,65,80,81,82].includes(code)) return '🌧'; if ([71,73,75,85,86].includes(code)) return '❄'; if ([95,96,99].includes(code)) return '⛈'; return '☀';
}
function formatCityTime(iso: string, timezone?: string) {
  const d = iso ? new Date(iso) : new Date();
  const fmt = (opts: Intl.DateTimeFormatOptions) => new Intl.DateTimeFormat(undefined, { ...opts, timeZone: timezone || undefined });
  return { time: fmt({hour:'numeric',minute:'2-digit'}).format(d), day: fmt({weekday:'long'}).format(d), date: fmt({day:'numeric',month:'long',year:'numeric'}).format(d) };
}

async function weather(location=selectedWeatherCity||profile.location, updateHeader=location===profile.location) {
  if(refreshing.has('weather'))return; refreshing.add('weather');
  setRefreshing('weather-refresh', true);
  void renderWeatherReports(location);
  try {
    const r: any = await invoke('weather_widget', { location });
    const cur = r?.current || r;
    const numberOrNull = (value:any) => value === null || value === undefined || value === '' ? null : Number.isFinite(Number(value)) ? Number(value) : null;
    const temperature = numberOrNull(cur?.temperature ?? cur?.temp);
    const tz = r?.timezone || cur?.timezone;
    weatherTimezone = tz;
    const iso = new Date().toISOString();
    const t = formatCityTime(iso, tz);
    document.querySelector('#weather-location')!.textContent = location===profile.location?`${location} · Home`:location;
    document.querySelector('#weather-time')!.textContent = t.time;
    document.querySelector('#weather-day')!.textContent = t.day;
    document.querySelector('#weather-date')!.textContent = t.date;
    const currentIcon=weatherIcon(Number(cur?.weather_code ?? cur?.weathercode ?? cur?.code ?? 0));
    document.querySelector('#weather-graphic')!.textContent = currentIcon;
    document.querySelector('#weather-temp')!.textContent = temperature === null ? '—' : `${Math.round(temperature)}°C`;
    document.querySelector('#weather-condition')!.textContent = String(cur?.condition ?? cur?.description ?? 'Current conditions');
    const feels=numberOrNull(cur?.apparent_temperature ?? cur?.feels_like), humidity=numberOrNull(cur?.humidity), wind=numberOrNull(cur?.wind ?? cur?.wind_speed);
    document.querySelector('#weather-facts')!.innerHTML = `<span>Feels ${feels===null?'—':`${Math.round(feels)}°`}</span><span>💧 ${humidity===null?'—':`${Math.round(humidity)}%`}</span><span>💨 ${wind===null?'—':`${Math.round(wind)} km/h`}</span>`;
    const days = Array.isArray(r?.daily) ? r.daily : Array.isArray(r?.forecast) ? r.forecast : [];
    if(updateHeader){
      latestWeather = { temperature, condition:String(cur?.condition ?? cur?.description ?? 'current conditions'), feels, humidity, wind, days, timezone:tz };
      document.querySelector('#header-location')!.textContent=profile.location;
      document.querySelector('#header-region')!.textContent=[profile.region,profile.country].filter(Boolean).join(', ');
      document.querySelector('#header-weather-icon')!.textContent=currentIcon;
      document.querySelector('#header-weather-temp')!.textContent=temperature===null?'—°':`${Math.round(temperature)}°C`;
      document.querySelector('#header-weather-summary')!.textContent=String(cur?.condition ?? cur?.description ?? 'Current conditions');
      const today=days[0];
      document.querySelector('#header-weather-forecast')!.textContent=today?`Today ${Math.round(Number(today.high ?? today.temp_max ?? today.max ?? 0))}° / ${Math.round(Number(today.low ?? today.temp_min ?? today.min ?? 0))}° · humidity ${humidity===null?'—':`${Math.round(humidity)}%`} · wind ${wind===null?'—':`${Math.round(wind)} km/h`}`:'Today’s forecast is unavailable';
    }
    document.querySelector('#forecast')!.innerHTML = days.slice(0,5).map((d:any, i:number) => { const date = new Date(`${d.date}T12:00:00`); const day = date.toLocaleDateString(undefined,{weekday:'short'}); const label = date.toLocaleDateString(undefined,{day:'numeric',month:'short'}); return `<div class="forecast-card ${i===0?'today':''}"><b>${esc(day)}</b><small>${esc(label)}</small><div class="forecast-icon">${weatherIcon(Number(d.weather_code ?? d.weathercode ?? d.code ?? 0))}</div><strong>${Math.round(Number(d.high ?? d.temp_max ?? d.max ?? 0))}°</strong><span>${Math.round(Number(d.low ?? d.temp_min ?? d.min ?? 0))}°</span></div>`; }).join('');
    document.querySelector('#weather-live')!.textContent = temperature === null ? 'INCOMPLETE' : 'LIVE';
  } catch (e) { document.querySelector('#weather-live')!.textContent = 'UNAVAILABLE'; document.querySelector('#weather-condition')!.textContent=String(e); }
  finally { refreshing.delete('weather'); setRefreshing('weather-refresh', false); }
}

async function refreshHomeWeather(){
  try{
    if(!homeCoordinates&&navigator.geolocation)homeCoordinates=await new Promise(resolve=>navigator.geolocation.getCurrentPosition(position=>resolve({latitude:position.coords.latitude,longitude:position.coords.longitude}),()=>resolve(null),{enableHighAccuracy:false,timeout:4500,maximumAge:30*60_000}));
    const r:any=homeCoordinates?await invoke('weather_coordinates',homeCoordinates):await invoke('weather_widget',{location:profile.location});const cur=r?.current||r;const days=Array.isArray(r?.daily)?r.daily:Array.isArray(r?.forecast)?r.forecast:[];const num=(v:any)=>v===null||v===undefined||v===''?null:Number.isFinite(Number(v))?Number(v):null;const temperature=num(cur?.temperature??cur?.temp),humidity=num(cur?.humidity),wind=num(cur?.wind??cur?.wind_speed),feels=num(cur?.apparent_temperature??cur?.feels_like);const currentIcon=weatherIcon(Number(cur?.weather_code??cur?.weathercode??cur?.code??0));latestWeather={temperature,condition:String(cur?.condition??cur?.description??'current conditions'),feels,humidity,wind,days,timezone:r?.timezone||cur?.timezone};document.querySelector('#header-location')!.textContent=profile.location;document.querySelector('#header-region')!.textContent=`${[profile.region,profile.country].filter(Boolean).join(', ')}${homeCoordinates?' · device location':''}`;document.querySelector('#header-weather-icon')!.textContent=currentIcon;document.querySelector('#header-weather-temp')!.textContent=temperature===null?'—°':`${Math.round(temperature)}°C`;document.querySelector('#header-weather-summary')!.textContent=latestWeather.condition;const today=days[0];const rain=today?num(today.rain_probability):null;document.querySelector('#header-weather-forecast')!.textContent=today?`Today ${Math.round(Number(today.high??today.temp_max??today.max??0))}° / ${Math.round(Number(today.low??today.temp_min??today.min??0))}° · rain ${rain===null?'—':`${Math.round(rain)}%`} · wind ${wind===null?'—':`${Math.round(wind)} km/h`}`:'Today’s forecast is unavailable';
  }catch{document.querySelector('#header-weather-forecast')!.textContent='Home forecast temporarily unavailable';}
}

const storySnapshots=new Map<string,NewsItem[]>();
const storyViews=new Map<Element,string>();
let storySnapshotContext='';
function paintSectionedStories(target:Element,items:NewsItem[]){
 const groups=groupStories(items);const sections=new Map<string,NewsItem[]>();for(const group of groups){const item=group.lead as NewsItem;const label=item.section||'Headlines';if(!sections.has(label))sections.set(label,[]);sections.get(label)!.push(item,...group.coverage.filter(row=>row!==item) as NewsItem[]);}
 if(sections.size<2)return paintStories(target,items);
 target.replaceChildren();target.classList.toggle('sports-culture',activeNews==='sports'&&target.id==='news-list');for(const [name,rows]of sections){const section=document.createElement('section');section.className='story-section';const label=document.createElement('h3');label.className='story-section-title';label.textContent=name;const list=document.createElement('div');section.append(label,list);target.append(section);paintStories(list,rows);}return groups;
}
function renderStories(target:Element,items:NewsItem[]){
 const context=JSON.stringify([profile.country,profile.region,profile.location]);
 if(context!==storySnapshotContext){storySnapshotContext=context;storySnapshots.clear();storyViews.clear();}
 const marketView=document.querySelector<HTMLElement>('.market-tabs [data-market-view].active')?.dataset.marketView||'market-news';
 const category=target.id==='news-list'?activeNews:target.closest('#market-browser')?(marketView.startsWith('business-')||marketView==='market-news'?'business':marketView):'';
 if(!category)return paintStories(target,items);
 storySnapshots.set(category,items);storyViews.set(target,category);
 const partitions=partitionStories([...storySnapshots].map(([category,items])=>({category,items})));
 let selected:ReturnType<typeof groupStories>=[];
 for(const [view,key] of storyViews){
  if(!view.isConnected){storyViews.delete(view);continue;}
  const groups=partitions.get(key)||[];
  const rendered=paintSectionedStories(view,groups.flatMap(group=>group.coverage));
  if(view===target)selected=rendered;
  if(view.id==='news-list'){
   const visible=rendered.map(group=>group.lead);
   renderNewsTicker(document.querySelector('#news-ticker'),visible);
   renderNewsTicker(document.querySelector('#chat-ticker'),visible);
   void invoke('visible_news_set',{headlines:visible.slice(0,8).map(item=>item.title)}).catch(()=>{});
  }
 }
 return selected;
}

function paintStories(target:Element,items:NewsItem[]){
 const groups=groupStories(items);
 target.innerHTML=groups.map((group,index)=>`<article class="story-card"><button class="story-lead" data-story="${index}"><div><b>${esc(headlineTitle(group.lead))}</b><span class="story-excerpt">${esc(articleExcerpt(group.lead))}</span><small>${esc(group.lead.source||'Publisher')} · ${esc(group.lead.published||'Date unavailable')}</small></div><span>↗</span></button>${group.coverage.length>1?`<details class="story-coverage"><summary>${group.coverage.length} reports · related coverage</summary>${group.coverage.map((item,other)=>`<button data-group="${index}" data-coverage="${other}"><b>${esc(item.source||'Publisher')}</b><span>${esc(item.title)}</span><small>${esc(item.published||'')}</small></button>`).join('')}</details>`:''}</article>`).join('')||'<p class="empty-state">No current stories are available from these sources.</p>';
 target.querySelectorAll<HTMLElement>('[data-story]').forEach(button=>button.addEventListener('click',()=>openNews(groups[Number(button.dataset.story)].lead,true)));
 target.querySelectorAll<HTMLElement>('[data-coverage]').forEach(button=>button.addEventListener('click',()=>openNews(groups[Number(button.dataset.group)].coverage[Number(button.dataset.coverage)],true)));
 return groups;
}
function renderNewsTicker(target:Element|null,items:NewsItem[]){if(!target)return;const groups=groupStories(items).slice(0,20);target.innerHTML=`<div class="butler-ticker-track">${groups.map((group,index)=>`<button data-ticker-story="${index}">${esc(headlineTitle(group.lead))} <b>${esc(group.lead.source||'News')}</b></button>`).join('')}</div>`;const track=target.querySelector<HTMLElement>('.butler-ticker-track');if(track){track.style.animationDuration=`${Math.max(40,groups.reduce((total,group)=>total+group.lead.title.length+(group.lead.source?.length||0)+12,0)/7)}s`;}
 target.querySelectorAll<HTMLElement>('[data-ticker-story]').forEach(button=>button.addEventListener('click',()=>openNews(groups[Number(button.dataset.tickerStory)].lead,true)));}

async function news(category = activeNews,force=false,readAloud=false) {
  const request=++newsRequest;if(readAloud)stopNewsReading();
  try{await loadSourceCatalog();}catch{/* Native widget reports the consent/network failure. */}
  if(request!==newsRequest)return;refreshing.add('news');
  activeNews = category;
  document.querySelector('#news-scope')!.textContent=newsScope(category,profile.country,profile.region,profile.location);
  const sourceSelect=document.querySelector<HTMLSelectElement>('#news-source')!;
  sourceSelect.innerHTML=sourceOptions(category);sourceSelect.value=currentSource(category);
  setRefreshing('news-refresh', true);
  document.querySelectorAll('#news-tabs button').forEach(b => b.classList.toggle('active', (b as HTMLElement).dataset.cat === category));
  try {
    const requests=[invoke<any>('news_widget',{category,source:currentSource(category),force})];
    const categories=[category];
    if(currentSource(category)==='all'){if(category==='science')categories.push('ai-tech');if(category==='local')categories.push('regional');if(category==='sports')categories.push('entertainment');if(category==='business')categories.push('business-world');}
    for(const extra of categories.slice(1))requests.push(invoke<any>('news_widget',{category:extra,source:'all',force}));
    const results=await Promise.allSettled(requests);
    const successful=results.flatMap((result,i)=>result.status==='fulfilled'?[{...result.value,items:(result.value.items||[]).map((item:any)=>({...item,section:categories[i]==='regional'?'Region':categories[i]==='local'?'Local':categories[i]==='entertainment'?'Culture':categories[i]==='sports'?'Sports':categories[i]==='business-world'?'World economy':categories[i]==='business'?'Home economy':/\b(government|election|parliament|minister|politic|president|council|assembly)\b/i.test(item.title)?'Public affairs':'Other news'}))}]:[]);
    if(!successful.length)throw (results[0] as PromiseRejectedResult).reason;
    const r:any={...successful[0],items:successful.flatMap(result=>result.items||[]),errors:successful.flatMap(result=>result.errors||[])};
    if(results[0].status!=='fulfilled')delete r.source_catalog;
    if(results.some(result=>result.status==='rejected'))r.errors.push('A source group is temporarily unavailable.');
    if(request!==newsRequest)return;
    if(r.source_catalog){curatedNews[category]=r.source_catalog;sourceSelect.innerHTML=sourceOptions(category);sourceSelect.value=currentSource(category);document.querySelector('#news-scope')!.textContent=r.source_catalog.scope;}
    const items:NewsItem[] = (r?.items || []).filter(isReadableStory);
    // Do not rank every politician mention above other news; groupStories orders dated reports.
    newsItems = items;
    renderNewsTicker(document.querySelector('#news-ticker'),items);
    renderNewsTicker(document.querySelector('#chat-ticker'),items);
    const list=document.querySelector('#news-list')!;const groups=renderStories(list,items);
    await invoke('visible_news_set',{headlines:groups.map(group=>group.lead.title)});
    if(request!==newsRequest)return;
    if(readAloud)void readCategoryHeadlines(items);
    document.querySelector('#news-live')!.textContent = `${r.errors?.length?'Partial · ':''}${groups.length} stories`;
  } catch(e) { if(request!==newsRequest)return; document.querySelector('#news-live')!.textContent = 'UNAVAILABLE'; document.querySelector('#news-list')!.textContent=String(e); void invoke('visible_news_set',{headlines:[]}).catch(()=>{}); }
  finally { if(request===newsRequest){refreshing.delete('news'); setRefreshing('news-refresh', false);} }
}

const spokenHeadline = (value:string) => value.replace(/\s+-\s+[^-]{2,45}$/,'').replace(/\s+/g,' ').trim();
const firstUsefulHeadline = (items:NewsItem[]) => items.filter(isReadableStory).map(item=>spokenHeadline(item.title)).find(title=>title.length>12) || '';

async function contextualNews(category:'local'|'regional'|'national'):Promise<NewsItem[]> {
  try {
    const result:any=await invoke('news_widget',{category,source:currentSource(category)});
    const items:Array<NewsItem>=Array.isArray(result?.items)?result.items:[];
    if(category==='local')localNewsItems=items;
    else if(category==='regional')regionalNewsItems=items;
    else nationalNewsItems=items;
    return items;
  } catch { return []; }
}

async function localEvents():Promise<NewsItem[]> {
  try{
    const result:any=await invoke('local_events_widget');
    localEventItems=Array.isArray(result?.items)?result.items:[];
    return localEventItems;
  }catch{return [];}
}

function scheduleForToday():string[] {
  const timezone=profile.timezone||Intl.DateTimeFormat().resolvedOptions().timeZone;
  const {day}=currentDayAndTime(new Date(),timezone);
  return listReminderEvents(profile).filter(event=>{
    if(!event.enabled||!event.date)return false;
    return event.annual?event.date.slice(5)===day.slice(5):event.date===day;
  }).sort((a,b)=>(a.time||'09:00').localeCompare(b.time||'09:00')).map(event=>`${event.time||'09:00'}, ${event.title}`);
}

const localDateKey=(date:Date)=>`${date.getFullYear()}-${String(date.getMonth()+1).padStart(2,'0')}-${String(date.getDate()).padStart(2,'0')}`;
function remindersForDate(date:Date){
  const key=localDateKey(date);
  return listReminderEvents(profile).filter(event=>event.enabled&&event.date&&(event.annual?event.date.slice(5)===key.slice(5):event.date===key)).sort((a,b)=>(a.time||'09:00').localeCompare(b.time||'09:00'));
}
function hinduCalendarContext(date:Date){
  let saka='Indian national calendar unavailable';
  try{saka=new Intl.DateTimeFormat('en-IN-u-ca-indian',{day:'numeric',month:'long',year:'numeric'}).format(date);}catch{}
  const synodic=29.53058867;
  const epoch=Date.UTC(2000,0,6,18,14);
  const age=(((date.getTime()-epoch)/86400000)%synodic+synodic)%synodic;
  const tithi=Math.floor(age/synodic*30)+1;
  const names=['Pratipada','Dwitiya','Tritiya','Chaturthi','Panchami','Shashthi','Saptami','Ashtami','Navami','Dashami','Ekadashi','Dwadashi','Trayodashi','Chaturdashi','Purnima'];
  const paksha=tithi<=15?'Shukla Paksha':'Krishna Paksha';
  const name=tithi===30?'Amavasya':names[(tithi-1)%15];
  const phase=tithi===30||tithi===1?'New Moon':tithi===15?'Full Moon':tithi<15?'Waxing Moon':'Waning Moon';
  return {saka,tithi:`${paksha} · ${name} (approximately tithi ${tithi})`,phase};
}

function renderCalendar(){
  const title=document.querySelector<HTMLElement>('#calendar-month-title');
  const grid=document.querySelector<HTMLElement>('#calendar-grid');
  if(!title||!grid)return;
  title.textContent=calendarCursor.toLocaleDateString(undefined,{month:'long',year:'numeric'});
  const first=new Date(calendarCursor.getFullYear(),calendarCursor.getMonth(),1);
  const total=new Date(calendarCursor.getFullYear(),calendarCursor.getMonth()+1,0).getDate();
  const cells:string[]=[];
  for(let i=0;i<first.getDay();i++)cells.push('<span class="calendar-blank"></span>');
  for(let day=1;day<=total;day++){
    const date=new Date(calendarCursor.getFullYear(),calendarCursor.getMonth(),day);
    const reminders=remindersForDate(date);
    const today=localDateKey(date)===localDateKey(new Date());
    const selected=localDateKey(date)===localDateKey(selectedCalendarDate);
    cells.push(`<button type="button" data-calendar-day="${day}" class="${today?'today ':''}${selected?'selected':''}"><b>${day}</b>${reminders.length?`<span>${reminders.length}</span>`:''}</button>`);
  }
  grid.innerHTML=cells.join('');
  grid.querySelectorAll<HTMLButtonElement>('[data-calendar-day]').forEach(button=>button.addEventListener('click',()=>{
    selectedCalendarDate=new Date(calendarCursor.getFullYear(),calendarCursor.getMonth(),Number(button.dataset.calendarDay));
    renderCalendar();renderDaybook();void loadCalendarContext(selectedCalendarDate);
  }));
}

function renderDaybook(){
  const box=document.querySelector<HTMLElement>('#daybook-summary');if(!box)return;
  const date=selectedCalendarDate;
  const hindu=hinduCalendarContext(date);
  const reminders=remindersForDate(date);
  const drikDate=`${String(date.getDate()).padStart(2,'0')}/${String(date.getMonth()+1).padStart(2,'0')}/${date.getFullYear()}`;
  const panchangProviders=[{name:'Drik Panchang',url:`https://www.drikpanchang.com/panchang/day-panchang.html?date=${encodeURIComponent(drikDate)}`},{name:'Prokerala',url:'https://www.prokerala.com/astrology/panchang/'},{name:'AstroSage',url:'https://panchang.astrosage.com/?language=en'},{name:'MyPanchang',url:'https://www.mypanchang.com/'}];
  box.innerHTML=`<div class="daybook-date"><strong>${esc(date.toLocaleDateString(undefined,{weekday:'long',day:'numeric',month:'long',year:'numeric'}))}</strong><span>${esc(hindu.saka)} · estimated ${esc(hindu.tithi)} · ${esc(hindu.phase)}</span><small>Panchang location: ${esc([profile.location,profile.region,profile.country].filter(Boolean).join(', '))}</small></div><div class="daybook-reminders"><b>${reminders.length?'Schedule':'Schedule clear'}</b>${reminders.map(item=>`<span>${esc(item.time||'09:00')} · ${esc(item.title)}</span>`).join('')||'<span>No saved reminder for this date.</span>'}</div><div class="panchang-options"><label>Panchang source<select id="panchang-provider" aria-label="Panchang source">${panchangProviders.map((provider,index)=>`<option value="${index}">${provider.name}</option>`).join('')}</select></label><button id="calendar-panchang" class="source-btn" type="button">Open Panchang ↗</button><small>Confirm your city and date on the provider’s page. If one site is unavailable, choose another.</small></div>`;
  box.querySelector('#calendar-panchang')?.addEventListener('click',()=>{const selected=Number(box.querySelector<HTMLSelectElement>('#panchang-provider')?.value||0);const url=panchangProviders[selected]?.url||panchangProviders[0].url;if('__TAURI_INTERNALS__' in window)void invoke('open_external',{url}).catch(error=>pushMessage(String(error)));else window.open(url,'_blank','noopener,noreferrer');});
}

async function loadCalendarContext(date=selectedCalendarDate){
  const box=document.querySelector<HTMLElement>('#day-context');if(!box)return;
  box.innerHTML='<p>Gathering local, regional, national, science and historical context…</p>';
  try{
    const data:any=await invoke('calendar_day_context',{year:date.getFullYear(),month:date.getMonth()+1,day:date.getDate()});
    const section=(label:string,items:any[])=>items?.length?`<section><h3>${esc(label)}</h3>${items.slice(0,5).map((item:any)=>`<button class="day-context-item" data-url="${esc(item.url)}"><b>${item.year!==undefined?`${esc(item.year)} · `:''}${esc(item.title||item.text)}</b><small>${esc(item.source||item.published||'Open source')}</small></button>`).join('')}</section>`:'';
    box.innerHTML=section(`Around ${profile.location}`,data.local)+section(`Across ${profile.region}`,data.regional)+section(profile.country||'National',data.national)+section('Science and discovery',data.science)+section('This day in history',data.history)+`<small>${esc(data.attribution||'')}</small>`;
    if(!box.querySelector('section'))box.innerHTML='<p>No dated context was returned. Your saved schedule remains available above.</p>';
    box.querySelectorAll<HTMLButtonElement>('[data-url]').forEach(button=>button.addEventListener('click',()=>void invoke('open_external',{url:button.dataset.url}).catch(error=>pushMessage(String(error)))));
  }catch(error){box.innerHTML=`<p>Day context is unavailable: ${esc(error)}</p>`;}
}

function weatherBriefing():string {
  if(!latestWeather||latestWeather.temperature===null)return `I could not get a reliable weather reading for ${profile.location} yet, so I will keep trying quietly.`;
  const today=latestWeather.days?.[0]||{};
  const high=Number(today.high??today.temp_max??today.max);
  const low=Number(today.low??today.temp_min??today.min);
  const range=Number.isFinite(high)&&Number.isFinite(low)?` Today's range is about ${Math.round(low)} to ${Math.round(high)} degrees.`:'';
  const wind=Number.isFinite(Number(latestWeather.wind))?` Winds are near ${Math.round(Number(latestWeather.wind))} kilometres per hour.`:'';
  const rain=Number(today.rain_probability);const chance=today.rain_probability!=null&&Number.isFinite(rain)?` There is a ${Math.round(rain)} percent chance of rain.`:'';
  return `In ${profile.location}, it is ${Math.round(Number(latestWeather.temperature))} degrees with ${latestWeather.condition}.${range}${chance}${wind}`;
}

function marketBriefing():string {
  const live=marketItems.filter(item=>item.price!==undefined);
  const outlook=marketNewsItems.find(item=>/analyst|outlook|brokerage|market forecast|market preview/i.test(item.title));
  const report=outlook?` One reported market outlook: ${headlineText(outlook)}. This is an outlook, not a certainty.`:'';
  if(!live.length)return 'Market quotes are still being checked.'+report;
  const movers=[...live].filter(item=>Number.isFinite(Number(item.change_percent))).sort((a,b)=>Math.abs(Number(b.change_percent))-Math.abs(Number(a.change_percent))).slice(0,3);
  if(!movers.length)return `${live.length} leading market indices have current quotes in Market Watch.`;
  return `Market watch: ${movers.map(item=>`${item.name} is ${Number(item.change_percent)>=0?'up':'down'} ${Math.abs(Number(item.change_percent)).toFixed(2)} percent`).join('; ')}.${report}`;
}

const wantsDailyBriefing=(text:string)=>/\b(daily|full|today'?s?|morning|evening)?\s*briefing\b/i.test(text)||/\bbrief me\b/i.test(text);
let briefingInProgress=false;
async function requestDailyBriefing(){if(responding)return;resumeSpeech();await runDailyBriefing('Your requested briefing');}
async function runDailyBriefing(reason='On request'){
 if(responding||speechSession.muted)return;
 stopNewsReading();cancelListening();
 responding=true;briefingInProgress=true;briefingCancelled=false;briefingOfferPending=false;lastInteractionAt=Date.now();
 const ticket=speechSession.ticket();const active=()=>!briefingCancelled&&speechSession.valid(ticket);const say=async(text:string,label:string)=>{if(active())await sayCompanionLine(text,label);};
 const fetched=invoke<any>('briefing_headlines').catch(()=>null);const updates=Promise.allSettled([refreshHomeWeather(),weather(),markets()]);
 try{await say(greetingLine(),'Greeting');await Promise.race([updates,new Promise(resolve=>setTimeout(resolve,2000))]);if(!active())return;
 await say(weatherBriefing(),'Weather now and today');const schedule=scheduleForToday();await say(schedule.length?`Today, ${schedule.slice(0,5).join('; ')}.`:'Your diary is clear today. A little breathing room is welcome.','Reminders and schedule');
 const result:any=await Promise.race([fetched,new Promise(resolve=>setTimeout(()=>resolve(null),1800))]);if(!active())return;
 const categories=result?.categories||[{category:'national',items:nationalNewsItems},{category:'regional',items:regionalNewsItems},{category:'local',items:localNewsItems},{category:activeNews,items:newsItems},{category:'business-national',items:marketNewsItems}];
 const headlines=briefingSelection(categories);await say(headlines.length?'Here are the selected headlines.':'Headlines are still being checked. You can find updates in News.','Headlines');
 for(const item of headlines){if(!active())break;await say(headlineText(item)+'.','Headline');}
 await say(marketBriefing(),'Market Watch');await say('That is your briefing. The full coverage is ready when you want it.','Closing');
 }finally{responding=false;briefingInProgress=false;lastChatterAt=Date.now();setButlerState('idle');if(active()&&!isQuiet())startListening();}
}

async function offerScheduledBriefing(){
  if(routineQuiet()||isQuiet()||!profile.onboarding_complete||briefingOfferPending||responding||document.hidden||!proactiveChatter||Date.now()-lastInteractionAt<10*60_000)return;
  const timezone=profile.timezone||Intl.DateTimeFormat().resolvedOptions().timeZone;
  const hour=Number(new Intl.DateTimeFormat('en-US',{timeZone:timezone,hour:'2-digit',hourCycle:'h23'}).format(new Date()));
  const slot=hour>=7&&hour<10?'morning':hour>=11&&hour<14?'midday':hour>=16&&hour<19?'afternoon':hour>=20&&hour<23?'evening':'';
  if(!slot)return;
  const day=currentDayAndTime(new Date(),timezone).day,key=`butler.cleanr5.briefingOffer.${day}.${slot}`;
  if(localStorage.getItem(key))return;
  localStorage.setItem(key,'offered');briefingOfferPending=true;
  const message=`${profile.name}, may I offer a short ${slot} briefing?`;
  pushMessage(message,'butler','Briefing offer');await sayCompanionLine(message,'Briefing offer',false);
}

let courtesyCount=0;
function greetingLine():string {const hour=Number(localClock().slice(0,2));const address=++courtesyCount%3===0?'sir':profile.name;return `${hour<12?'Good morning':hour<18?'Good afternoon':'Good evening'}, ${address}. A pleasure to be at your service.`;}

async function waitForSpeechReady(maximumMs=20_000):Promise<boolean>{
  const until=Date.now()+maximumMs;
  while(Date.now()<until){
    await refreshStatus();
    if(lastVoice?.tts_ready&&!['Listening','Speaking','Synthesizing'].includes(lastVoice.state))return true;
    await new Promise(resolve=>setTimeout(resolve,600));
  }
  return false;
}

function hideLaunchSplash(){
  const splash=document.querySelector<HTMLElement>('#launch-splash');
  if(!splash)return;
  splash.classList.add('leaving');
  setTimeout(()=>splash.classList.add('hidden'),650);
}

async function sayCompanionLine(text:string,meta:string,show=true){
  if(isQuiet())return;
  if(show){pushMessage(text,'butler',meta);document.querySelector('#hero-line')!.textContent=text;}
  if(isQuiet()||!lastVoice?.tts_ready)return;
  setButlerState('talk');setVoiceTop('Speaking…');
  try{await invoke('voice_speak',{text,request:meta,capability:'persona.briefing'});}catch(error){pushMessage(`Speech problem: ${String(error)}`,'butler');}
  finally{setButlerState('idle');void refreshStatus();}
}

async function launchBriefing(){if(launchBriefingStarted||!profile.onboarding_complete||!document.querySelector('#onboarding')?.classList.contains('hidden'))return;launchBriefingStarted=true;startupBriefingWaitingSince=0;document.querySelector('#hero-line')!.textContent='Ready when you are. Use Daily briefing in the chat header.';}

function openNews(item: NewsItem, openArticle = false) {
  stopNewsReading();
  const request = ++coverageRequest;
  const modal = document.querySelector('#media-modal')!; const c = document.querySelector('#media-content')!; modal.classList.remove('hidden');
  c.innerHTML = `<div class="article-view">${item.thumbnail ? `<img src="${esc(item.thumbnail)}" class="article-image" alt=""/>` : ''}<h3>${esc(item.title)}</h3><small>${esc(item.source||'Publisher')} · Publisher excerpt</small><p>${esc(articleExcerpt(item))}</p><button class="source-btn" id="media-source">Read source ${icon('link')}</button></div>`;
  document.querySelector('#media-source')?.addEventListener('click', () => void invoke('open_external', { url: item.url }).catch(error => { const status=c.querySelector('#article-open-status'); if(status)status.textContent=`Could not open the article: ${String(error)}`; }));
  if (!openArticle) return;
  c.insertAdjacentHTML('beforeend', '<p id="article-open-status" class="article-open-status" role="status">Publisher excerpt shown. Read source opens the full article; publisher access conditions apply.</p><section class="coverage-section" aria-label="Other news outlets"><h4>Other outlets covering this story</h4><div id="coverage-list" role="status">Looking for matching coverage…</div></section>');
  void invoke<{items: NewsItem[]; searched: number}>('news_coverage', { title:item.title, source:item.source||'', url:item.url }).then(result => {
    if (request !== coverageRequest) return;
    const list=c.querySelector<HTMLElement>('#coverage-list'); if(!list)return;
    const related=result.items||[];
    list.innerHTML=related.length ? related.map((story,i)=>`<button class="coverage-story" data-index="${i}" type="button"><b>${esc(story.source||'News outlet')}</b><span>${esc(story.title)}</span>${icon('link')}</button>`).join('') : '<p>Other independently matching outlets were not found in this search. Try the source article for more context.</p>';
    list.querySelectorAll<HTMLButtonElement>('.coverage-story').forEach(button=>button.addEventListener('click',()=>void invoke('open_external',{url:related[Number(button.dataset.index)].url}).catch(error=>{const status=c.querySelector('#article-open-status');if(status)status.textContent=`Could not open that outlet: ${String(error)}`;})));
  }).catch(error=>{if(request===coverageRequest){const list=c.querySelector('#coverage-list');if(list)list.textContent=`Related coverage is unavailable: ${String(error)}`;}});
}

async function markets() {
  const request=++marketRequest;
  try{await loadSourceCatalog();}catch{/* The widget reports unavailable feeds. */}
  if(request!==marketRequest)return;
  const sourceSelect=document.querySelector<HTMLSelectElement>('#market-news-source');
  if(sourceSelect){const selected=sourceSelect.value;sourceSelect.innerHTML=marketSourceOptions();if(Array.from(sourceSelect.options).some(option=>option.value===selected))sourceSelect.value=selected;}
  refreshing.add('markets');
  setRefreshing('market-refresh', true);
  try {
    const exchange=document.querySelector<HTMLSelectElement>('#market-exchange')?.value||'all';
    const quoteTask=invoke<any>('market_widget',{exchange});
    void quoteTask.then(r=>{if(request!==marketRequest)return;marketItems = (r?.items || []).map((x:any)=>({...x,price: x.price ?? (x.close !== null && x.close !== undefined && x.close !== "" && Number.isFinite(Number(x.close)) ? Number(x.close):undefined)}));renderMarketIndices();}).catch(()=>{});
    const [quoteResult,newsRequestResult]=await Promise.allSettled([quoteTask,invoke<any>('market_news',{exchange,source:document.querySelector<HTMLSelectElement>('#market-news-source')?.value||'market'})]);
    const r:any=quoteResult.status==='fulfilled'?quoteResult.value:{items:marketItems.map(item=>({...item,quote:{...item.quote,refresh_error:'Refresh unavailable'}}))};
    const newsResult:any=newsRequestResult.status==='fulfilled'?newsRequestResult.value:{items:marketNewsItems};
    if(request!==marketRequest)return;
    marketItems = (r?.items || []).map((x:any)=>({...x,price: x.price ?? (x.close !== null && x.close !== undefined && x.close !== "" && Number.isFinite(Number(x.close)) ? Number(x.close):undefined)}));
    marketNewsItems=Array.isArray(newsResult?.items)?newsResult.items:[];
    if(newsResult.source_catalog){curatedMarket=newsResult.source_catalog;const select=document.querySelector<HTMLSelectElement>('#market-news-source');if(select){const selected=select.value;select.innerHTML=marketSourceOptions();if(Array.from(select.options).some(option=>option.value===selected))select.value=selected;}}
    renderNewsTicker(document.querySelector('#market-news-ticker'),marketNewsItems);
    await refreshWatchlistQuotes();renderMarketIndices();
  } catch(e) { if(request!==marketRequest)return; document.querySelector('#market-live')!.textContent='UNAVAILABLE'; document.querySelector('#market-grid')!.textContent=String(e); } finally { if(request===marketRequest){refreshing.delete('markets'); setRefreshing('market-refresh', false);} }
}

function filteredMarkets(){return marketItems.filter(item=>item.kind!=='stock');}
function renderMarketIndices(){
  drawWatchlistPopover();
  const refreshSelect=document.querySelector<HTMLSelectElement>('#price-refresh');if(refreshSelect)refreshSelect.value=String(priceRefreshMinutes());
  const selectedList=document.querySelector<HTMLSelectElement>('#ticker-choice')?.value||'home';
  const items=filteredMarkets();const grid=document.querySelector<HTMLElement>('#market-grid')!;
  grid.innerHTML=items.map(item=>{const time=item.timestamp?new Date(item.timestamp*1000).toLocaleString(undefined,{month:'short',day:'numeric',hour:'numeric',minute:'2-digit'}):'';const detail=item.price!=null?`${item.source||'Quote'} · ${time}`:item.error?'Feed unavailable':'No quote';const change=item.change_percent==null?NaN:Number(item.change_percent);return `<button class="market-card" data-symbol="${esc(item.symbol)}" title="${esc(item.error||detail)}"><div class="market-symbol">${esc((item.name||item.symbol).slice(0,3))}</div><div><b>${esc(item.name||item.symbol)}</b><small>${esc(item.region||'')} · ${esc(detail)}</small></div><strong>${item.price!=null?Number(item.price).toLocaleString(undefined,{maximumFractionDigits:2}):'—'}${Number.isFinite(change)?`<small class="${change>=0?'up':'down'}">${change>=0?'+':''}${change.toFixed(2)}%</small>`:''}</strong></button>`;}).join('')||'<p>No quotes yet. Select a saved watchlist or exchange and refresh.</p>';
  grid.querySelectorAll<HTMLButtonElement>('.market-card').forEach(button=>button.addEventListener('click',()=>{const item=items.find(x=>x.symbol===button.dataset.symbol);if(item){selectedMarketIndex=item;void openMarketDetail(item.symbol,item.name||item.symbol);}}));
  const tape=document.querySelector('#ticker-tape')!;
  const tickerSelection=document.querySelector<HTMLSelectElement>('#ticker-choice')?.value||'home';const ordered=[...filteredMarkets(),...marketItems.filter(row=>row.kind==='stock'),...(tickerSelection==='home'?[]:watchlistQuoteItems)];const tickerRows=ordered.filter((row,index)=>ordered.findIndex(other=>other.symbol===row.symbol)===index);
  const previousTime=tape.querySelector('.price-ticker-track')?.getAnimations()[0]?.currentTime;
  tape.innerHTML=`<div class="price-ticker-heading"><b>MARKET WATCH</b><span>Yahoo Finance · refreshing every ${priceRefreshMinutes()} minutes · exchange delays apply</span><button id="ticker-pause" type="button" aria-pressed="${tape.classList.contains('ticker-paused')}">${tape.classList.contains('ticker-paused')?'Resume ticker':'Pause ticker'}</button></div><div class="price-ticker-window"><div class="price-ticker-track">${tickerRows.map(item=>{const change=item.change_percent==null?NaN:Number(item.change_percent);return `<button data-price-symbol="${esc(item.symbol)}"><strong>${esc(item.name||item.symbol)}</strong><b>${esc(item.currency||'')} ${item.price!=null?Number(item.price).toLocaleString(undefined,{maximumFractionDigits:2}):'Unavailable'}</b><em class="${Number.isFinite(change)?change>=0?'up':'down':''}">${Number.isFinite(change)?`${change>=0?'▲':'▼'} ${Math.abs(change).toFixed(2)}%`:'Change unavailable'}</em><small>${esc(item.quote?.session||'Session unknown')} · ${esc(quoteAge(item.timestamp))}${item.quote?.refresh_error?' · cached':''}${item.quote?.session==='Open'&&item.quote?.stale?' · stale':''}</small></button>`;}).join('')}</div></div>`;
  const track=tape.querySelector<HTMLElement>('.price-ticker-track');if(track){const buttons=track.innerHTML;track.innerHTML=`<div class="ticker-loop">${buttons}</div><div class="ticker-loop" aria-hidden="true">${buttons}</div>`;track.querySelectorAll<HTMLElement>('[aria-hidden] button').forEach(button=>button.tabIndex=-1);track.style.setProperty('--ticker-duration',`${Math.max(35,Math.round(track.scrollWidth/2/45))}s`);const animation=track.getAnimations()[0];if(animation&&previousTime!=null)animation.currentTime=previousTime;}
  tape.querySelectorAll<HTMLElement>('[data-price-symbol]').forEach(button=>button.addEventListener('click',()=>{const item=tickerRows.find(item=>item.symbol===button.dataset.priceSymbol);if(item)void openMarketDetail(item.symbol,item.name||item.symbol);}));
  tape.querySelector('#ticker-pause')?.addEventListener('click',event=>{const paused=tape.classList.toggle('ticker-paused');const button=event.currentTarget as HTMLButtonElement;button.textContent=paused?'Resume ticker':'Pause ticker';button.setAttribute('aria-pressed',String(paused));});
  const live=marketItems.filter(item=>item.price!=null).length;document.querySelector('#market-live')!.textContent=live?`${live}/${marketItems.length} QUOTES`:'UNAVAILABLE';
}

async function showMarketView(view:string){
  document.querySelectorAll<HTMLButtonElement>('[data-market-view]').forEach(button=>button.classList.toggle('active',button.dataset.marketView===view));
  const grid=document.querySelector<HTMLElement>('#market-grid')!,ticker=document.querySelector<HTMLElement>('#ticker-tape')!,browser=document.querySelector<HTMLElement>('#market-browser')!;
  if(view==='indices'){browser.classList.add('hidden');grid.classList.remove('hidden');ticker.classList.remove('hidden');renderMarketIndices();return;}
  grid.classList.add('hidden');ticker.classList.remove('hidden');browser.classList.remove('hidden');
  if(view==='business-world'||view==='business-national'){browser.innerHTML='<p>Fetching business headlines…</p>';try{const result:any=await invoke('news_widget',{category:view,source:'all'});browser.innerHTML=`<h3>${view==='business-world'?'World':'National'} business</h3><div class="company-news-list"></div>`;renderStories(browser.querySelector('.company-news-list')!,result.items||[]);}catch(error){browser.textContent=String(error);}return;}
  if(view==='business-news'){
    browser.innerHTML='<p>Loading business, company and corporate-action news…</p>';
    const exchange=document.querySelector<HTMLSelectElement>('#market-exchange')?.value||'all';
    const sections=await Promise.allSettled([invoke<any>('news_widget',{category:'business-national',source:'all'}),invoke<any>('market_corporate_actions',{exchange})]);
    const items:NewsItem[]=sections.flatMap((r,i)=>r.status==='fulfilled'?(r.value.items||[]).map((row:NewsItem)=>({...row,section:i?'Corporate actions':'Companies & business'})):[]);
    browser.innerHTML='<div class="company-news-list"></div>';paintSectionedStories(browser.querySelector('.company-news-list')!,items);return;
  }
  if(view==='market-news'){
    browser.innerHTML='<p>Loading home and world market headlines…</p>';
    const exchange=document.querySelector<HTMLSelectElement>('#market-exchange')?.value||'all';
    const results=await Promise.allSettled([invoke<any>('market_news',{exchange,source:document.querySelector<HTMLSelectElement>('#market-news-source')?.value||'market'}),invoke<any>('news_widget',{category:'business-world',source:'all'})]);
    const items:NewsItem[]=results.flatMap((r,i)=>r.status==='fulfilled'?(r.value.items||[]).map((row:NewsItem)=>({...row,section:i?'World & other markets':'Home market · latest'})):[]);
    browser.innerHTML='<div class="company-news-list"></div>';paintSectionedStories(browser.querySelector('.company-news-list')!,items);return;
  }
  if(view==='corporate-actions'){
    browser.innerHTML='<p>Loading current-week and current-month corporate actions…</p>';
    try{const exchange=document.querySelector<HTMLSelectElement>('#market-exchange')?.value||'all';const data:any=await invoke('market_corporate_actions',{exchange});const items:NewsItem[]=data.items||[];browser.innerHTML=`<h3>Corporate actions · current week and month</h3><p>${esc(data.scope||'')}</p><div class="company-news-list">${items.map((item,i)=>`<button data-corporate-news="${i}"><b>${esc(item.title)}</b><small>${esc(item.source||item.published||'Exchange notice')}</small></button>`).join('')||'<p>No recent corporate-action item was returned.</p>'}</div>`;renderStories(browser.querySelector('.company-news-list')!,items);}catch(error){browser.innerHTML=`<p>Corporate actions unavailable: ${esc(error)}</p>`;}return;
  }
  const index=selectedMarketIndex||filteredMarkets()[0];
  if(!index){browser.innerHTML='<p>Select an exchange with an available index first.</p>';return;}
  selectedMarketIndex=index;
  browser.innerHTML='<p>Loading index companies…</p>';
  try{
    const data:any=await invoke('market_detail',{symbol:index.symbol});const components:Array<{name:string,symbol:string,industry?:string}>=data.components||[];
    browser.innerHTML=`<h3>${esc(index.name||index.symbol)} components</h3><p>${esc(data.components_note||'')}</p>${data.components_url?'<button id="market-components-source" class="source-btn">Official index page ↗</button>':''}<div class="market-component-list">${components.map(component=>`<button data-company-symbol="${esc(component.symbol)}" data-company-name="${esc(component.name)}"><b>${esc(component.name)}</b><span>${esc(component.symbol)}${component.industry?` · ${esc(component.industry)}`:''}</span></button>`).join('')}</div>`;
    browser.querySelector('#market-components-source')?.addEventListener('click',()=>void invoke('open_external',{url:data.components_url}).catch(error=>pushMessage(String(error))));
    browser.querySelectorAll<HTMLButtonElement>('[data-company-symbol]').forEach(button=>button.addEventListener('click',()=>{selectedMarketCompany={name:button.dataset.companyName||'',symbol:button.dataset.companySymbol||''};void loadCompanyNews(selectedMarketCompany!);}));
    if(view==='company-news'&&components.length){selectedMarketCompany||={name:components[0].name,symbol:components[0].symbol};await loadCompanyNews(selectedMarketCompany);}
  }catch(error){browser.innerHTML=`<p>Index components unavailable: ${esc(error)}</p>`;}
}

async function loadCompanyNews(company:{name:string,symbol:string}){
  const browser=document.querySelector<HTMLElement>('#market-browser')!;browser.innerHTML=`<p>Loading ${esc(company.name)} news and corporate actions…</p>`;
  try{
    const data:any=await invoke('market_company_news',{name:company.name,symbol:company.symbol});const items:NewsItem[]=data.items||[];
    browser.innerHTML=`<h3>${esc(company.name)} · ${esc(company.symbol)}</h3><div class="company-market-actions"><button id="company-quote">Price &amp; chart</button><button id="company-actions">Official corporate actions ↗</button></div><div class="company-news-list">${items.map((item,i)=>`<button data-company-news="${i}"><b>${esc(item.title)}</b><small>${esc(item.source||item.published||'News')}</small></button>`).join('')||'<p>No recent matching company item was returned.</p>'}</div>`;
    browser.querySelector('#company-quote')?.addEventListener('click',()=>void openMarketDetail(company.symbol,company.name));
    browser.querySelector('#company-actions')?.addEventListener('click',()=>void invoke('open_external',{url:data.exchange_url}).catch(error=>pushMessage(String(error))));
    renderStories(browser.querySelector('.company-news-list')!,items);
  }catch(error){browser.innerHTML=`<p>Company news unavailable: ${esc(error)}</p>`;}
}

let stockDetailRequest=0;
async function openMarketDetail(symbol:string,label:string){
 const request=++stockDetailRequest;let chartRequest=0;let providerSymbol=symbol;
 const modal=document.querySelector('#market-modal')!;const content=document.querySelector('#market-detail')!;
 const active=()=>request===stockDetailRequest&&!modal.classList.contains('hidden');
 const number=(value:unknown)=>typeof value==='number'&&Number.isFinite(value)?value.toLocaleString(undefined,{maximumFractionDigits:2}):'—';
 modal.classList.remove('hidden');
 document.querySelector("#market-dialog-title")!.textContent=label;
 content.innerHTML=`<p class="stock-symbol">${esc(symbol)}</p><div id="stock-quote" aria-live="polite">Loading latest available price…</div><div class="stock-range" role="group" aria-label="Chart range">${[['1d','1 day'],['1mo','1 month'],['6mo','6 months'],['1y','1 year'],['5y','5 years']].map(([range,title])=>`<button type="button" class="action-btn ${range==='1d'?'':'ghost'}" data-chart-range="${range}" aria-pressed="${range==='1d'}">${title}</button>`).join('')}</div><div id="stock-chart" aria-live="polite">Loading chart…</div><dl id="stock-stats" class="stock-stats"></dl><section class="stock-section"><h3>Technical indicators</h3><div id="stock-indicators">Loading daily history…</div></section><div id="stock-components"></div><section class="stock-section"><h3>Company news</h3><div id="stock-company-news">Loading recent company coverage…</div></section><section class="stock-section"><h3>Analysts’ views</h3><p>Published analyst and brokerage coverage where available. Headlines do not establish a consensus rating.</p><div id="stock-analyst-news">Looking for published analysis…</div></section><div class="settings-page-actions"><button id="market-yahoo" class="action-btn">Yahoo Finance ↗</button><button id="market-analysis" class="action-btn ghost">Yahoo analyst page ↗</button><button id="market-tradingview" class="action-btn ghost">TradingView chart ↗</button><button id="market-moneycontrol" class="action-btn ghost">Moneycontrol markets ↗</button></div>`;
 const requests=new Map<string,Promise<any>>();
 const detail=(range:string)=>{let task=requests.get(range);if(!task){task=invoke<any>('market_detail',{symbol,range});requests.set(range,task);void task.catch(()=>requests.delete(range));}return task;};
 async function loadChart(range:string){
  const selection=++chartRequest;const chart=content.querySelector('#stock-chart')!;chart.textContent='Loading chart…';
  content.querySelectorAll<HTMLButtonElement>('[data-chart-range]').forEach(button=>{const selected=button.dataset.chartRange===range;button.classList.toggle('ghost',!selected);button.setAttribute('aria-pressed',String(selected));});
  try{
   const data=await detail(range);if(!active()||selection!==chartRequest)return;
   const quote=data.quote||{},meta=data.metadata||{};providerSymbol=quote.symbol||symbol;
   const change=quote.change_percent;const movement=typeof change==='number'&&Number.isFinite(change)?`<span class="${change>=0?'up':'down'}">${change>=0?'+':''}${number(quote.change)} (${change>=0?'+':''}${change.toFixed(2)}%) today</span>`:'Daily movement unavailable';
   content.querySelector('#stock-quote')!.innerHTML=`<p class="market-detail-quote">${esc(quote.currency||meta.currency||'')} ${number(quote.close)} ${movement}</p><p>${esc(quote.session||'Session unknown')} · latest available, may be delayed · ${esc(quoteAge(quote.timestamp))}${quote.refresh_error?' · cached price':''}</p>`;
   const points=pricePoints(data.points),prices=points.map(p=>p[1]),low=Math.min(...prices),high=Math.max(...prices),span=high-low||1;
   const poly=prices.map((price,i)=>`${(i/Math.max(1,prices.length-1)*600).toFixed(1)},${(145-(price-low)/span*125).toFixed(1)}`).join(' ');
   chart.innerHTML=points.length>1?`<div class="stock-chart-labels"><span>High ${number(high)}</span><span>Low ${number(low)}</span></div><svg class="market-chart" viewBox="0 0 600 160" role="img" aria-label="${esc(range)} price chart for ${esc(label)}"><polyline fill="none" stroke="#087e83" stroke-width="3" points="${poly}"/></svg><div class="stock-chart-labels"><span>${new Date(points[0][0]*1000).toLocaleString()}</span><span>${new Date(points.at(-1)![0]*1000).toLocaleString()}</span></div><small>Yahoo Finance · ${esc(data.interval||'')} bars · missing observations omitted</small>`:'No chart observations were returned for this period.';
   content.querySelector('#stock-stats')!.innerHTML=[['Previous close',quote.previous_close],['Day open',meta.regularMarketOpen],['Day high',meta.regularMarketDayHigh],['Day low',meta.regularMarketDayLow],['Volume',meta.regularMarketVolume],['52-week high',meta.fiftyTwoWeekHigh],['52-week low',meta.fiftyTwoWeekLow]].map(([name,value])=>`<div><dt>${esc(name)}</dt><dd>${number(value)}</dd></div>`).join('');
   const companies:Array<{name:string,symbol:string}>=data.components||[];
   const target=content.querySelector('#stock-components')!;target.innerHTML=companies.length?`<section class="stock-section"><h3>Index companies</h3><p>${esc(data.components_note||'')}</p><div class="market-components">${companies.map(company=>`<button data-component="${esc(company.symbol)}">${esc(company.name)} <small>${esc(company.symbol)}</small></button>`).join('')}</div></section>`:'';
   target.querySelectorAll<HTMLButtonElement>('[data-component]').forEach(button=>button.addEventListener('click',()=>void openMarketDetail(button.dataset.component!,button.textContent||button.dataset.component!)));
  }catch(error){if(active()&&selection===chartRequest){chart.textContent=`Chart unavailable: ${String(error)}. Select a period to retry.`;content.querySelector('#stock-quote')!.textContent='Price unavailable. Provider pages below remain available.';}}
 }
 content.querySelectorAll<HTMLButtonElement>('[data-chart-range]').forEach(button=>button.addEventListener('click',()=>void loadChart(button.dataset.chartRange!)));
 void loadChart('1d');
 void detail('1y').then(data=>{
  if(!active())return;const indicators=dailyIndicators(pricePoints(data.history||data.points));
  content.querySelector('#stock-indicators')!.innerHTML=`<dl class="stock-stats"><div><dt>20-day SMA</dt><dd>${number(indicators.sma20)}</dd></div><div><dt>50-day SMA</dt><dd>${number(indicators.sma50)}</dd></div><div><dt>14-day RSI (Wilder)</dt><dd>${number(indicators.rsi)}</dd></div></dl><p>${esc(indicators.trend)}</p><small>Computed from ${indicators.bars} daily adjusted-close bars where available${indicators.asOf?` · through ${new Date(indicators.asOf*1000).toLocaleDateString()}`:''}. The current session may be incomplete. These describe price history; they are not analyst recommendations.</small>`;
 }).catch(()=>{if(active())content.querySelector('#stock-indicators')!.textContent='Daily history unavailable; indicators cannot be calculated.';});
 for(const [command,selector] of [['market_company_news','#stock-company-news'],['market_analyst_news','#stock-analyst-news']]){
  void invoke<any>(command,{name:label.slice(0,120),symbol}).then(data=>{if(!active())return;const target=content.querySelector(selector)!;if(data.items?.length)renderStories(target,data.items);else target.textContent='No recent matching coverage was returned.';}).catch(()=>{if(active())content.querySelector(selector)!.textContent='Coverage is unavailable at present.';});
 }
 const open=(url:string)=>void invoke('open_external',{url}).catch(error=>pushMessage(String(error)));
 content.querySelector('#market-yahoo')?.addEventListener('click',()=>open(`https://finance.yahoo.com/quote/${encodeURIComponent(providerSymbol)}/`));
 content.querySelector('#market-analysis')?.addEventListener('click',()=>open(`https://finance.yahoo.com/quote/${encodeURIComponent(providerSymbol)}/analysis/`));
 content.querySelector('#market-tradingview')?.addEventListener('click',()=>open(`https://www.tradingview.com/chart/?symbol=${encodeURIComponent(tradingViewSymbol(providerSymbol))}`));
 content.querySelector('#market-moneycontrol')?.addEventListener('click',()=>open('https://www.moneycontrol.com/stocksmarketsindia/'));
}

function setupCities() {
  let saved:string[]=[];try{saved=JSON.parse(localStorage.getItem('butler.cleanr5.cities')||'[]');}catch{}
  cityList=[...new Set(saved.map(city=>String(city).trim()).filter(city=>city&&city.toLowerCase()!==profile.location.toLowerCase()))].slice(0,5);
  selectedWeatherCity=localStorage.getItem('butler.cleanr5.weatherCity')||profile.location;
  if(selectedWeatherCity!==profile.location&&!cityList.includes(selectedWeatherCity))selectedWeatherCity=profile.location;
  const renderButtons=()=>{const bar=document.querySelector<HTMLElement>('#weather-city-buttons')!;const cities=[profile.location,...cityList];bar.innerHTML=cities.map(city=>`<button type="button" data-weather-city="${esc(city)}" class="${city===selectedWeatherCity?'active':''}">${city===profile.location?'⌂ ':''}${esc(city)}</button>${city!==profile.location?`<button type="button" class="city-remove" data-weather-remove="${esc(city)}" aria-label="Remove ${esc(city)}">−</button>`:''}`).join('');bar.querySelectorAll<HTMLButtonElement>('[data-weather-city]').forEach(button=>button.addEventListener('click',()=>{selectedWeatherCity=button.dataset.weatherCity||profile.location;localStorage.setItem('butler.cleanr5.weatherCity',selectedWeatherCity);renderButtons();void weather(selectedWeatherCity,selectedWeatherCity===profile.location);}));bar.querySelectorAll<HTMLButtonElement>('[data-weather-remove]').forEach(button=>button.onclick=()=>{const city=button.dataset.weatherRemove;cityList=cityList.filter(item=>item!==city);if(selectedWeatherCity===city)selectedWeatherCity=profile.location;localStorage.setItem('butler.cities',JSON.stringify(cityList));localStorage.setItem('butler.weatherCity',selectedWeatherCity);renderButtons();void weather(selectedWeatherCity,selectedWeatherCity===profile.location);});};
  renderButtons();
  (document.querySelector<HTMLButtonElement>('#add-city')!).onclick=()=>{
    const add=document.querySelector<HTMLButtonElement>('#add-city')!;if(document.querySelector('.weather-city-editor'))return;
    const editor=document.createElement('form');editor.className='weather-city-editor';editor.innerHTML='<label>City<input name="city" required maxlength="100" autocomplete="address-level2"></label><small role="status"></small><div><button type="submit" aria-label="Save city">＋</button><button type="button" aria-label="Cancel city editor">×</button></div>';add.after(editor);editor.querySelector('input')!.focus();editor.querySelector('button[type=button]')!.addEventListener('click',()=>{editor.remove();add.focus();});
    editor.onsubmit=event=>{event.preventDefault();const value=editor.querySelector('input')!.value.trim();if(!value)return;if(cityList.length>=5){editor.querySelector('small')!.textContent='Remove an extra city first (maximum five).';return;}if(value.toLowerCase()!==profile.location.toLowerCase()&&!cityList.some(city=>city.toLowerCase()===value.toLowerCase()))cityList.push(value);localStorage.setItem('butler.cities',JSON.stringify(cityList));selectedWeatherCity=value;localStorage.setItem('butler.weatherCity',value);editor.remove();renderButtons();void weather(value,value===profile.location);};
  };
}

function openScheduleEditor(){
  const modal=document.querySelector('#schedule-modal')!;modal.classList.remove('hidden');
  (document.querySelector<HTMLInputElement>('#schedule-title')!).value='';
  (document.querySelector<HTMLInputElement>('#schedule-date')!).value=localDateKey(selectedCalendarDate);
  (document.querySelector<HTMLInputElement>('#schedule-time')!).value='09:00';
  (document.querySelector<HTMLInputElement>('#schedule-annual')!).checked=false;
  document.querySelector<HTMLInputElement>('#schedule-title')!.focus();
}
async function saveScheduleItem(event:SubmitEvent){
  event.preventDefault();const title=document.querySelector<HTMLInputElement>('#schedule-title')!.value.trim(),date=document.querySelector<HTMLInputElement>('#schedule-date')!.value,time=document.querySelector<HTMLInputElement>('#schedule-time')!.value,annual=document.querySelector<HTMLInputElement>('#schedule-annual')!.checked,lead_days=Number(document.querySelector<HTMLSelectElement>('#schedule-lead')!.value);const error=document.querySelector<HTMLElement>('#schedule-error')!;
  if(!title||!date||!/^(?:[01]\d|2[0-3]):[0-5]\d$/.test(time)){error.textContent='Enter a title, date and valid time.';return;}
  const reminders=Array.isArray(profile.reminders)?[...profile.reminders]:[];reminders.push({id:crypto.randomUUID(),title,date,time,annual,lead_days,enabled:true,contact_id:'',channel:'in_app'});const updated={...profile,reminders};
  try{profile=await invoke<Profile>('profile_save',{profile:updated});document.querySelector('#schedule-modal')!.classList.add('hidden');selectedCalendarDate=new Date(`${date}T12:00:00`);calendarCursor=new Date(selectedCalendarDate.getFullYear(),selectedCalendarDate.getMonth(),1);renderCalendar();renderDaybook();pushMessage(`Added “${title}” to your schedule.`,'butler','Daybook');}catch(reason){error.textContent=`Could not save: ${String(reason)}`;}
}

async function sendChat() {
  const input = document.querySelector<HTMLInputElement>('#chat-input')!; const text = input.value.trim(); if (!text || (responding&&!quietCommand(text)&&quietDuration(text)===null&&!/^(stop|cancel)( the)? (briefing|reading)$/i.test(text))) return; input.value=''; lastInteractionAt=Date.now();recordConversation();cancelListening();pushMessage(text,'user');
  if(quietCommand(text)){quietSpeech();return;}
  if(/^(stop|cancel)( the)? (briefing|reading)$/i.test(text)){document.querySelector<HTMLButtonElement>('#briefing-stop')?.click();return;}
  if(handleQuiet(text))return;
  if(wantsDailyBriefing(text)||(briefingOfferPending&&/^(yes|yes please|sure|please|okay|ok)$/i.test(text))){await runDailyBriefing('Your requested briefing');return;}
  if(briefingOfferPending&&/^(no|no thanks|not now|later)$/i.test(text)){briefingOfferPending=false;pushMessage('Sure. Ask for a daily briefing whenever you want it.','butler');return;}
  responding=true; setButlerState('listen');
  try { const t0=performance.now(); const r:any=await invoke('chat',{input:text}); const response=responseWithAction(r); pushMessage(response,'butler',`${Math.round(performance.now()-t0)} ms`);showPersonaSource(r); document.querySelector('#hero-line')!.textContent=response; if(r.plugin?.plugin_id==='news'&&r.plugin?.data?.items){newsItems=r.plugin.data.items; document.querySelector('#news-live')!.textContent='READY';} setButlerState('happy'); setTimeout(()=>setButlerState('idle'),2500); } catch(e){pushMessage(String(e),'butler'); setButlerState('idle');} finally{responding=false;startListening();}
}

async function nextContextualChatter():Promise<string>{
  const topic=chatterTopic++%14;
  if(topic>=8){const prompts=['Tell me a joke','Tell me a science fact','Encourage me','Find an audiobook','Give me a medical fact','Tell me a thoughtful story'];const result:any=await invoke('chat',{input:prompts[topic-8]});showPersonaSource(result);return responseWithAction(result);}
  if(topic===0){
    if(!localNewsItems.length||!localEventItems.length)await Promise.all([contextualNews('local'),localEvents()]);
    const headline=firstUsefulHeadline(localNewsItems);
    const event=firstUsefulHeadline(localEventItems);
    if(headline&&event)return `An update from ${profile.location}: ${headline}. And for something to do, ${event}. Want me to open either one?`;
    if(headline)return `An update from ${profile.location}: ${headline}. Want me to open the local news?`;
    if(event)return `Something happening around ${profile.location} caught my eye: ${event}. Want the details?`;
  }
  if(topic===1){
    const recent=newsItems.filter(item=>spokenHeadline(item.title)!==localStorage.getItem('butler.cleanr5.lastChatterHeadline'));
    const headline=firstUsefulHeadline(recent);
    if(headline)localStorage.setItem('butler.cleanr5.lastChatterHeadline',headline);
    if(headline)return `${profile.name}, a headline that may interest you: ${headline}. Shall I read more?`;
  }
  if(topic===2){
    if(!nationalNewsItems.length)await contextualNews('national');
    const story=nationalNewsItems.find(item=>/bollywood|film|cinema|actor|actress|movie|music/i.test(`${item.title} ${item.summary||''}`));
    if(story)return `An entertainment update: ${spokenHeadline(story.title)}. Say “tell me more” if that caught your eye.`;
  }
  if(topic===4){if(!marketItems.length)await markets();const headline=firstUsefulHeadline(marketNewsItems);return marketBriefing()+(headline?` In business news, ${headline}. Shall we look at Market Watch?`:'');}
  if(topic===3&&latestWeather&&latestWeather.temperature!==null)return `The weather outlook: ${weatherBriefing()} Would you like the full five-day outlook?`;
  if(topic>=5){const tips=[`I can help organise reminders, ${profile.name}. Please tell me the task and time.`,`You can choose a publisher for each news category, or compare reports under related coverage.`,`Market Watch can display a scrolling tape for your selected exchange. Quote timestamps help you judge how current each price is.`];return tips[topic-5];}
  return await invoke<string>('chatter');
}

async function offerChatter(automatic=false){
  if(isQuiet() || responding || document.hidden || !document.querySelector('#onboarding')?.classList.contains('hidden') || !document.querySelector('#settings-panel')?.classList.contains('hidden'))return;
  if(automatic && (Date.now()-lastInteractionAt>30*60_000 || !relaxationTime() || routineQuiet() || !proactiveChatter || Date.now()-lastInteractionAt<2*60_000 || Date.now()-lastChatterAt<5*60_000))return;
  if(automatic && (!lastVoice?.tts_ready || !['Ready','WakeListening','Stopped'].includes(lastVoice.state) || document.querySelector<HTMLAudioElement>('#audio')?.paused===false))return;
  const speechTicket=speechSession.ticket();responding=true;lastChatterAt=Date.now();
  try{
    let words=await nextContextualChatter();
    if(recentChatter.includes(words)){if(automatic)return;words=await invoke<string>('chatter');}
    recentChatter.push(words);if(recentChatter.length>16)recentChatter.shift();
    if(!speechSession.valid(speechTicket))return;
    pushMessage(words,'butler',automatic?'Companion check-in':'Smart Butler suggestion');
    document.querySelector('#hero-line')!.textContent=words;
    if(lastVoice?.tts_ready && !['Listening','Speaking','Synthesizing'].includes(lastVoice.state)){
      setButlerState('talk');await invoke('voice_speak',{text:words,request:'chatter',capability:'persona.chatter'});
    }
  }catch(error){if(!automatic)pushMessage(`Smart Butler could not start a suggestion: ${String(error)}`);}
  finally{responding=false;setButlerState('idle');}
}

function stopVideoPlayback(){document.querySelector('#media-content')?.replaceChildren();document.querySelector('#media-modal')?.classList.add('hidden');return 'Article closed.';}

function responseWithAction(response: { capability?: string; text?: string; plugin?: {data?: {query?: string;destination?:string}} }): string {
  if (response.capability === 'site.open') void invoke('open_external',{url:response.plugin?.data?.destination||''}).catch(error=>pushMessage(`Could not open that site: ${String(error)}`,'butler'));
  if (response.capability === 'web.search') void invoke('open_external',{url:`https://www.google.com/search?q=${encodeURIComponent(response.plugin?.data?.destination||'')}`}).catch(error=>pushMessage(`Could not open web search: ${String(error)}`,'butler'));
  return response.text || 'I’m here with you.';
}

function isSpokenNews(capability: string | undefined, request: string): boolean {
  return capability === 'news.read_continue' || capability === 'news.read_visible' || (capability === 'news.headlines' && /\b(read|headlines)\b/i.test(request));
}

async function handleVoiceEvent(event: VoiceEvent) {
  idleTimer = 0;
  if(event.kind==='silence'){handleQuiet('quiet');return;}
  if(event.kind==='error'){pushMessage(event.text||'Voice error');return;}
  if(event.kind==='timeout'){setVoiceTop('No clear speech · try Talk again');return;}
  if(event.kind==='barge_in'){briefingCancelled=true;stopNewsReading();briefingCancelled=true;voiceTurn++;setVoiceTop('Interrupted · listening');return;}
  if (event.kind === 'wake') { setButlerState('listen'); setVoiceTop('Listening…'); return; }
  if (event.kind !== 'transcript' || !event.text) return;
  stopNewsReading();lastInteractionAt=Date.now();
  const turn=++voiceTurn;
  pushMessage(event.text,'user',`${event.asr_ms ?? 0} ms ASR`); setVoiceTop('Thinking…'); setButlerState('happy');
  if(quietCommand(event.text)){quietSpeech();return;}
  if(/^(stop|cancel)( the)? (briefing|reading)$/i.test(event.text.trim())){document.querySelector<HTMLButtonElement>('#briefing-stop')?.click();return;}
  if(handleQuiet(event.text))return;
  if(isQuiet()){pushMessage('Quiet mode is active. I can still answer typed messages.','butler');return;}
  if(wantsDailyBriefing(event.text)||(briefingOfferPending&&/^(yes|yes please|sure|please|okay|ok)$/i.test(event.text.trim()))){await runDailyBriefing('Your requested briefing');return;}
  if(briefingOfferPending&&/^(no|no thanks|not now|later)$/i.test(event.text.trim())){briefingOfferPending=false;await sayCompanionLine('Sure. Ask for a daily briefing whenever you want it.','Briefing');return;}
  try {
    recordConversation();cancelListening();const t0=performance.now(); const r:any=await invoke('chat',{input:event.text}); if(turn!==voiceTurn)return; const response=responseWithAction(r); const newsRead=isSpokenNews(r.capability,event.text); if(!newsRead){pushMessage(response,'butler',`${Math.round(performance.now()-t0)} ms response`);showPersonaSource(r);} document.querySelector('#hero-line')!.textContent=newsRead?'Reading the headlines aloud.':response; setButlerState('talk'); setVoiceTop('Speaking…'); await invoke('voice_speak',{text:response,request:event.text,capability:r.capability||''}); void refreshStatus();
  } catch(e){ pushMessage(`Voice response problem: ${String(e)}`,'butler'); setVoiceTop('Voice error'); }
  finally { setTimeout(()=>setButlerState('idle'),900); }
}

function setVoiceTop(label: string){ if(isQuiet())label=quietUntil>8e15?'Quiet · press Speak to resume':`Quiet until ${new Date(quietUntil).toLocaleTimeString()}`; const el=document.querySelector('#voice-top-state'); if(el) el.textContent=label; const summary=document.querySelector('#header-voice-state');if(summary)summary.textContent=label; }
function setupVoiceTuning(){
  const pause=document.querySelector<HTMLInputElement>('#tune-pause')!;
  const limit=document.querySelector<HTMLInputElement>('#tune-limit')!;
  const savedPause=Number(localStorage.getItem('butler.cleanr5.voice.pauseMs'));
  const savedLimit=Number(localStorage.getItem('butler.cleanr5.voice.limitMs'));
  if(savedPause>=400&&savedPause<=1800)pause.value=String(savedPause);
  if(savedLimit>=6000&&savedLimit<=30000)limit.value=String(savedLimit);
  const apply=()=>{const p=Number(pause.value),m=Number(limit.value);document.querySelector('#tune-pause-value')!.textContent=`${(p/1000).toFixed(1)} s`;document.querySelector('#tune-limit-value')!.textContent=`${Math.round(m/1000)} s`;localStorage.setItem('butler.cleanr5.voice.pauseMs',String(p));localStorage.setItem('butler.cleanr5.voice.limitMs',String(m));if(!('__TAURI_INTERNALS__' in window)){setVoiceTop('Desktop preview');return;}void invoke('voice_tune',{pauseToFinishMs:p,maxListeningMs:m}).catch(e=>setVoiceTop(`Timing error: ${e}`));};
  pause.addEventListener('change',apply);limit.addEventListener('change',apply);apply();
}
function showLearning(data:any){
  const report=document.querySelector('#learning-report');
  if(!report)return;
  const news=Number(data?.knowledge_items||0);
  const cues=Number(data?.conversation_items||0);
  const sources=Number(data?.conversation_sources||0);
  const last=data?.last_prefetch?new Date(data.last_prefetch).toLocaleString():'Not run yet';
  report.innerHTML=`<b>Prefetch</b><div>${news} news references · ${cues} conversation cues from ${sources} public sources · ${Number(data?.fresh_stock_items||0)} fresh stock items</div><div>Last run: ${esc(last)}</div><div role="alert">${esc(data?.last_error||'No source error reported')}</div>`;
}
function renderCapture(v:VoiceDiag){
  const parent=document.querySelector('#device-report');
  if(!parent)return;
  let el=document.querySelector<HTMLElement>('#capture-report');
  if(!el){el=document.createElement('div');el.id='capture-report';el.className='device-report';parent.insertAdjacentElement('afterend',el);}
  const rate=Number(v.input_rate||0),frames=Number(v.last_turn_frames||0);
  el.innerHTML=`<b>Microphone capture</b><div>${rate||'—'} Hz · ${v.input_channels||'—'} channel(s) · ${Number(v.input_frames||0).toLocaleString()} frames delivered</div><div>Raw level ${(Number(v.raw_input_level||0)*100).toFixed(2)}% · processed level ${(Number(v.input_level||0)*100).toFixed(2)}%</div><div>Last turn: ${rate?(frames/rate).toFixed(1):'0.0'} s captured · peak ${(Number(v.last_turn_peak||0)*100).toFixed(1)}% · ${v.last_turn_hypotheses||0} ASR hypotheses</div>`;
}
function setupVoiceSignalTest(){
  const report=document.querySelector<HTMLElement>('#voice-test-result')!;
  const playback=document.querySelector<HTMLAudioElement>('#mic-playback')!;
  const save=document.querySelector<HTMLButtonElement>('#mic-save')!;
  const replay=document.querySelector<HTMLButtonElement>('#mic-replay')!;
  const file=document.querySelector<HTMLInputElement>('#voice-wav-file')!;
  let recorded:Uint8Array|null=null;
  let playbackUrl:string|null=null;
  const runWav=async(bytes:Uint8Array)=>{
    if(bytes.byteLength>16*1024*1024){report.textContent='Choose a WAV file no larger than 16 MB.';return;}
    report.textContent='Running WAV through local ASR, Smart Butler and speaker…';
    try{
      const result:any=await invoke('voice_test_wav',{wavBytes:Array.from(bytes)});
      const asr=result.asr||{};
      report.textContent=[
        `WAV: ${asr.sample_rate||'—'} Hz · ${asr.frames||0} frames`,
        `Whisper ASR (raw): ${asr.raw_transcript||'(no words)'}`,
        `Whisper ASR (denoised): ${asr.enhanced_transcript??'(denoiser unavailable)'}`,
        `Accepted: ${asr.accepted?'yes':'no'} · ${asr.asr_ms??result.asr_ms??0} ms`,
        `Smart Butler: ${result.response?.text||'(no response because ASR was not accepted)'}`,
        `Speaker: ${result.spoken?'completed':result.speaker_error||'not run'}`
      ].join('\n');
      void refreshStatus();
    }catch(error){report.textContent=`WAV test failed: ${String(error)}`;}
  };
  document.querySelector('#mic-record')!.addEventListener('click',async()=>{
    report.textContent='Recording five seconds from the Windows microphone. Say “Read the news” now…';
    try{
      const clip:any=await invoke('voice_test_microphone');
      recorded=new Uint8Array(clip.wav_bytes);
      if(playbackUrl)URL.revokeObjectURL(playbackUrl);
      playbackUrl=URL.createObjectURL(new Blob([recorded.buffer as ArrayBuffer],{type:'audio/wav'}));
      playback.src=playbackUrl;playback.hidden=false;save.disabled=false;replay.disabled=false;
      report.textContent=`Captured ${clip.frames} frames at ${clip.sample_rate} Hz from ${clip.device}. Peak ${(Number(clip.peak)*100).toFixed(1)}%. Play the recording to check whether your words sound clear.`;
    }catch(error){report.textContent=`Microphone test failed: ${String(error)}`;}
  });
  save.addEventListener('click',()=>{
    if(!playbackUrl)return;
    const link=document.createElement('a');link.href=playbackUrl;link.download='butler-microphone-test.wav';link.click();
  });
  replay.addEventListener('click',()=>{if(recorded)void runWav(recorded);});
  document.querySelector('#voice-wav-run')!.addEventListener('click',async()=>{
    const selected=file.files?.[0];
    if(!selected){report.textContent='Choose a WAV file first.';return;}
    if(selected.size>16*1024*1024){report.textContent='Choose a WAV file no larger than 16 MB.';return;}
    await runWav(new Uint8Array(await selected.arrayBuffer()));
  });
}
async function pollVoice() { if(pollingVoice)return; pollingVoice=true; try { const events:VoiceEvent[]=await invoke('voice_events'); for(const e of events) void handleVoiceEvent(e); } catch {} finally {pollingVoice=false;} }
async function refreshStatus(){
  if(!('__TAURI_INTERNALS__' in window)){setVoiceTop('Desktop preview');document.querySelector('#header-weather-summary')!.textContent='Desktop preview';document.querySelector('#header-voice-state')!.textContent='Desktop preview';return;}
  try {
    const s:AppStatus=await invoke('status');
    profile=s.profile||profile;
    const headerLocation=document.querySelector('#header-location');if(headerLocation)headerLocation.textContent=profile.location||'Location not set';
    const headerRegion=document.querySelector('#header-region');if(headerRegion)headerRegion.textContent=[profile.region,profile.country].filter(Boolean).join(', ');
    const v=s.voice;
    lastVoice=v;
    renderCapture(v);
    const voicePermitted=!!(profile.onboarding_complete && profile.consents?.microphone && profile.consents?.voice_processing);
    setVoiceTop(!voicePermitted?'Voice off · consent needed':v.always_on?`${v.state} · ${v.wake_mode}`:'Preparing voice…');
    document.querySelector('#footer-log')!.textContent=`Logs: ${s.log_path}`;
    const eng=document.querySelector<HTMLElement>('#engine-pill');
    if(eng){eng.classList.toggle('warn',!v.always_on||!v.asr_ready||!v.tts_ready);eng.title=`${s.build||'Build unknown'} · ${v.always_on?'wake-word listening':'voice runtime initializing or recovering'}`;}
    const mic=document.querySelector<HTMLElement>('#mic-pill');
    if(mic){mic.classList.toggle('bad',!v.microphone);mic.classList.toggle('warn',!!v.microphone&&v.always_on&&!v.microphone_ready);mic.title=v.microphone?`${v.microphone} · ${v.microphone_ready?'capturing':v.always_on?'capture failed':'detected, capture off'}`:'No default microphone detected';}
    const sp=document.querySelector<HTMLElement>('#speaker-pill');
    if(sp){sp.classList.toggle('bad',!v.speaker);sp.classList.toggle('warn',!!v.speaker&&v.always_on&&!v.speaker_ready);sp.title=v.speaker?`${v.speaker} · ${v.speaker_ready?'output initialized':'detected, output not initialized'}`:'No default speaker detected';}
    if(v.state==='Speaking') setButlerState('talk');
    else if(v.state==='Listening'||v.state==='WakeListening') setButlerState('listen');
    const rep=document.querySelector('#device-report');
    if(rep) rep.innerHTML=`<b>Voice diagnostics · ${esc(s.build||'build unknown')}</b><div>🎙 ${esc(v.microphone || 'No default microphone')} · ${v.microphone_ready?'capture running':v.microphone?v.always_on?'capture not ready':'detected; capture off':'device not detected'}</div><div>🔊 ${esc(v.speaker || 'No default speaker')} · ${v.speaker_ready?'output initialized':v.speaker?'detected; output not initialized':'device not detected'}</div><div>VAD ${v.vad_ready?'✓':'—'} · KWS ${v.kws_ready?'✓':'—'} · denoise ${v.denoise_ready?'✓':'—'} · punctuation ${v.punctuation_ready?'✓':'—'}</div><div>Whisper ASR ${v.asr_ready?'✓':'—'} · TTS ${v.tts_ready?'✓':'—'} · barge-ins ${v.barge_in_count??0}</div><div>Runtime: ${esc(v.state)} · capture ${v.always_on?'active':'off'} · mode ${esc(v.wake_mode)}</div><div>Wake detections ${v.wake_count??0} · KWS decode passes ${v.kws_decode_count??0} · last KWS pass ${v.kws_last_decode_ms??0} ms · dropped mic buffers ${v.dropped_audio_buffers??0}</div><div>Input level ${(Number(v.input_level||0)*100).toFixed(1)}% · VAD ${v.vad_active?'speech':'quiet'} · rejected fragments ${v.rejected_fragments??0}</div><div>Recognition runs after a speech pause.</div><div>Last turn: ${esc(v.last_turn_outcome||'None')}</div><div>Last transcript: ${esc(v.last_transcript||'None')} · ${v.last_voice_ms??0} ms</div><div role="alert">${esc(v.last_error||'No runtime error reported')}</div>`;
  }catch(error){const rep=document.querySelector('#device-report');if(rep)rep.textContent=`Status request failed: ${String(error)}`;}
}

function tickClock(){
  idleTimer += 1;
  if(idleTimer > 180) setButlerState('sleep'); else if(idleTimer % 60 === 0) setButlerState('idle');
  const a=document.querySelector('.analog-clock'); if(!a)return;
  const d=new Date();
  let hour=d.getHours(),minute=d.getMinutes(),second=d.getSeconds();
  if(weatherTimezone){
    try{
      const parts=new Intl.DateTimeFormat('en-US',{timeZone:weatherTimezone,hour:'numeric',minute:'numeric',second:'numeric',hourCycle:'h23'}).formatToParts(d);
      const value=(key:string)=>Number(parts.find(p=>p.type===key)?.value||0);
      hour=value('hour');minute=value('minute');second=value('second');
    }catch{weatherTimezone=undefined;}
  }
  const h=(hour%12)/12*360+minute/60*30, m=minute/60*360+second/60*6, s=second/60*360;
  (a.querySelector('.hour') as HTMLElement).style.transform=`rotate(${h}deg)`;
  (a.querySelector('.minute') as HTMLElement).style.transform=`rotate(${m}deg)`;
  (a.querySelector('.second') as HTMLElement).style.transform=`rotate(${s}deg)`;
  if(second===0||document.querySelector('#weather-time')?.textContent==='—'){
    const time=formatCityTime(d.toISOString(),weatherTimezone);
    document.querySelector('#weather-time')!.textContent=time.time;
    document.querySelector('#weather-day')!.textContent=time.day;
    document.querySelector('#weather-date')!.textContent=time.date;
  }
}

async function loadSettingsLogs(){
  const view=document.querySelector<HTMLElement>('#settings-log')!;
  view.textContent='Loading recent log entries…';
  try{view.textContent=await invoke<string>('read_logs');view.scrollTop=view.scrollHeight;}
  catch(error){view.textContent=`Unable to read the log: ${String(error)}`;}
}
function settingsResult(message:string){
  const result=document.querySelector<HTMLElement>('#settings-action-result');
  if(result) result.textContent=message;
}
function renderSettingsProfile(){
  const summary=document.querySelector<HTMLElement>('#settings-profile-summary');
  if(!summary)return;
  const contacts=Array.isArray(profile.contacts)?profile.contacts:[];
  const reminders=Array.isArray(profile.reminders)?profile.reminders:[];
  const consentCount=Object.values(profile.consents||{}).filter(Boolean).length;
  const items=[
    ['Preferred name',profile.name||'Not set'],
    ['Home', [profile.location,profile.region,profile.country].filter(Boolean).join(', ')||'Not set'],
    ['Language',profile.language||'Not set'],
    ['Time zone',profile.timezone||'Not set'],
    ['People',`${contacts.length} saved contact${contacts.length===1?'':'s'}`],
    ['Important dates',`${reminders.length} reminder${reminders.length===1?'':'s'}`],
    ['Privacy choices',`${consentCount} enabled`]
  ];
  summary.innerHTML=items.map(([label,value])=>`<div class="settings-summary-row"><span>${esc(label)}</span><strong>${esc(value)}</strong></div>`).join('');
}
function showSettingsView(view:string){
  document.querySelectorAll<HTMLElement>('[data-settings-view]').forEach(button=>button.classList.toggle('active',button.dataset.settingsView===view));
  document.querySelectorAll<HTMLElement>('[data-settings-page]').forEach(page=>page.classList.toggle('hidden',page.dataset.settingsPage!==view));
  if(view==='logs')void loadSettingsLogs();
  if(view==='status')void refreshStatus();
  if(view==='learning')void invoke('learning_status').then(showLearning).catch(()=>{});
  if(view==='assistant'){void loadLlmSettings();void loadFreeModels();void loadLlmUsage();}
  if(view==='voice')void loadPiperVoices();
  if(view==='profile')renderSettingsProfile();
}

async function loadPiperVoices(){
  const report=document.querySelector<HTMLElement>('#piper-voice-report')!;
  try{
    const voices:any=await invoke('piper_voices');const list=voices.installed||[];
    const select=document.querySelector<HTMLSelectElement>('#piper-voice')!;
    select.innerHTML=list.map((voice:any)=>`<option value="${esc(voice.id)}">${esc(voice.name)} · ${esc(voice.language)}</option>`).join('');
    select.value=voices.selected||'';
    document.querySelector<HTMLInputElement>('#piper-auto')!.checked=!!voices.auto;
    report.textContent=`${list.length} complete Piper voice package${list.length===1?'':'s'} installed. ${voices.auto?'Automatic voice and pace':'Fixed voice'}; current: ${list.find((voice:any)=>voice.id===voices.selected)?.name||'none'}.`;
    const catalog=document.querySelector<HTMLElement>('#piper-voice-catalog')!;
    catalog.innerHTML=(voices.available||[]).map((voice:any)=>`<div class="voice-offer"><div><strong>${esc(voice.name)}</strong><small>${esc(voice.gender)} · ${esc(voice.language)} · ${esc(voice.dataset_license)}</small></div><button type="button" data-voice-install="${esc(voice.id)}" ${voice.installed?'disabled':''}>${voice.installed?'Ready':'Download'}</button></div>`).join('');
  }catch(error){report.textContent=`Could not list Piper voices: ${String(error)}`;}
}

let llmLatestStatus:any=null;
function updateLocalChoice(){
  const model=document.querySelector<HTMLSelectElement>('#llm-local-model')?.value;
  const localOption=document.querySelector<HTMLOptionElement>('#llm-provider option[value="local"]');
  if(localOption)localOption.disabled=!localChoiceUsable(model||'',llmLatestStatus);const prepare=document.querySelector<HTMLButtonElement>('#llm-prepare-local');if(prepare){prepare.disabled=!model||localServiceModel(model)||prepare.dataset.runtimeAvailable==='false';prepare.textContent=prepare.dataset.runtimeAvailable==='false'?'Local generative runtime unavailable':localPreparationLabel(model||'');}
}
async function loadLlmSettings(syncFields=true){
  const report=document.querySelector<HTMLElement>('#llm-report')!;
  try{
    const status:any=await invoke('llm_status');const s=status.settings||{};
    llmLatestStatus=status;
    if(syncFields){
      (document.querySelector('#llm-provider') as HTMLSelectElement).value=s.provider||'auto';
      document.querySelector<HTMLInputElement>('#llm-online-enabled')!.checked=!!s.online_enabled;
      const localSelect=document.querySelector<HTMLSelectElement>('#llm-local-model')!;if(s.local_model&&![...localSelect.options].some(o=>o.value===s.local_model))localSelect.add(new Option(`${s.local_model} · previous choice`,s.local_model));localSelect.value=s.local_model||localSelect.options[0]?.value||'';
      (document.querySelector('#llm-online-provider') as HTMLSelectElement).value=s.online_provider||'groq';
      (document.querySelector('#llm-online-model') as HTMLInputElement).value=s.online_model||'';
      document.querySelectorAll<HTMLElement>('[data-provider-row]').forEach(row=>{
        const provider=row.dataset.providerRow;
        const select=row.querySelector<HTMLSelectElement>('select');
        const fixed=row.dataset.fixedModel;
        if(provider===s.online_provider&&!fixed&&select&&s.online_model){if(![...select.options].some(o=>o.value===s.online_model))select.add(new Option(s.online_model+' · saved choice',s.online_model));select.value=s.online_model;}
        const matches=provider===s.online_provider&&(fixed?fixed===s.online_model:select?.value===s.online_model);
        const radio=row.querySelector<HTMLInputElement>('input[type="radio"]');if(radio)radio.checked=!!matches;
      });
    }
    updateLocalChoice();
    const configured=Object.entries(status.provider_keys||{}).filter(([,ready])=>ready).map(([name])=>name);
    const models=(status.local_models||[]).map((model:any)=>model.id).join(', ')||'none';
    report.textContent=`${localServiceModel(s.local_model||'')?'Selected service: '+s.local_model.split(':')[0]+' (start server to use). ':''}Local GGUF runtime: ${status.local_runtime_ready?'found':'missing'} · reusable models: ${models}. ${status.local_preparation_status||''} Credential vault: ${status.vault_unlocked?'unlocked':status.vault_exists?'locked':'not created'} · Available provider keys: ${configured.join(', ')||'none'}.`;
  }catch(error){report.textContent=`Could not read model status: ${String(error)}`;}
}

async function loadFreeModels(){
  const status=document.querySelector<HTMLElement>('#llm-free-model-status')!;
  const select=document.querySelector<HTMLSelectElement>('#llm-free-model')!;
  status.textContent='Checking current free text models…';
  try{
    const models:any[]=await invoke('llm_free_models');
    const shown=models.slice(0,60);
    select.innerHTML='<option value="openrouter/free">Free model router</option>'+shown.map(model=>`<option value="${esc(model.id)}">${esc(model.name)}</option>`).join('');
    const current=document.querySelector<HTMLInputElement>('#llm-online-model')!.value;
    select.value=Array.from(select.options).some(option=>option.value===current)?current:'openrouter/free';
    status.textContent=`${models.length} currently verified free text models; ${shown.length} listed. These share one OpenRouter key. If fewer than 15 are available, Smart Butler shows the actual count.`;
  }catch(error){status.textContent=`Live free-model list unavailable: ${String(error)}. Check the provider page before choosing a model.`;}
}

async function loadLlmUsage(){
  const report=document.querySelector<HTMLElement>('#llm-usage-report')!;
  report.textContent='Checking provider usage…';
  try{
    const usage:any=await invoke('llm_usage');
    const openrouter=usage.openrouter||{};
    report.textContent=openrouter.status==='available'
      ?`OpenRouter: ${openrouter.is_free_tier?'free-tier key':'account key'} · daily usage ${openrouter.usage_daily??'unreported'} · monthly usage ${openrouter.usage_monthly??'unreported'} · key limit remaining ${openrouter.limit_remaining??'not set'}. Verify current quotas in your provider account.`
      :`OpenRouter: ${openrouter.status||'usage unavailable'}`;
  }catch(error){report.textContent=`Usage check failed: ${String(error)}`;}
}

const llmProviderHelp:Record<string,{keys:string,limits:string,model:string}>=Object.fromEntries(onlineProviders.map(p=>[p.id,{keys:p.url,limits:p.guide,model:p.model}]));

let entranceTimer:ReturnType<typeof setTimeout>|undefined;
function revealButler(workspace:string){
 const stage=document.querySelector<HTMLElement>('#butler-entrance'),image=document.querySelector<HTMLImageElement>('#entrance-pose'),caption=document.querySelector<HTMLElement>('#entrance-caption');if(!stage||!image||!caption)return;
 const poses:Record<string,[string,string]>={chatter:['welcome','At your service. Naturally.'],news:['news','The world, without the noise.'],market:['market','Figures first. Drama optional.'],weather:['resting','Even butlers check the forecast.'],calendar:['listening','Consider your day attended to.']};
 const [pose,line]=poses[workspace]||poses.chatter;image.src=`/assets/butler-${pose}.png`;caption.textContent=line;
 clearTimeout(entranceTimer);stage.classList.remove('is-entering');void stage.offsetWidth;stage.classList.add('is-entering');entranceTimer=setTimeout(()=>stage.classList.remove('is-entering'),3200);
}
function setup(){
  const showDesktopPanel=(name:string)=>{
    stopNewsReading();
    revealButler(name);
    document.querySelectorAll<HTMLElement>('.dashboard>.panel').forEach(panel=>panel.classList.toggle('desktop-active',panel.classList.contains(`${name}-panel`)));
    document.querySelectorAll<HTMLButtonElement>('.desktop-widget-nav [data-desktop-panel]').forEach(button=>{const active=button.dataset.desktopPanel===name;button.classList.toggle('active',active);button.setAttribute('aria-selected',String(active));button.tabIndex=active?0:-1;if(active)button.setAttribute('aria-current','page');else button.removeAttribute('aria-current');});
    const mascot=document.querySelector<HTMLImageElement>('#butler-mascot img');if(mascot){mascot.src=`/assets/butler-${name==='news'?'news':name==='market'?'market':'welcome'}.png`;mascot.alt=`Your orangutan butler presenting ${name==='chatter'?'chat':name}`;}
    const selected=document.querySelector<HTMLElement>(`.dashboard>.${name}-panel`);
    if(selected){selected.classList.remove('collapsed');selected.querySelector<HTMLButtonElement>('[data-toggle-panel]')?.setAttribute('aria-expanded','true');selected.tabIndex=0;}
    if(name==='news')void news(activeNews,false,true);
    if(name==='market')void markets();
  };
  document.querySelectorAll<HTMLButtonElement>('.desktop-widget-nav [data-desktop-panel]').forEach(button=>button.addEventListener('click',()=>showDesktopPanel(button.dataset.desktopPanel||'chatter')));
  document.querySelector('.desktop-widget-nav')!.addEventListener('keydown',event=>{const key=event as KeyboardEvent;if(!['ArrowLeft','ArrowRight','Home','End'].includes(key.key))return;const tabs=Array.from(document.querySelectorAll<HTMLButtonElement>('.desktop-widget-nav [role="tab"]'));const current=tabs.indexOf(key.target as HTMLButtonElement);if(current<0)return;key.preventDefault();const next=key.key==='Home'?0:key.key==='End'?tabs.length-1:(current+(key.key==='ArrowRight'?1:-1)+tabs.length)%tabs.length;tabs[next].click();tabs[next].focus();});

  document.querySelectorAll<HTMLElement>('.dashboard>.panel:not(.chatter-panel)').forEach(panel=>{const tools=panel.querySelector<HTMLElement>('.panel-tools');if(!tools)return;const close=document.createElement('button');close.className='widget-close';close.type='button';close.title='Close widget and return to Smart Butler';close.setAttribute('aria-label','Close widget and return to Smart Butler');close.textContent='×';close.addEventListener('click',()=>{showDesktopPanel('chatter');});tools.append(close);});
  document.querySelectorAll<HTMLButtonElement>('.mobile-nav button').forEach(button=>button.addEventListener('click',()=>{
    const panel=button.dataset.mobilePanel;
    document.querySelectorAll('.dashboard .panel').forEach(card=>card.classList.toggle('mobile-active',card.classList.contains(`${panel}-panel`)));
    document.querySelectorAll<HTMLButtonElement>('.mobile-nav button').forEach(tab=>{
      const active=tab===button;tab.classList.toggle('active',active);
      if(active)tab.setAttribute('aria-current','page');else tab.removeAttribute('aria-current');
    });
  }));
  void invoke('learning_status').then(showLearning).catch(()=>{});
  document.querySelector('#settings')!.addEventListener('click',()=>void invoke('learning_status').then(showLearning).catch(()=>{}));
  setupCities();
  renderCalendar();renderDaybook();
  document.querySelector('#daybook-add')!.addEventListener('click',openScheduleEditor);
  document.querySelector('#schedule-close')!.addEventListener('click',()=>document.querySelector('#schedule-modal')!.classList.add('hidden'));
  document.querySelector<HTMLFormElement>('#schedule-form')!.addEventListener('submit',event=>void saveScheduleItem(event));
  document.querySelector('#calendar-toggle')!.addEventListener('click',event=>{const button=event.currentTarget as HTMLButtonElement;const month=document.querySelector('#calendar-month')!;const open=month.classList.toggle('hidden')===false;button.setAttribute('aria-expanded',String(open));button.textContent=open?'▴':'▾';if(open){renderCalendar();void loadCalendarContext(selectedCalendarDate);}});
  document.querySelector('#calendar-prev')!.addEventListener('click',()=>{calendarCursor=new Date(calendarCursor.getFullYear(),calendarCursor.getMonth()-1,1);renderCalendar();});
  document.querySelector('#calendar-next')!.addEventListener('click',()=>{calendarCursor=new Date(calendarCursor.getFullYear(),calendarCursor.getMonth()+1,1);renderCalendar();});
  document.querySelector('#briefing-now')!.addEventListener('click',()=>void requestDailyBriefing());

  document.querySelectorAll<HTMLButtonElement>('[data-toggle-panel]').forEach(button=>button.addEventListener('click',()=>{const panel=button.closest<HTMLElement>('.collapsible-panel');if(!panel)return;const open=panel.classList.toggle('collapsed')===false;button.setAttribute('aria-expanded',String(open));button.textContent=open?'▴':'▾';}));
  document.querySelector('.market-tabs')!.addEventListener('click',event=>{const button=(event.target as HTMLElement).closest<HTMLButtonElement>('[data-market-view]');if(button)void showMarketView(button.dataset.marketView||'indices');});
  document.querySelector('#market-exchange')!.addEventListener('change',()=>{const sourceSelect=document.querySelector<HTMLSelectElement>('#market-news-source');if(sourceSelect)sourceSelect.innerHTML=marketSourceOptions();selectedMarketIndex=null;selectedMarketCompany=null;void markets();const view=document.querySelector<HTMLButtonElement>('[data-market-view].active')?.dataset.marketView||'indices';void showMarketView(view);});
  document.querySelector('#news-tabs')!.addEventListener('click',e=>{const b=(e.target as HTMLElement).closest('button'); if(b?.dataset.cat) news(b.dataset.cat,false,true);});
  document.querySelector('#news-refresh')!.addEventListener('click',()=>news(activeNews,true)); document.querySelector('#weather-refresh')!.addEventListener('click',()=>void weather()); document.querySelector('#market-refresh')!.addEventListener('click',markets);
  document.querySelector('#talk')!.addEventListener('click',async()=>{if(isQuiet())handleQuiet('resume speaking');stopNewsReading();recordConversation();cancelListening();voiceTurn++;try{await invoke('voice_arm'); setButlerState('listen'); setVoiceTop('Listening…');}catch(e){pushMessage(String(e),'butler');}});
  document.querySelector('#send')!.addEventListener('click',sendChat); document.querySelector<HTMLInputElement>('#chat-input')!.addEventListener('keydown',(e)=>{if(e.key==='Enter')sendChat();});
  document.querySelectorAll<HTMLButtonElement>('[data-persona]').forEach(button=>button.addEventListener('click',()=>{document.querySelector<HTMLInputElement>('#chat-input')!.value=button.dataset.persona||'';void sendChat();}));

  document.querySelector<HTMLInputElement>('#proactive-chatter')!.addEventListener('change',event=>{proactiveChatter=(event.target as HTMLInputElement).checked;localStorage.setItem('butler.cleanr5.proactiveChatter',proactiveChatter?'on':'off');lastChatterAt=Date.now();});
  document.querySelector('#settings')!.addEventListener('click',()=>{document.querySelector('#settings-panel')!.classList.remove('hidden');showSettingsView('profile');}); document.querySelector('#settings-close')!.addEventListener('click',()=>document.querySelector('#settings-panel')!.classList.add('hidden')); document.querySelector('#media-close')!.addEventListener('click',stopVideoPlayback);
  document.querySelector('.settings-nav')!.addEventListener('click',event=>{const button=(event.target as HTMLElement).closest<HTMLButtonElement>('[data-settings-view]');if(button?.dataset.settingsView)showSettingsView(button.dataset.settingsView);});
  document.querySelector('#refresh-logs')!.addEventListener('click',()=>void loadSettingsLogs());
  document.querySelector('#piper-select')!.addEventListener('click',async()=>{
    const id=document.querySelector<HTMLSelectElement>('#piper-voice')!.value;
    try{await invoke('piper_select_voice',{id});settingsResult('Piper voice selected. Use Test speaker to hear it.');void loadPiperVoices();}
    catch(error){settingsResult(`Could not select Piper voice: ${String(error)}`);}
  });
  document.querySelector<HTMLInputElement>('#piper-auto')!.addEventListener('change',async event=>{
    const enabled=(event.target as HTMLInputElement).checked;
    try{await invoke('piper_set_auto',{enabled});settingsResult(enabled?'Adaptive pacing enabled.':'Fixed voice enabled.');await loadPiperVoices();}
    catch(error){settingsResult(`Voice mode could not be saved: ${String(error)}`);await loadPiperVoices();}
  });
  document.querySelector('#piper-voice-catalog')!.addEventListener('click',async event=>{
    const button=(event.target as HTMLElement).closest<HTMLButtonElement>('[data-voice-install]');if(!button)return;
    const id=button.dataset.voiceInstall!;button.disabled=true;settingsResult(`Preparing ${id}; checking the verified cache first…`);
    try{await invoke('piper_prepare_voice',{id});settingsResult(`${id} is installed. Choose it from Ready to use, then test the speaker.`);await loadPiperVoices();}
    catch(error){button.disabled=false;settingsResult(`Voice package could not be prepared: ${String(error)}`);}
  });
  document.querySelector('#llm-save')!.addEventListener('click',async()=>{
    const value=(id:string)=>(document.querySelector(id) as HTMLInputElement|HTMLSelectElement).value.trim();
    const settings={provider:value('#llm-provider'),online_enabled:value('#llm-provider')==='online'||(value('#llm-provider')==='auto'&&document.querySelector<HTMLInputElement>('#llm-online-enabled')!.checked),online_provider:value('#llm-online-provider'),online_model:value('#llm-online-model'),local_model:value('#llm-local-model')};
    try{
      await invoke('llm_configure',{settings,apiKey:null});
      settingsResult(`Conversation setup saved: ${settings.provider}.`);
      void loadLlmSettings();
    }catch(error){settingsResult(`Model choice was not saved: ${String(error)}`);}
  });
  document.querySelector('#llm-prepare-local')!.addEventListener('click',async()=>{
    const button=document.querySelector<HTMLButtonElement>('#llm-prepare-local')!;
    const model=document.querySelector<HTMLSelectElement>('#llm-local-model')!.value;
    button.disabled=true;settingsResult(`Preparing ${model}; checking reusable files first…`);
    const polling=setInterval(()=>void loadLlmSettings(false),2000);
    try{await invoke('llm_prepare_local',{model});settingsResult(`${model} passed a local conversation test. You can now choose Automatic or Local.`);}
    catch(error){settingsResult(`Local model preparation failed: ${String(error)}`);}
    finally{clearInterval(polling);button.disabled=false;void loadLlmSettings(false);}
  });
  document.querySelector('#llm-unlock')!.addEventListener('click',async()=>{
    const field=document.querySelector<HTMLInputElement>('#llm-vault-password')!;
    try{await invoke('llm_unlock',{password:field.value});field.value='';settingsResult('Credential vault unlocked. Add a key and save its provider.');void loadLlmSettings();}
    catch(error){settingsResult(`Could not unlock credential vault: ${String(error)}`);}
  });
  document.querySelector('#llm-remove-key')!.addEventListener('click',async()=>{
    const provider=(document.querySelector<HTMLSelectElement>('#llm-online-provider')!).value;
    try{await invoke('llm_remove_key',{provider});settingsResult(`Removed the saved ${provider} key.`);void loadLlmSettings();}
    catch(error){settingsResult(`Could not remove provider key: ${String(error)}`);}
  });
  document.querySelector('#llm-online-provider')!.addEventListener('change',()=>{
    const provider=(document.querySelector<HTMLSelectElement>('#llm-online-provider')!).value;
    (document.querySelector<HTMLInputElement>('#llm-online-model')!).value=llmProviderHelp[provider].model;
  });
  document.querySelector('#llm-free-model')!.addEventListener('change',()=>{
    const selected=document.querySelector<HTMLSelectElement>('#llm-free-model')!.value;
    document.querySelector<HTMLSelectElement>('#llm-online-provider')!.value='openrouter';
    document.querySelector<HTMLInputElement>('#llm-online-model')!.value=selected;
  });
  document.querySelector('#llm-refresh-free')!.addEventListener('click',()=>void loadFreeModels());
  document.querySelector('#llm-refresh-usage')!.addEventListener('click',()=>void loadLlmUsage());
  document.querySelectorAll<HTMLElement>('[data-provider-row]').forEach(row=>{
    const activate=()=>{
      const provider=row.dataset.providerRow!;
      const model=row.dataset.fixedModel||row.querySelector<HTMLSelectElement>('select')?.value||llmProviderHelp[provider].model;
      document.querySelector<HTMLSelectElement>('#llm-online-provider')!.value=provider;
      document.querySelector<HTMLInputElement>('#llm-online-model')!.value=model;
      const radio=row.querySelector<HTMLInputElement>('input[type="radio"]');if(radio)radio.checked=true;
      settingsResult(`${provider} · ${model} selected. Use Save choices to make it active.`);
    };
    row.querySelector<HTMLInputElement>('input[type="radio"]')?.addEventListener('change',activate);
    row.querySelector<HTMLSelectElement>('select')?.addEventListener('change',activate);
    row.querySelector<HTMLButtonElement>('.llm-row-key')?.addEventListener('click',async()=>{
      const input=row.querySelector<HTMLInputElement>('.row-api-key')!;
      try{await invoke('llm_store_key',{provider:row.dataset.providerRow,apiKey:input.value});input.value='';activate();settingsResult(`${row.dataset.providerRow} key encrypted and saved. Save choices to make this model active.`);void loadLlmSettings(false);}
      catch(error){settingsResult(`Key was not saved: ${String(error)}`);}
    });
  });
  document.querySelectorAll<HTMLButtonElement>('.llm-key-save').forEach(button=>button.addEventListener('click',async()=>{
    const provider=button.dataset.keyProvider!;const input=document.querySelector<HTMLInputElement>(`#${button.dataset.keyInput}`)!;
    try{await invoke('llm_store_key',{provider,apiKey:input.value});input.value='';settingsResult(`${provider} token encrypted and saved.`);void loadLlmSettings(false);}
    catch(error){settingsResult(`Token was not saved: ${String(error)}`);}
  }));
  document.querySelector('#profile-ai-keys')!.addEventListener('click',()=>showSettingsView('assistant'));
  document.querySelector('#llm-local-model')!.addEventListener('change',updateLocalChoice);
  document.querySelectorAll<HTMLButtonElement>('.llm-guide,.llm-direct-guide').forEach(button=>button.addEventListener('click',()=>{
    void invoke('open_external',{url:button.dataset.url}).catch(error=>settingsResult(`Could not open model guide: ${String(error)}`));
  }));
  document.querySelector('#open-log-folder')!.addEventListener('click',()=>void invoke('open_logs').then(()=>settingsResult('Opened the Smart Butler log folder.')).catch(error=>settingsResult(`Could not open log folder: ${String(error)}`)));
  document.querySelector('#market-close')!.addEventListener('click',()=>{stockDetailRequest++;document.querySelector('#market-modal')!.classList.add('hidden');});
  document.querySelector('#pronunciation-save')!.addEventListener('click',async()=>{
    const term=document.querySelector<HTMLInputElement>('#pronunciation-term')!.value,pronunciation=document.querySelector<HTMLInputElement>('#pronunciation-spoken')!.value,status=document.querySelector<HTMLElement>('#pronunciation-status')!;
    try{await invoke('pronunciation_set',{term,pronunciation});status.textContent=pronunciation.trim()?'Pronunciation saved. Ask Smart Butler to say this word to hear it.':'Saved pronunciation removed.';}catch(error){status.textContent=String(error);}
  });
  document.querySelector('#speaker-test')!.addEventListener('click',async()=>{settingsResult('Sending a test phrase to the selected speaker…');setVoiceTop('Speaking…');setButlerState('talk');try{await invoke('voice_speak',{text:`Hello ${profile.name}. This is Smart Butler. Please confirm that you heard this speaker test.`});settingsResult('Playback command completed. Confirm by listening; this cannot prove the sound was audible.');}catch(e){settingsResult(`Speaker test failed: ${String(e)}`);}finally{void refreshStatus();setButlerState('idle');}});
  document.querySelector('#prefetch')!.addEventListener('click',async()=>{settingsResult('Refreshing conversation sources and live information…');try{const r:any=await invoke('prefetch_now');document.querySelector('#footer-status')!.textContent=r.last_error?`Prefetch: ${r.last_error}`:'Background learning refreshed';await Promise.allSettled([weather(),news(),markets()]);showSettingsView('learning');settingsResult(r.last_error?`Prefetch reported: ${r.last_error}`:'Prefetch completed; weather, news, markets were asked to refresh. Check each widget for its live status.');}catch(e){settingsResult(`Prefetch failed: ${String(e)}`);}});
  document.querySelector('#edit-profile')!.addEventListener('click',()=>{document.querySelector('#settings-panel')!.classList.add('hidden');void companion.open(true);});
}

render(); setup(); setupVoiceTuning(); setupVoiceSignalTest(); setupDisplay();
companion=setupCompanion(()=>profile,p=>{profile=p;},pushMessage,async()=>{
  document.querySelector('#greeting')!.textContent='Hello, '+profile.name+'.';
  renderSettingsProfile();
  const select=document.querySelector<HTMLSelectElement>('#city-select')!;
  if(!Array.from(select.options).some(o=>o.value===profile.location)){const o=document.createElement('option');o.value=o.textContent=profile.location;select.add(o);}select.value=profile.location;
  renderCalendar();renderDaybook();
  setupCities();if(launchBriefingStarted)void Promise.allSettled([refreshHomeWeather(),weather(),news(),markets()]);else void launchBriefing();
});
setInterval(tickClock,1000);setInterval(refreshStatus,1500);setInterval(pollVoice,100);
setInterval(()=>{if(!document.hidden){void refreshHomeWeather();void weather();void news();void markets();}},180000);
let lastPriceRefresh=0;
setInterval(()=>{if(!document.hidden&&Date.now()-lastPriceRefresh>=priceRefreshMinutes()*60000){lastPriceRefresh=Date.now();void markets();}},15000);
setInterval(()=>companion.checkReminders(),30000);
setInterval(()=>void offerChatter(true),60_000);
// Briefings are manual.
document.addEventListener('pointerdown',()=>{lastInteractionAt=Date.now();});
document.addEventListener('keydown',()=>{lastInteractionAt=Date.now();});
document.addEventListener('keydown',e=>{if(e.key==='Escape'){if(watchlistPopoverId)return;const mediaOpen=!document.querySelector('#media-modal')?.classList.contains('hidden');const openModal=Array.from(document.querySelectorAll('.modal')).some(el=>!el.classList.contains('hidden'));if(mediaOpen)stopVideoPlayback();document.querySelectorAll('.modal').forEach(el=>el.classList.add('hidden'));if(!openModal){document.querySelector<HTMLButtonElement>('[data-desktop-panel="chatter"]')?.click();}}});
(async()=>{
  const minimumSplash=new Promise(resolve=>setTimeout(resolve,3500));
  await refreshStatus();
  if(profile.onboarding_complete)launchContextPromise=Promise.all([refreshHomeWeather(),weather(),news('breaking'),markets(),contextualNews('local'),contextualNews('regional'),contextualNews('national'),localEvents(),loadCalendarContext(new Date())]);
  const message=document.querySelector<HTMLElement>('#launch-message');
  if(message)message.textContent=`Welcome, ${profile.name}. I’m tuning my voice and checking ${profile.location}.`;
  launchVoiceReady=(profile.onboarding_complete && profile.consents?.microphone && profile.consents?.voice_processing)
    ?await waitForSpeechReady(12_000):false;
  launchWarmupCompleted=true;
  if(message)message.textContent=launchVoiceReady?'Voice ready—let’s begin.':'I’m opening now and will keep preparing voice in the background.';
  await minimumSplash;
  hideLaunchSplash();
  await new Promise(resolve=>setTimeout(resolve,650));
  if('__TAURI_INTERNALS__' in window)await companion.open();
  else pushMessage('Welcome to the Smart Butler layout preview. Live news, quotes and voice are available in the installed desktop app.','butler');
  companion.checkReminders();
  if(document.querySelector('#onboarding')?.classList.contains('hidden'))await launchBriefing();
})();

document.querySelector('#news-source')?.addEventListener('change',event=>{localStorage.setItem(`butler.cleanr5.curatedNewsSource.${profile.country}.${profile.region}.${profile.location}.${activeNews}`,(event.target as HTMLSelectElement).value);void news(activeNews,false,true);});

void nativeInvoke('voice_quiet_until',{until:quietUntil}).catch(()=>{});


document.querySelector('#market-news-source')?.addEventListener('change',()=>void markets());


document.querySelector('#quiet-start')?.addEventListener('click',()=>{const input=document.querySelector<HTMLInputElement>('#quiet-minutes')!;if(input.reportValidity())handleQuiet(`keep quiet for ${input.value} minutes`);});
document.querySelector('#quiet-end')?.addEventListener('click',()=>{handleQuiet('resume speaking');setVoiceTop('Ready');});

document.querySelector('#briefing-stop')?.addEventListener('click',quietSpeech);

let listeningTimer:ReturnType<typeof setTimeout>|undefined;
let listeningGeneration=0;
function routine(){return profile.conversation_routine||{};}
function localClock(){const time=currentDayAndTime(new Date(),profile.timezone||Intl.DateTimeFormat().resolvedOptions().timeZone);return time.time.slice(0,5);}
function routineQuiet(){return quietAt(routine(),localClock());}
function relaxationTime(){const r=routine();return inTimeWindow(localClock(),r.relax_start||'18:00',r.relax_end||'21:00');}
function cancelListening(){clearTimeout(listeningTimer);listeningGeneration++;void invoke('voice_follow_up',{milliseconds:0}).catch(()=>{});}
function startListening(){
 cancelListening();if(isQuiet()||routineQuiet()||!profile.consents?.microphone||!profile.consents?.voice_processing||!lastVoice?.always_on)return;
 const generation=listeningGeneration;let attempt=0;
 const open=async()=>{
  if(generation!==listeningGeneration||isQuiet()||routineQuiet())return;
  await refreshStatus();
  if(generation!==listeningGeneration)return;
  if(responding||['Speaking','Synthesizing'].includes(lastVoice?.state||'')){listeningTimer=setTimeout(()=>void open(),500);return;}
  if(lastVoice?.state==='Listening')return;
  try{await invoke('voice_follow_up',{milliseconds:6000});}catch{return;}
  const gap=listeningGaps[attempt++];
  if(gap===undefined){listeningTimer=setTimeout(()=>{if(generation===listeningGeneration)document.querySelector('#briefing-progress')!.textContent='I am on alert. Press Speak or say Hey Butler when you need me.';},6000);return;}
  const counts=routine().hour_counts||[];const hour=Number(localClock().slice(0,2));const expected=Number(counts[hour]||0)>=3;
  listeningTimer=setTimeout(()=>void open(),6000+gap*(attempt===1||expected?1:2));
 };
 void open();
}
function recordConversation(){
 if(!profile.consents?.personalization||!profile.consents?.profile_storage)return;
 const hour=Number(localClock().slice(0,2));
 void invoke('conversation_interaction',{hour}).then(routine=>{profile.conversation_routine=routine;}).catch(()=>{});
}
function syncRoutineFields(){const r=routine();for(const [id,value] of Object.entries({'routine-quiet-start':r.quiet_start||'13:00','routine-quiet-end':r.quiet_end||'15:00','routine-relax-start':r.relax_start||'18:00','routine-relax-end':r.relax_end||'21:00'})){const input=document.getElementById(id) as HTMLInputElement|null;if(input)input.value=String(value);}const enabled=document.getElementById('routine-quiet-enabled') as HTMLInputElement|null;if(enabled)enabled.checked=!!r.quiet_enabled;}
document.querySelector('#settings')?.addEventListener('click',syncRoutineFields);
document.querySelector('#routine-save')?.addEventListener('click',async()=>{
 const value=(id:string)=>(document.getElementById(id) as HTMLInputElement).value;
 const r={...routine(),quiet_enabled:(document.getElementById('routine-quiet-enabled') as HTMLInputElement).checked,quiet_start:value('routine-quiet-start'),quiet_end:value('routine-quiet-end'),relax_start:value('routine-relax-start'),relax_end:value('routine-relax-end')};
 const status=document.querySelector('#routine-status')!;
 if([r.quiet_start,r.quiet_end,r.relax_start,r.relax_end].some(time=>!/^([01]\d|2[0-3]):[0-5]\d$/.test(time))||r.relax_start===r.relax_end||(r.quiet_enabled&&r.quiet_start===r.quiet_end)){status.textContent='Choose valid start and end times.';return;}
 try{profile=await invoke<Profile>('profile_save',{profile:{...profile,conversation_routine:r}});status.textContent='Daily rhythm saved.';if(routineQuiet())cancelListening();}catch{status.textContent='Enable profile storage in Profile before saving.';}
});

type Watchlist={id:string;name:string;symbols:WatchEntry[]};
let watchlistQuoteItems:MarketItem[]=[];let watchlistDraft:Watchlist[]=[];let watchlistIndex=0;let watchlistRequest=0;
let csvReview:CsvReview|undefined;let csvListId='';let csvReadRequest=0;
let watchlistPopoverId='';let watchlistPopoverTrigger:HTMLButtonElement|undefined;
function closeWatchlistPopover(){watchlistPopoverId='';document.querySelector('#watchlist-popover')?.classList.add('hidden');document.querySelectorAll('[data-quick-list]').forEach(button=>button.setAttribute('aria-expanded','false'));watchlistPopoverTrigger?.focus();}
function drawQuickWatchlists(){const row=document.querySelector('#watchlist-quick');if(!row)return;row.innerHTML=savedWatchlists().map(list=>`<button type="button" class="action-btn ghost" data-quick-list="${esc(list.id)}" aria-controls="watchlist-popover" aria-expanded="${watchlistPopoverId===list.id}">${esc(list.name)}</button>`).join('');row.querySelectorAll<HTMLButtonElement>('[data-quick-list]').forEach(button=>button.addEventListener('click',async()=>{document.querySelectorAll('[data-quick-list]').forEach(row=>row.setAttribute('aria-expanded','false'));watchlistPopoverTrigger=button;watchlistPopoverId=button.dataset.quickList!;document.querySelector('#watchlist-popover')?.classList.remove('hidden');button.setAttribute('aria-expanded','true');const choice=document.querySelector<HTMLSelectElement>('#ticker-choice')!;if(choice.value!==watchlistPopoverId){watchlistRequest++;watchlistQuoteItems=[];choice.value=watchlistPopoverId;}drawWatchlistPopover();document.querySelector<HTMLButtonElement>('#watchlist-popover-close')?.focus();await refreshWatchlistQuotes();renderMarketIndices();}));}
function drawWatchlistPopover(){if(!watchlistPopoverId)return;const list=savedWatchlists().find(row=>row.id===watchlistPopoverId);if(!list)return;document.querySelector('#watchlist-popover-title')!.textContent=list.name;const rows=document.querySelector('#watchlist-popover-rows')!;rows.innerHTML=list.symbols.length?`<p>Latest available prices · refresh every ${priceRefreshMinutes()} minutes · select a stock for details</p><div class="watchlist-price-rows">${list.symbols.map(entry=>{const item=watchlistQuoteItems.find(row=>row.symbol===entry.symbol);const change=item?.change_percent;return `<button type="button" data-watch-price="${esc(entry.symbol)}"><span><b>${esc(entry.name||entry.symbol)}</b><small>${esc(entry.symbol)}</small></span><span>${esc(item?.currency||'')} ${item?.price!=null?Number(item.price).toLocaleString(undefined,{maximumFractionDigits:2}):'—'}<small class="${change!=null&&Number(change)<0?'down':'up'}">${change!=null&&Number.isFinite(Number(change))?`${Number(change)>=0?'+':''}${Number(change).toFixed(2)}%`:'Movement unavailable'}</small><small>${esc(item?.quote?.session||'Session unknown')} · ${esc(quoteAge(item?.timestamp))}${item?.quote?.refresh_error?' · cached':''}</small></span></button>`;}).join('')}</div>`:'<p>This list is empty. Use Edit lists to add symbols or import a CSV.</p>';rows.querySelectorAll<HTMLButtonElement>('[data-watch-price]').forEach(button=>button.addEventListener('click',()=>{const entry=list.symbols.find(row=>row.symbol===button.dataset.watchPrice)!;closeWatchlistPopover();void openMarketDetail(entry.symbol,entry.name||entry.symbol);}));}
document.querySelector('#watchlist-popover-close')?.addEventListener('click',closeWatchlistPopover);
document.addEventListener('keydown',event=>{if(event.key==='Escape'&&watchlistPopoverId)closeWatchlistPopover();});
document.addEventListener('click',event=>{const target=event.target as Element;if(watchlistPopoverId&&!target.closest('#watchlist-popover,[data-quick-list]'))closeWatchlistPopover();});
function clearCsvReview(){csvReadRequest++;csvReview=undefined;csvListId='';const review=document.querySelector('#watchlist-import-review');if(review)review.textContent='';const input=document.querySelector<HTMLInputElement>('#watchlist-csv');if(input)input.value='';const apply=document.querySelector<HTMLButtonElement>('#watchlist-import-apply');if(apply)apply.disabled=true;}
document.querySelector('#watchlist-csv')?.addEventListener('change',async event=>{
 const file=(event.target as HTMLInputElement).files?.[0];if(!file)return;
 const request=++csvReadRequest;csvReview=undefined;
 const review=document.querySelector('#watchlist-import-review')!;const apply=document.querySelector<HTMLButtonElement>('#watchlist-import-apply')!;apply.disabled=true;
 const target=watchlistDraft[watchlistIndex];csvListId=target.id;review.textContent='Reading symbols…';
 try{
  if(file.size>262144)throw new Error('Choose a CSV smaller than 256 KB.');
  const text=await file.text();if(request!==csvReadRequest)return;
  csvReview=reviewWatchlistCsv(text,document.querySelector<HTMLSelectElement>('#watchlist-exchange')!.value,curatedExchanges);
  review.innerHTML=`<p><b>${csvReview.entries.length} symbols for ${esc(target.name)}</b> · ${csvReview.duplicates} duplicates skipped · ${csvReview.warnings.length} invalid rows skipped</p><div class="csv-symbol-preview">${csvReview.entries.map(item=>`<span>${esc(item.symbol)} <small>${esc(item.exchange)}</small></span>`).join('')}</div>${csvReview.warnings.length?`<details><summary>Review skipped rows</summary><ul>${csvReview.warnings.map(w=>`<li>${esc(w)}</li>`).join('')}</ul></details>`:''}<p>Review the symbols and exchange suffixes. Prices are checked after saving; a syntactically valid symbol may still be unavailable from the provider.</p>`;
  apply.disabled=false;
 }catch(error){if(request===csvReadRequest)review.textContent=String(error);}
});
document.querySelector('#watchlist-import-apply')?.addEventListener('click',()=>{
 const status=document.querySelector('#watchlist-status')!;const list=watchlistDraft[watchlistIndex];
 if(!csvReview||list.id!==csvListId){status.textContent='Choose the CSV again for this list.';return;}
 try{list.name=document.querySelector<HTMLInputElement>('#watchlist-name')!.value.trim()||list.name;list.symbols=mergeWatchlist(list.symbols,csvReview.entries,document.querySelector<HTMLSelectElement>('#watchlist-import-mode')!.value==='replace');drawWatchlist();status.textContent='Imported into draft. Review the list, then Save watchlists.';}catch(error){status.textContent=String(error);}
});
function savedWatchlists():Watchlist[]{const saved=Array.isArray(profile.market_watchlists)?profile.market_watchlists:[];return Array.from({length:Math.max(3,saved.length)},(_,i)=>saved[i]||{id:`list-${i+1}`,name:i===0?'Portfolio':`Watchlist ${i}`,symbols:[]});}
function syncWatchlistChoice(){drawQuickWatchlists();const choice=document.querySelector<HTMLSelectElement>('#ticker-choice');if(!choice)return;const value=choice.value;choice.innerHTML='<option value="home">Home-market basket</option>'+savedWatchlists().map(list=>`<option value="${esc(list.id)}">${esc(list.name)}</option>`).join('');if(Array.from(choice.options).some(option=>option.value===value))choice.value=value;}
async function refreshWatchlistQuotes(){const id=document.querySelector<HTMLSelectElement>('#ticker-choice')?.value;if(!id||id==='home')return;const request=++watchlistRequest;try{const response:any=await invoke('market_watchlist',{id});if(request===watchlistRequest)watchlistQuoteItems=response.items||[];}catch{if(request===watchlistRequest)watchlistQuoteItems=watchlistQuoteItems.map(item=>({...item,quote:{...item.quote,refresh_error:'Refresh unavailable'}}));}}
function drawWatchlist(){clearCsvReview();const list=watchlistDraft[watchlistIndex];document.querySelector('#watchlist-tabs')!.innerHTML=watchlistDraft.map((row,i)=>`<button type="button" data-watchlist="${i}" class="action-btn ${i===watchlistIndex?'':'ghost'}">${esc(row.name)}</button>`).join('');(document.querySelector('#watchlist-name') as HTMLInputElement).value=list.name;document.querySelector('#watchlist-entries')!.innerHTML=list.symbols.map((item,i)=>`<div class="watchlist-entry"><b>${esc(item.name||item.symbol)}</b><span>${esc(item.symbol)} · ${esc(item.exchange)}</span><button type="button" data-remove-symbol="${i}" aria-label="Remove ${esc(item.symbol)}">Remove</button></div>`).join('')||'<p>This list is empty. Add your first symbol below.</p>';document.querySelectorAll<HTMLElement>('[data-watchlist]').forEach(button=>button.addEventListener('click',()=>{watchlistDraft[watchlistIndex].name=(document.querySelector('#watchlist-name') as HTMLInputElement).value.trim()||`Watchlist ${watchlistIndex+1}`;watchlistIndex=Number(button.dataset.watchlist);drawWatchlist();}));document.querySelectorAll<HTMLElement>('[data-remove-symbol]').forEach(button=>button.addEventListener('click',()=>{list.symbols.splice(Number(button.dataset.removeSymbol),1);drawWatchlist();}));}
document.querySelector('#watchlists-open')?.addEventListener('click',async()=>{try{await loadSourceCatalog();}catch{}watchlistDraft=structuredClone(savedWatchlists());watchlistIndex=0;document.querySelector('#watchlist-modal')?.classList.remove('hidden');const selector=document.querySelector<HTMLSelectElement>('#watchlist-exchange')!;selector.innerHTML=curatedExchanges.map(market=>`<option value="${esc(market.id)}">${esc(market.country)} · ${esc(market.name)}</option>`).join('')||'<option value="India">India</option><option value="USA">USA</option>';const home=curatedExchanges.find(market=>market.country?.toLowerCase()===profile.country?.toLowerCase());if(home)selector.value=home.id;drawWatchlist();});
document.querySelector('#watchlists-close')?.addEventListener('click',()=>{clearCsvReview();document.querySelector('#watchlist-modal')?.classList.add('hidden');});
document.querySelector('#watchlist-add')?.addEventListener('click',()=>{
 const input=document.querySelector<HTMLInputElement>('#watchlist-symbol')!;const status=document.querySelector('#watchlist-status')!;
 try{const item=normalizeWatchEntry(input.value,document.querySelector<HTMLInputElement>('#watchlist-label')!.value,document.querySelector<HTMLSelectElement>('#watchlist-exchange')!.value,curatedExchanges);const list=watchlistDraft[watchlistIndex];if(list.symbols.some(row=>row.symbol===item.symbol))throw new Error('That symbol is already in this list.');list.name=document.querySelector<HTMLInputElement>('#watchlist-name')!.value.trim()||list.name;list.symbols=mergeWatchlist(list.symbols,[item]);input.value='';document.querySelector<HTMLInputElement>('#watchlist-label')!.value='';drawWatchlist();status.textContent='Added to draft. Save watchlists to keep it.';}catch(error){status.textContent=String(error);}
});
document.querySelector('#watchlist-save')?.addEventListener('click',async()=>{watchlistDraft[watchlistIndex].name=document.querySelector<HTMLInputElement>('#watchlist-name')!.value.trim()||`Watchlist ${watchlistIndex+1}`;const status=document.querySelector('#watchlist-status')!;try{profile=await invoke<Profile>('profile_save',{profile:{...profile,market_watchlists:watchlistDraft}});syncWatchlistChoice();status.textContent='Watchlists saved. Choose a list beside Ticker to see its prices.';watchlistRequest++;await refreshWatchlistQuotes();renderMarketIndices();}catch(error){status.textContent=`Could not save: ${String(error)}`;}});
document.querySelector('#ticker-choice')?.addEventListener('change',async()=>{if(watchlistPopoverId)closeWatchlistPopover();watchlistRequest++;watchlistQuoteItems=[];renderMarketIndices();await refreshWatchlistQuotes();renderMarketIndices();});

function priceRefreshMinutes(){const minutes=Number(profile.market_refresh_minutes);return [5,6,7].includes(minutes)?minutes:6;}
document.querySelector('#price-refresh')?.addEventListener('change',async event=>{const select=event.target as HTMLSelectElement;const minutes=Number(select.value);try{profile=await invoke<Profile>('profile_save',{profile:{...profile,market_refresh_minutes:minutes}});lastPriceRefresh=0;void markets();}catch{select.value=String(priceRefreshMinutes());pushMessage('Enable profile storage to save the price refresh interval.','butler');}});
syncWatchlistChoice();

setInterval(()=>{if(!startupBriefingWaitingSince)return;if(isQuiet()||routineQuiet()){startupBriefingWaitingSince=0;return;}if(Date.now()-startupBriefingWaitingSince>90000){startupBriefingWaitingSince=0;document.querySelector('#briefing-progress')!.textContent='Voice is still preparing. Use Daily briefing when it is ready.';return;}if(lastVoice?.tts_ready&&!responding)void launchBriefing();},1000);

void setupMobileVoice();

let mobileResumeRefresh=0;
let isAndroidApp=false;
void nativeInvoke<{supported:boolean}>('android_model_status').then(result=>{isAndroidApp=result.supported;}).catch(()=>{});
document.addEventListener('visibilitychange',()=>{
 if(!isAndroidApp||document.hidden||!profile.onboarding_complete||!profile.consents?.live_information||Date.now()-mobileResumeRefresh<60000)return;
 mobileResumeRefresh=Date.now();lastPriceRefresh=0;
 void nativeInvoke('prefetch_now').catch(()=>{});
 void markets();
 void news(activeNews);
});

installOnlineProviders();
setupRefinedSettings(()=>profile,p=>{profile=p;},'Smart Butler',loadPiperVoices);

import './settings-refresh.css';

installSpeechControls('Smart Butler');
document.addEventListener('app-quiet',()=>{briefingCancelled=true;briefingOfferPending=false;voiceTurn++;lastChatterAt=Date.now();stopNewsReading();cancelListening();quietUntil=8_640_000_000_000_000;localStorage.setItem('butler.cleanr5.quietUntil',String(quietUntil)); document.querySelector('#hero-line')!.textContent='Quiet. Press Talk or Daily briefing when you want speech again.';});
document.addEventListener('app-resume-speech',()=>{quietUntil=0;localStorage.removeItem('butler.cleanr5.quietUntil');});

import "./ui-foundation.css";

bindThemePicker();

import './bootstrap-components.css';
installNewsNavigation();
installPhoneLayout('Smart Butler');
installBootstrapUI();
import './phone_layout.css';

installBaselineUI('Smart Butler',()=>profile);
import './baseline_ui.css';

import './baseline_themes.css';

import './butler-layout.css';

import './readability-contract.css';
import './setup_patch.css';

// A freshly completed setup may not have fetched the weather widget yet.
document.addEventListener('click',event=>{if((event.target as HTMLElement).closest('[data-desktop-panel="weather"],[data-mobile-panel="weather"]'))void weather();});

document.addEventListener('click',event=>{if(!marketItems.length&&!refreshing.has('markets')&&(event.target as HTMLElement).closest('[data-desktop-panel="market"],[data-mobile-panel="market"]'))void markets();});

import {installMediaLayout} from "./media_layout";
import "./media_layout.css";
installMediaLayout();

document.title = 'Smart Butler · SB-20261114-R12-NEWS-LOCATION';
