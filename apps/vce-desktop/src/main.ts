import { setupLocalLearning } from './learning';
import { setupCompanion } from './companion';
import { setupMusic } from './music';
import { setupDisplay } from './display';
import { currentDayAndTime, listReminderEvents } from './reminders';
import { invoke } from '@tauri-apps/api/core';
import './style.css';

type Profile = { [key: string]: any; name: string; location: string; country: string; region: string; interests: string[]; preferences: string[]; consent: boolean };
type NewsItem = { title: string; url: string; summary?: string; published?: string; source?: string; thumbnail?: string; video_url?: string | null };
type MarketItem = { close?: string | number; quote?: any; symbol: string; name?: string; region?: string; kind?: string; price?: number; change?: number; change_percent?: number; currency?: string; source?: string; timestamp?: number; error?: string };
type VoiceDiag = { state: string; always_on: boolean; wake_mode: string; wake_word: string; microphone_ready: boolean; microphone?: string; speaker_ready: boolean; speaker?: string; asr_ready: boolean; tts_ready: boolean; vad_ready: boolean; kws_ready: boolean; denoise_ready: boolean; punctuation_ready: boolean; last_transcript?: string; last_voice_ms?: number; barge_in_count?: number; wake_count?: number; kws_decode_count?: number; kws_last_decode_ms?: number; dropped_audio_buffers?: number; input_level?: number; raw_input_level?: number; input_rate?: number; input_channels?: number; input_frames?: number; last_turn_peak?: number; last_turn_frames?: number; last_turn_hypotheses?: number; vad_active?: boolean; asr_partial?: string; rejected_fragments?: number; last_turn_outcome?: string; pause_to_finish_ms?: number; max_listening_ms?: number; last_error?: string };
type AppStatus = { build?: string; profile: Profile; voice: VoiceDiag; learning: any; log_path: string };
type VoiceEvent = { kind: string; text?: string; keyword?: string; asr_ms?: number; at_ms: number };

const app = document.querySelector<HTMLDivElement>('#app')!;
let musicController: ReturnType<typeof setupMusic> | undefined;
let profile: Profile = { name: 'Friend', location: '', country: '', region: '', interests: [], preferences: [], consent: false };
let activeNews = 'breaking';
let activeVideoCategory = 'breaking';
let newsItems: NewsItem[] = [];
let localNewsItems: NewsItem[] = [];
let regionalNewsItems: NewsItem[] = [];
let nationalNewsItems: NewsItem[] = [];
let localEventItems: NewsItem[] = [];
let latestWeather: any = null;
let coverageRequest = 0;
let videoItems: NewsItem[] = [];
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
let conversationInterrupt: Promise<unknown> = Promise.resolve();
function interruptConversation(): Promise<unknown> {
  conversationInterrupt = invoke('cancel_chat').catch(() => undefined);
  return conversationInterrupt;
}
const refreshing = new Set<string>();
let companion: ReturnType<typeof setupCompanion>;
let lastVoice: VoiceDiag | null = null;
let lastInteractionAt = Date.now();
let lastChatterAt = Date.now();
let launchBriefingStarted = false;
let chatterTopic = 0;
let launchWarmupCompleted = false;
let launchVoiceReady = false;
let launchContextPromise:Promise<unknown>|null = null;
let proactiveChatter = localStorage.getItem('buddy.proactiveChatter') !== 'off';
let calendarCursor = new Date();
let selectedCalendarDate = new Date();
let briefingOfferPending = false;

const esc = (s: unknown) => String(s ?? '').replace(/[&<>"]/g, m => ({ '&': '&amp;', '<': '&lt;', '>': '&gt;', '"': '&quot;' }[m]!));
const icon = (name: string) => {
  const p: Record<string, string> = {
    weather:'<path d="M7 19h10a4 4 0 0 0 .6-7.96A6 6 0 0 0 6 9.5 4.5 4.5 0 0 0 7 19Z"/><path d="M16 4v3M19.5 5.5l-2 2M21 9h-3"/>',
    news:'<path d="M4 5h16v14H4z"/><path d="M7 9h10M7 12h10M7 15h6"/>',
    music:'<path d="M9 18V6l10-2v12"/><circle cx="6" cy="18" r="3"/><circle cx="16" cy="16" r="3"/>',
    market:'<path d="M4 18 9 12l3 3 7-9"/><path d="M4 5v14h16"/>',
    video:'<rect x="3" y="5" width="18" height="14" rx="3"/><path d="m10 9 5 3-5 3z"/>',
    mic:'<path d="M12 3a3 3 0 0 0-3 3v6a3 3 0 0 0 6 0V6a3 3 0 0 0-3-3Z"/><path d="M19 11a7 7 0 0 1-14 0M12 18v3M9 21h6"/>',
    refresh:'<path d="M20 11a8 8 0 0 0-14.7-4L3 10"/><path d="M3 5v5h5"/><path d="M4 13a8 8 0 0 0 14.7 4L21 14"/><path d="M21 19v-5h-5"/>',
    plus:'<path d="M12 5v14M5 12h14"/>',
    settings:'<path d="M12 15.5a3.5 3.5 0 1 0 0-7 3.5 3.5 0 0 0 0 7Z"/><path d="m19.4 15 .1 2.2-2 1.2-1.8-1a7 7 0 0 1-1.7 1l-.2 2.1h-2.4l-.2-2.1a7 7 0 0 1-1.7-1l-1.8 1-2-1.2.1-2.2a7 7 0 0 1-1-1.7L2.8 13v-2.4l2.1-.2a7 7 0 0 1 1-1.7l-.1-2.2 2-1.2 1.8 1a7 7 0 0 1 1.7-1l.2-2.1h2.4l.2 2.1a7 7 0 0 1 1.7 1l1.8-1 2 1.2-.1 2.2a7 7 0 0 1 1 1.7l2.1.2V13l-2.1.2a7 7 0 0 1-1 1.8Z"/>',
    chevron:'<path d="m9 18 6-6-6-6"/>',
    play:'<path d="m8 5 11 7-11 7Z"/>',
    pause:'<path d="M8 5v14M16 5v14"/>',
    link:'<path d="M10 13a5 5 0 0 0 7.1.1l2-2a5 5 0 0 0-7.1-7.1l-1.1 1.1"/><path d="M14 11a5 5 0 0 0-7.1-.1l-2 2a5 5 0 0 0 7.1 7.1l1.1-1.1"/>',
    spark:'<path d="m12 3 1.3 5.7L19 10l-5.7 1.3L12 17l-1.3-5.7L5 10l5.7-1.3Z"/><path d="m19 16 .7 2.3L22 19l-2.3.7L19 22l-.7-2.3L16 19l2.3-.7Z"/>',
    globe:'<circle cx="12" cy="12" r="9"/><path d="M3 12h18M12 3c2.2 2.6 3.3 5.6 3.3 9S14.2 18.4 12 21C9.8 18.4 8.7 15.4 8.7 12S9.8 5.6 12 3Z"/>',
  };
  return `<svg viewBox="0 0 24 24" aria-hidden="true">${p[name] ?? p.spark}</svg>`;
};

function render() {
  app.innerHTML = `
  <section class="launch-splash" id="launch-splash" aria-label="BestBuddy is starting" aria-live="polite">
    <div class="launch-sky"><span></span><span></span><span></span></div>
    <div class="launch-macaw-wrap"><img src="/assets/macaw-buddy.png" class="launch-macaw" alt="BestBuddy macaw welcoming you"/></div>
    <div class="launch-copy"><div class="kicker">WELCOME TO</div><h1>BestBuddy</h1><p id="launch-message">Hello! I’m warming up my voice and gathering your day.</p><div class="launch-progress" aria-hidden="true"><i></i><i></i><i></i></div></div>
  </section>
  <div class="app-shell">
    <header class="topbar">
      <div class="brand">
        <div class="macaw-wrap" id="parrot"><img src="/assets/macaw-buddy.png" class="macaw" alt="BestBuddy macaw"/></div>
        <div class="brand-copy"><div class="brand-name">BestBuddy</div><div class="brand-sub">Your always-there companion</div></div>
      </div>
      <div class="header-greeting"><h1 id="greeting">Hello, ${esc(profile.name)}.</h1><p id="hero-line">I’m here with you.</p></div>
      <div class="header-actions">
        <div class="header-pill" id="engine-pill"><span class="status-dot"></span><span>STATUS</span></div>
        <div class="header-pill" id="mic-pill">${icon('mic')}<span>MIC</span></div>
        <div class="header-pill" id="speaker-pill">◖<span>SPEAKER</span></div>
        <div class="voice-mini"><span class="small-state" id="voice-top-state">Voice off</span><button id="talk" class="talk-button">${icon('mic')}<b>TALK</b></button></div>
        <button class="round-icon" id="settings" title="Settings">${icon('settings')}</button>
      </div>
    </header>

    <section class="desktop-context-bar" aria-label="Location, weather and voice summary">
      <div class="context-location"><span>${icon('globe')}</span><b id="header-location">${esc(profile.location)}</b><small id="header-region">${esc([profile.region,profile.country].filter(Boolean).join(', '))}</small></div>
      <div class="context-weather"><span id="header-weather-icon">☀</span><b id="header-weather-temp">—°</b><div class="header-weather-copy"><small id="header-weather-summary">Checking weather…</small><span id="header-weather-forecast">Today’s forecast is loading…</span></div></div>
      <div class="context-voice"><span class="status-dot"></span><b>Voice</b><small id="header-voice-state">Preparing always-on listening…</small></div>
    </section>

    <main class="workspace">
      <nav class="desktop-widget-nav" aria-label="Dashboard widgets">
        <div class="widget-nav-heading"><span>Dashboard</span><small>Choose a view</small></div>
        <button class="active" data-desktop-panel="chatter" aria-current="page">${icon('spark')}<span><b>BestBuddy</b><small>Conversation home</small></span></button>
        <button data-desktop-panel="weather">${icon('weather')}<span><b>Weather</b><small>Current and forecast</small></span></button>
        <button data-desktop-panel="calendar">${icon('spark')}<span><b>Daybook</b><small>Calendar and Panchang</small></span></button>
        <button data-desktop-panel="news">${icon('news')}<span><b>News</b><small>Headlines and sources</small></span></button>
        <button data-desktop-panel="market">${icon('market')}<span><b>Markets</b><small>Indices and companies</small></span></button>
        <button data-desktop-panel="music">${icon('music')}<span><b>Music</b><small>Library and player</small></span></button>
        <button data-desktop-panel="video">${icon('video')}<span><b>Video</b><small>News and local files</small></span></button>
      </nav>
      <section class="dashboard">
        <article class="panel weather-panel">
          <div class="panel-head"><div class="panel-title"><span class="panel-icon weather">${icon('weather')}</span><div><h2>Weather</h2><small id="weather-location">${esc(profile.location)}</small></div></div><div class="panel-tools"><span class="live-label" id="weather-live">NOT FETCHED</span><button class="refresh" id="weather-refresh" title="Refresh weather" aria-label="Refresh weather">${icon('refresh')}</button></div></div>
          <div class="weather-city-bar"><div id="weather-city-buttons" class="weather-city-buttons"></div><button id="add-city" class="weather-city-add" type="button">${icon('plus')} Add city</button><small>Home stays tied to your profile location · up to five extra cities</small></div>
          <div class="weather-overview"><div class="current-weather"><div class="weather-graphic" id="weather-graphic">☀</div><div class="weather-measure"><div class="current-temp" id="weather-temp">—°</div><div class="current-condition" id="weather-condition">Fetching current weather…</div><div class="weather-facts" id="weather-facts"></div></div></div><div class="weather-top"><div class="analog-clock" id="analog"><i class="hour"></i><i class="minute"></i><i class="second"></i></div><div class="weather-timecopy"><div class="weather-clockline"><span class="big-time" id="weather-time">—</span><span class="mini-day" id="weather-day">—</span></div><div class="big-date" id="weather-date">—</div></div></div></div>
          <div class="forecast-row" id="forecast"></div>
        </article>

        <article class="panel calendar-panel">
          <div class="panel-head"><div class="panel-title"><span class="panel-icon calendar-icon">◫</span><div><h2>Calendar &amp; daybook</h2><small>Schedule, Hindu calendar context and this day in history</small></div></div><div class="panel-tools"><button class="action-btn" id="daybook-add" type="button">${icon('plus')} Add schedule item</button><button class="action-btn ghost" id="briefing-now" type="button">Daily briefing</button><button class="panel-toggle" id="calendar-toggle" title="Open month view" aria-expanded="false">▾</button></div></div>
          <div class="daybook-summary" id="daybook-summary"></div>
          <div class="calendar-month hidden" id="calendar-month"><div class="calendar-nav"><button id="calendar-prev" type="button" aria-label="Previous month">‹</button><strong id="calendar-month-title"></strong><button id="calendar-next" type="button" aria-label="Next month">›</button></div><div class="calendar-weekdays"><span>Sun</span><span>Mon</span><span>Tue</span><span>Wed</span><span>Thu</span><span>Fri</span><span>Sat</span></div><div class="calendar-grid" id="calendar-grid"></div></div>
          <div class="day-context" id="day-context"><p>Choose a date for local events, regional and national context, science history and notable events.</p></div>
        </article>

        <article class="panel news-panel">
          <div class="panel-head"><div class="panel-title"><span class="panel-icon news">${icon('news')}</span><div><h2>News</h2><small>Headlines, pictures & video</small></div></div><div class="panel-tools"><span class="live-label" id="news-live">NOT FETCHED</span><button class="refresh" id="news-refresh" title="Refresh news">${icon('refresh')}</button></div></div>
          <div class="tabs" id="news-tabs"><button class="active" data-cat="breaking">BREAKING</button><button data-cat="world">WORLD</button><button data-cat="national">NATIONAL</button><button data-cat="regional">REGION</button><button data-cat="local">LOCAL</button><button data-cat="political">POLITICAL</button><button data-cat="business">BUSINESS</button><button data-cat="science">SCIENCE</button><button data-cat="ai-tech">AI &amp; TECH</button><button data-cat="sports">SPORTS</button><button data-cat="entertainment">ENTERTAINMENT</button><button data-cat="viral">VIRAL NEWS</button></div>
          <div class="news-ticker" id="news-ticker" aria-label="Current category news ticker"></div>
          <div class="news-list" id="news-list"></div>
        </article>

        <article class="panel chatter-panel mobile-active desktop-active">
          <div class="panel-head"><div class="panel-title"><span class="parrot-badge"><img src="/assets/macaw-buddy.png" alt=""/></span><div><h2>BestBuddy</h2><small>Conversation in motion</small></div></div><span class="live-label alive">ALIVE</span></div>
          <div class="persona-shelf" aria-label="Talk with BestBuddy"><button data-persona="Tell me a joke">Joke</button><button data-persona="Tell me a science fact">Science</button><button data-persona="Give me a medical fact">Wellness</button><button data-persona="Encourage me">Lift me up</button><button data-persona="Find an audiobook">Audiobooks</button><button id="daily-briefing-chat" type="button">Daily briefing</button><button id="chatter-now" type="button">Surprise me</button></div>
          <div class="chat-scroll" id="chat-scroll"><div class="chat-messages" id="chat-messages"></div></div>
          <label class="chatter-switch"><input id="proactive-chatter" type="checkbox" ${proactiveChatter?'checked':''}/> BestBuddy may start timely companion check-ins</label>
          <div class="chat-compose"><input id="chat-input" autocomplete="off" placeholder="Talk to BestBuddy…"/><button id="send">${icon('chevron')}</button></div>
        </article>

        <article class="panel music-panel collapsible-panel collapsed">
          <div class="panel-head"><div class="panel-title"><span class="panel-icon music">${icon('music')}</span><div><h2>Music</h2><small>Play inside BestBuddy</small></div></div><div class="panel-tools"><span class="live-label">READY</span><button class="refresh" id="music-refresh" title="Refresh music">${icon('refresh')}</button><button class="panel-toggle" data-toggle-panel="music" aria-expanded="false" title="Open music player">▾</button></div></div>
          <div class="music-hero"><div class="album-disc"><div class="album-hole"></div></div><div class="track-info"><b id="track-name">Choose a song</b><small id="track-meta">Select files or a music folder</small></div></div>
          <div class="media-discovery"><div><b>Discover online</b><small>Search official music services; sign-in and playback remain with the provider.</small></div><input id="music-online-query" type="search" placeholder="Song, film, artist or lyrics"><button data-music-provider="youtube">YouTube Music ↗</button><button data-music-provider="spotify">Spotify ↗</button><button data-music-provider="soundcloud">SoundCloud ↗</button></div>
          <audio id="audio" preload="metadata"></audio>
          <div class="music-controls"><button id="music-open" class="media-icon" title="Open audio">${icon('plus')}</button><button id="music-play" class="media-play" title="Play / pause">${icon('play')}</button><button id="music-next" class="media-icon" title="Next">${icon('chevron')}</button><div class="eq" id="eq"><i></i><i></i><i></i><i></i><i></i><i></i></div></div>
          <div class="progress"><span id="music-progress"></span></div>
        </article>

        <article class="panel market-panel">
          <div class="panel-head"><div class="panel-title"><span class="panel-icon market">${icon('market')}</span><div><h2>World Markets</h2><small>Major indices · refreshable board</small></div></div><div class="panel-tools"><span class="live-label" id="market-live">NOT FETCHED</span><button class="refresh" id="market-refresh" title="Refresh markets">${icon('refresh')}</button></div></div>
          <div class="market-tabs" role="tablist"><button class="active" data-market-view="indices">Indices</button><button data-market-view="components">Components</button><button data-market-view="market-news">Market news</button><button data-market-view="corporate-actions">Corporate actions</button><button data-market-view="company-news">Selected company</button><label>Exchange<select id="market-exchange"><option value="all">Home market</option><option value="India">India · NSE/BSE</option><option value="USA">USA</option><option value="UK">UK</option><option value="Germany">Germany</option><option value="Japan">Japan</option></select></label></div>
          <div class="ticker-tape" id="ticker-tape"></div>
          <div class="market-grid" id="market-grid"></div>
          <div class="market-browser hidden" id="market-browser"></div>
          <div class="market-news-ticker" id="market-news-ticker" aria-label="Business and market news ticker"></div>
        </article>

        <article class="panel video-panel collapsible-panel collapsed">
          <div class="panel-head"><div class="panel-title"><span class="panel-icon video">${icon('video')}</span><div><h2>Video</h2><small>News and local video player</small></div></div><div class="panel-tools"><span class="live-label">PLAYER</span><button class="refresh" id="video-refresh" title="Refresh video">${icon('refresh')}</button><button class="panel-toggle" data-toggle-panel="video" aria-expanded="false" title="Open video player">▾</button></div></div>
          <div class="tabs video-tabs" id="video-tabs"><button class="active" data-cat="breaking">BREAKING</button><button data-cat="world">WORLD</button><button data-cat="national">NATIONAL</button><button data-cat="regional">REGION</button><button data-cat="local">LOCAL</button><button data-cat="political">POLITICAL</button><button data-cat="business">BUSINESS</button><button data-cat="science">SCIENCE</button><button data-cat="ai-tech">AI &amp; TECH</button><button data-cat="sports">SPORTS</button><button data-cat="entertainment">ENTERTAINMENT</button><button data-cat="viral">VIRAL NEWS</button></div>
          <div class="youtube-search"><select id="video-provider" aria-label="Video source"><option value="youtube">YouTube</option><option value="msn">MSN Video</option><option value="vimeo">Vimeo</option></select><input id="youtube-query" type="search" aria-label="Video search or YouTube video URL" placeholder="Search videos or paste a YouTube link"/><button id="youtube-search" class="action-btn" type="button">Search / play</button><button id="youtube-open" class="action-btn ghost" type="button">Open source ↗</button><button id="video-stop" class="action-btn ghost" type="button">Stop video</button></div>
          <div class="video-discovery"><span>Trending searches:</span><button data-video-trend="latest music videos">Music</button><button data-video-trend="trending news videos">News</button><button data-video-trend="science documentaries">Science</button><button data-video-trend="new technology videos">Technology</button></div>
          <div class="video-stage" id="video-stage"><div class="video-empty">${icon('video')}<b>Video lives here</b><small>Choose a news video, search YouTube, or paste a YouTube link.</small></div></div>
          <div class="video-actions"><button id="video-open" class="action-btn">${icon('plus')} Open local video</button><button id="video-news" class="action-btn ghost">${icon('news')} Play news video</button></div>
        </article>
      </section>
    </main>

    <nav class="mobile-nav" aria-label="BestBuddy sections">
      <button type="button" data-mobile-panel="chatter" class="active" aria-current="page">${icon('spark')}<span>BestBuddy</span></button>
      <button type="button" data-mobile-panel="news">${icon('news')}<span>News</span></button>
      <button type="button" data-mobile-panel="weather">${icon('weather')}<span>Weather</span></button>
      <button type="button" data-mobile-panel="calendar">${icon('spark')}<span>Daybook</span></button>
      <button type="button" data-mobile-panel="market">${icon('market')}<span>Markets</span></button>
      <button type="button" data-mobile-panel="music">${icon('music')}<span>Music</span></button>
      <button type="button" data-mobile-panel="video">${icon('video')}<span>Video</span></button>
    </nav>

    <footer class="statusbar"><span id="footer-status">Local-first · personal data in Settings</span><span id="footer-log">Logs: %LOCALAPPDATA%\\BestBuddy\\logs\\buddy.log</span></footer>
  </div>

  <div class="modal hidden" id="media-modal"><div class="modal-card"><button id="media-close" class="modal-close">×</button><div id="media-content"></div></div></div>
  <div class="modal hidden" id="market-modal"><div class="modal-card market-detail-card"><button id="market-close" class="modal-close" aria-label="Close market detail">×</button><div id="market-detail"></div></div></div>
  <div class="modal hidden" id="schedule-modal"><form class="modal-card schedule-card" id="schedule-form"><button id="schedule-close" class="modal-close" type="button" aria-label="Close schedule editor">×</button><h2>Add schedule item</h2><p>Add a one-time or annual reminder to your encrypted local profile.</p><label>Title<input id="schedule-title" required maxlength="120"></label><div class="schedule-fields"><label>Date<input id="schedule-date" type="date" required></label><label>Time<input id="schedule-time" type="time" value="09:00" required></label></div><label class="check"><input id="schedule-annual" type="checkbox"> Repeat annually</label><label>Reminder lead time<select id="schedule-lead"><option value="0">On the day</option><option value="1">1 day before</option><option value="7">7 days before</option><option value="30">30 days before</option></select></label><div id="schedule-error" role="alert"></div><button class="primary-wide" type="submit">Save schedule item</button></form></div>
  <div class="modal hidden" id="settings-panel"><div class="modal-card settings-card" role="dialog" aria-modal="true" aria-labelledby="settings-title"><button id="settings-close" class="modal-close" aria-label="Close settings">×</button><div class="settings-header"><div><h2 id="settings-title">BestBuddy settings</h2><p>Your profile, voice, AI and app activity in one place.</p></div></div><div id="settings-action-result" class="settings-action-result" role="status" aria-live="polite"></div>
    <div class="settings-shell"><nav class="settings-nav" aria-label="Settings sections">
      <button class="setting-tile active" data-settings-view="profile" type="button">Profile & people</button>
      <button class="setting-tile" data-settings-view="status" type="button">Status & diagnostics</button>
      <button class="setting-tile" data-settings-view="voice" type="button">Voice & speech</button>
      <button class="setting-tile" data-settings-view="assistant" type="button">Conversation models</button>
      <button class="setting-tile" data-settings-view="learning" type="button">Learning & prefetch</button>
      <button class="setting-tile" data-settings-view="connections" type="button">Connections</button>
      <button class="setting-tile" id="open-logs" data-settings-view="logs" type="button">Logs</button>
    </nav><div class="settings-content">
      <section class="settings-view" data-settings-page="profile"><h3>Profile & people</h3><p>Review the details BestBuddy uses to address you and remember important people and dates.</p><div id="settings-profile-summary" class="settings-summary"></div><div class="settings-page-actions settings-nav-actions"><button id="edit-profile" class="action-btn" type="button">Edit profile and consents</button><button id="profile-ai-keys" class="action-btn ghost" type="button">AI keys and model choices</button></div><div id="profile-settings-detail" class="settings-detail" aria-live="polite"></div></section>
      <section class="settings-view hidden" data-settings-page="status"><h3>Status and diagnostics</h3><div class="device-report" id="device-report"></div></section>
      <section class="settings-view hidden" data-settings-page="voice"><h3>Voice & speech</h3><div class="settings-page-actions"><button id="speaker-test" class="action-btn ghost" type="button">${icon('music')} Test speaker</button></div><div class="voice-tuning"><h3>Conversation timing</h3><label>Pause before BestBuddy responds <output id="tune-pause-value">0.7 s</output><input id="tune-pause" type="range" min="400" max="1800" step="100" value="700"></label><label>Maximum listening time <output id="tune-limit-value">18 s</output><input id="tune-limit" type="range" min="6000" max="30000" step="1000" value="18000"></label><small>A longer pause helps when you speak slowly; a shorter pause makes replies start sooner.</small></div>
        <div class="device-report"><b>BestBuddy's speaking voices</b><label class="voice-auto-label"><input id="piper-auto" type="checkbox"> Change voice and pace for the conversation</label><small>BestBuddy uses a playful voice for jokes, calmer pacing for stories and explicit requests for comfort, and suitable installed voices for other topics. You can keep one voice instead.</small><label>Ready to use<select id="piper-voice"></select></label><button id="piper-select" class="action-btn ghost" type="button">Keep this voice</button><div id="piper-voice-report">Checking installed voices…</div><p>The Macaw choice is a playful effect on BestBuddy's bundled voice, not a separately trained voice. Optional voices are downloaded only when you choose one, verified before installation, and kept in BestBuddy's writable data folder.</p><div id="piper-voice-catalog" class="voice-catalog"></div></div>
        <div class="voice-test device-report"><b>Voice signal test</b><small>Click Record microphone, then say “Read the news” clearly. BestBuddy records five seconds through the device microphone path. Listen locally, or feed the WAV through ASR → BestBuddy → speaker.</small>
          <div class="voice-test-actions"><button id="mic-record" class="action-btn" type="button">Record microphone</button><button id="mic-save" class="action-btn ghost" type="button" disabled>Save WAV</button><button id="mic-replay" class="action-btn ghost" type="button" disabled>Feed recording to BestBuddy</button></div>
          <audio id="mic-playback" controls hidden></audio>
          <label class="voice-file-label">Or choose a WAV file <input id="voice-wav-file" type="file" accept=".wav,audio/wav,audio/x-wav"/></label><button id="voice-wav-run" class="action-btn ghost" type="button">Feed selected WAV to BestBuddy</button>
          <div id="voice-test-result" role="status">No voice signal test run yet.</div>
        </div></section>
      <section class="settings-view hidden conversation-settings" data-settings-page="assistant"><div class="settings-view-heading"><div><h3>Conversation models</h3><p>Choose how BestBuddy composes open conversation. Commands, phrasebook, RAG and prefetched facts remain first in the response path.</p></div><div id="llm-report" class="model-status">Checking setup…</div></div>
        <div class="model-section vault-section"><div class="model-section-title"><div><span class="section-number">1</span><div><h4>User data and key vault</h4><p>Profile, contacts, reminders and learning files are encrypted for your Windows account. Provider keys use a separate password-protected AES-256 vault.</p></div></div></div><div class="model-inline"><label class="model-field"><span>Vault password</span><input id="llm-vault-password" type="password" autocomplete="new-password" placeholder="At least 12 characters"></label><button id="llm-unlock" class="action-btn" type="button">Create / unlock vault</button></div><small>The password is never saved and cannot be recovered. Unlock once after each app launch.</small></div>
        <div class="model-section hf-section"><div class="model-section-title"><div><span class="section-number">2</span><div><h4>Hugging Face access</h4><p>Used for optional model downloads and Hugging Face online inference.</p></div></div></div><div class="provider-key-line"><label class="model-field"><span>Hugging Face token</span><input id="llm-hf-token" type="password" autocomplete="off" placeholder="hf_…"></label><button class="action-btn llm-key-save" data-key-provider="huggingface" data-key-input="llm-hf-token" type="button">Save token</button><button class="action-btn ghost llm-direct-guide" data-url="https://huggingface.co/settings/tokens" type="button">Get token ↗</button><button class="action-btn ghost llm-direct-guide" data-url="https://huggingface.co/docs/hub/security-tokens" type="button">Guide ↗</button></div></div>
        <div class="model-section"><div class="model-section-title"><div><span class="section-number">3</span><div><h4>Compact local model</h4><p>BestBuddy downloads verified model files into its own private app-data folder. It never imports models or runtimes from older apps or unrelated caches.</p></div></div></div><div class="model-inline local-model-line"><label class="model-field"><span>Local model</span><select id="llm-local-model"><option value="qwen3-0.6b">Qwen3 0.6B · about 0.6 GB</option><option value="qwen3-1.7b">Qwen3 1.7B · about 1.3 GB</option></select></label><button id="llm-prepare-local" class="action-btn" type="button">Prepare model</button></div><div class="compact-links"><button class="text-link llm-guide" data-url="https://huggingface.co/ggml-org/Qwen3-1.7B-GGUF/blob/main/Qwen3-1.7B-Q4_K_M.gguf">Qwen3 1.7B details ↗</button><button class="text-link llm-guide" data-url="https://huggingface.co/Qwen/Qwen3-0.6B-GGUF">Qwen3 0.6B details ↗</button></div></div>
        <div class="model-section"><div class="model-section-title"><div><span class="section-number">4</span><div><h4>Response composer</h4><p>Automatic is recommended: verified local sources, local model, then an available configured online model.</p></div></div></div><div class="composer-box"><label class="model-field"><span>Composer mode</span><select id="llm-provider"><option value="auto">Automatic · sources → local → online</option><option value="rules">Built-in sources only</option><option value="local" disabled>Local GGUF only · prepare first</option><option value="online">Online provider only</option></select></label><label class="model-field"><span>Active online provider</span><select id="llm-online-provider"><option value="openrouter">OpenRouter</option><option value="groq">Groq</option><option value="gemini">Google Gemini</option><option value="huggingface">Hugging Face</option></select></label><label class="model-field"><span>Active model</span><input id="llm-online-model" list="online-model-suggestions" autocomplete="off" placeholder="Provider model ID"><datalist id="online-model-suggestions"><option value="openrouter/free"><option value="openai/gpt-oss-20b"><option value="openai/gpt-oss-120b"><option value="qwen/qwen3.8-27b"><option value="gemini-3.8-flash"></datalist></label><button id="llm-save" class="action-btn" type="button">Save choices</button></div></div>
        <div class="model-section online-models"><div class="model-section-title"><div><span class="section-number">5</span><div><h4>Online model choices</h4><p>Select a model, enter that provider's key, then use the row. Limits and availability can change; official account and limit links are beside each key.</p></div></div><button id="llm-refresh-free" class="action-btn ghost" type="button">Refresh free list</button></div>
          <div class="online-model-head"><span>Use</span><span>Provider</span><span>Model</span><span>Encrypted API key</span><span>Account and help</span></div>
          <div class="online-model-row" data-provider-row="openrouter" data-fixed-model="openrouter/free"><input type="radio" name="online-active" aria-label="Use OpenRouter automatic free router"><strong>OpenRouter</strong><select disabled><option>Automatic free router</option></select><input class="row-api-key" type="password" autocomplete="off" placeholder="OpenRouter key"><div><button class="text-link llm-row-key" type="button">Save key</button><button class="text-link llm-direct-guide" data-url="https://openrouter.ai/keys" type="button">Get key ↗</button><button class="text-link llm-direct-guide" data-url="https://openrouter.ai/docs/quickstart" type="button">Guide ↗</button></div></div>
          <div class="online-model-row" data-provider-row="openrouter"><input type="radio" name="online-active" aria-label="Use a specific free OpenRouter model"><strong>OpenRouter</strong><select id="llm-free-model"><option value="openrouter/free">Loading current free models…</option></select><input class="row-api-key" type="password" autocomplete="off" placeholder="Same OpenRouter key"><div><button class="text-link llm-row-key" type="button">Save key</button><button class="text-link llm-direct-guide" data-url="https://openrouter.ai/models?fmt=cards&max_price=0" type="button">Models ↗</button><button class="text-link llm-direct-guide" data-url="https://openrouter.ai/docs/faq" type="button">Limits ↗</button></div></div>
          <div class="online-model-row" data-provider-row="groq"><input type="radio" name="online-active" aria-label="Use Groq"><strong>Groq</strong><select><option value="openai/gpt-oss-20b">GPT-OSS 20B</option><option value="openai/gpt-oss-120b">GPT-OSS 120B</option><option value="qwen/qwen3.8-27b">Qwen3.8 27B</option></select><input class="row-api-key" type="password" autocomplete="off" placeholder="Groq key"><div><button class="text-link llm-row-key" type="button">Save key</button><button class="text-link llm-direct-guide" data-url="https://console.groq.com/keys" type="button">Get key ↗</button><button class="text-link llm-direct-guide" data-url="https://console.groq.com/docs/rate-limits" type="button">Limits ↗</button></div></div>
          <div class="online-model-row" data-provider-row="gemini"><input type="radio" name="online-active" aria-label="Use Google Gemini"><strong>Gemini</strong><select><option value="gemini-3.8-flash">Gemini 3.8 Flash</option><option value="gemini-3.5-flash-lite">Gemini 3.5 Flash-Lite</option><option value="gemini-2.5-flash-lite">Gemini 2.5 Flash-Lite</option></select><input class="row-api-key" type="password" autocomplete="off" placeholder="Gemini key"><div><button class="text-link llm-row-key" type="button">Save key</button><button class="text-link llm-direct-guide" data-url="https://aistudio.google.com/app/apikey" type="button">Get key ↗</button><button class="text-link llm-direct-guide" data-url="https://ai.google.dev/gemini-api/docs/pricing" type="button">Limits ↗</button></div></div>
          <div class="online-model-row" data-provider-row="huggingface"><input type="radio" name="online-active" aria-label="Use Hugging Face inference"><strong>Hugging Face</strong><select><option value="openai/gpt-oss-120b:fastest">GPT-OSS 120B</option><option value="openai/gpt-oss-20b:fastest">GPT-OSS 20B</option></select><input type="text" disabled value="Uses token above" aria-label="Uses Hugging Face token above"><div><button class="text-link llm-direct-guide" data-url="https://huggingface.co/playground" type="button">Playground ↗</button><button class="text-link llm-direct-guide" data-url="https://huggingface.co/docs/inference-providers/pricing" type="button">Credits ↗</button></div></div>
          <div id="llm-free-model-status" class="row-status" role="status">Refresh to verify the current OpenRouter free catalog.</div>
        </div>
        <div class="model-section usage-section"><div class="model-section-title"><div><h4>Usage and saved credentials</h4><p>BestBuddy falls back after a provider rate-limit or availability error.</p></div></div><div class="model-inline"><button id="llm-refresh-usage" class="action-btn ghost" type="button">Check reported usage</button><button id="llm-remove-key" class="action-btn ghost" type="button">Remove active provider key</button><div id="llm-usage-report" role="status">No usage check run.</div></div></div>
        <p class="model-privacy-note">Online mode sends your request and relevant public references to the active provider. Stored contacts, birth dates and the private local draft are not supplied as reference context. Current facts continue to use BestBuddy's public news, weather and market sources.</p>
      </section>
      <section class="settings-view hidden" data-settings-page="logs"><div class="settings-view-heading"><h3>BestBuddy log</h3><div><button id="refresh-logs" class="action-btn ghost">Refresh</button><button id="open-log-folder" class="action-btn ghost">Open folder ↗</button></div></div><pre id="settings-log" tabindex="0" aria-label="Recent BestBuddy log entries">Choose Logs to load recent entries.</pre></section>
      <section class="settings-view hidden" data-settings-page="learning"><h3>Learning & prefetch</h3><div class="settings-page-actions"><button id="prefetch" class="action-btn" type="button">${icon('refresh')} Prefetch now</button></div><div class="device-report" id="learning-report"></div><div class="settings-page-actions" id="learning-settings-actions"></div><div id="learning-settings-detail" class="settings-detail" aria-live="polite"></div></section>
      <section class="settings-view hidden" data-settings-page="connections"><h3>Connections</h3><p>Sign in through each service's own website. Add a contact's conversation or meeting link in Profile & people, then choose that contact to open it. BestBuddy does not read passwords, online presence or start calls on your behalf.</p><div class="settings-connection-grid" id="connection-settings-actions">
        <button id="open-messenger" class="action-btn ghost" type="button">Messenger ↗</button>
        <button class="action-btn ghost connection-open" data-url="https://web.whatsapp.com/" data-name="WhatsApp" type="button">WhatsApp ↗</button>
        <button class="action-btn ghost connection-open" data-url="https://app.slack.com/client" data-name="Slack" type="button">Slack ↗</button>
        <button class="action-btn ghost connection-open" data-url="https://discord.com/app" data-name="Discord" type="button">Discord ↗</button>
        <button class="action-btn ghost connection-open" data-url="https://zoom.us/signin" data-name="Zoom" type="button">Zoom ↗</button>
        <button class="action-btn ghost connection-open" data-url="https://teams.microsoft.com/" data-name="Teams" type="button">Teams ↗</button>
      </div><p>Direct account linking needs a registered developer application and service-specific permissions. Messenger and WhatsApp personal contact lists and video calls are not exposed through their business messaging APIs.</p></section>
    </div></div></div></div>
  <div class="modal hidden" id="onboarding"><div class="modal-card onboarding-card"><div class="onboard-macaw"><img src="/assets/macaw-buddy.png" alt="BestBuddy"/></div><div class="kicker">LET’S GET ACQUAINTED</div><h2>What should I call you?</h2><p>BestBuddy works best when it knows who it is talking with.</p><input id="profile-name" placeholder="Your preferred name"/><input id="profile-location" placeholder="City"/><input id="profile-region" placeholder="Region / state"/><input id="profile-interests" placeholder="Interests, separated by commas"/><label class="consent"><input id="profile-consent" type="checkbox"/> Save these details on this device so BestBuddy can personalize conversations.</label><button id="save-profile" class="primary-wide">Meet BestBuddy</button></div></div>
  `;
}

function setParrotState(state: 'idle'|'listen'|'talk'|'happy'|'sleep') {
  const p = document.querySelector('#parrot');
  if (p) p.setAttribute('data-state', state);
  const hero = document.querySelector('#voice-caption');
  if (hero) hero.textContent = state === 'listen' ? 'Voice status' : state === 'talk' ? 'BestBuddy is talking…' : state === 'happy' ? 'That made BestBuddy smile.' : state === 'sleep' ? 'Resting, but still here.' : 'Always ready when you are.';
}

function pushMessage(text: string, who: 'user'|'buddy' = 'buddy', meta = '') {
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
  if(response?.plugin?.plugin_id!=='buddy-persona' || !source?.source_url || !String(source.source_url).startsWith('https://'))return;
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

async function news(category = activeNews) {
  if(refreshing.has('news'))return; refreshing.add('news');
  activeNews = category;
  setRefreshing('news-refresh', true);
  document.querySelectorAll('#news-tabs button').forEach(b => b.classList.toggle('active', (b as HTMLElement).dataset.cat === category));
  try {
    const r: any = await invoke('news_widget', { category });
    const items = r?.items || [];
    newsItems = items;
    const ticker=document.querySelector<HTMLElement>('#news-ticker');if(ticker)ticker.innerHTML=items.slice(0,12).map((item:NewsItem)=>`<span>${esc(item.title)} <b>${esc(item.source||'News')}</b></span>`).join('');
    const list = document.querySelector('#news-list')!;
    if (!items.length) { list.innerHTML = '<div class="empty-state">No fresh headlines yet. Refresh again when the network is available.</div>'; await invoke('visible_news_set',{headlines:[]}); return; }
    list.innerHTML = items.slice(0,8).map((item:NewsItem, i:number) => `<button class="news-row" data-index="${i}"><div class="news-thumb">${item.thumbnail ? `<img src="${esc(item.thumbnail)}" alt=""/>` : `<span>${i<3?'◆':'•'}</span>`}</div><div class="news-copy"><b>${esc(item.title)}</b><small>${esc(item.source || 'News')} · ${esc(item.published || 'today')}</small></div><div class="news-action">${item.video_url ? '▶' : ''}${icon('chevron')}</div></button>`).join('');
    list.querySelectorAll('.news-row').forEach((el, i) => el.addEventListener('click', () => openNews(newsItems[i], true)));
    await invoke('visible_news_set',{headlines:items.slice(0,8).map((item:NewsItem)=>item.title)});
    document.querySelector('#news-live')!.textContent = 'LIVE';
  } catch(e) { document.querySelector('#news-live')!.textContent = 'UNAVAILABLE'; document.querySelector('#news-list')!.textContent=String(e); void invoke('visible_news_set',{headlines:[]}).catch(()=>{}); }
  finally { refreshing.delete('news'); setRefreshing('news-refresh', false); }
}

const spokenHeadline = (value:string) => value.replace(/\s+-\s+[^-]{2,45}$/,'').replace(/\s+/g,' ').trim();
const firstUsefulHeadline = (items:NewsItem[]) => items.map(item=>spokenHeadline(item.title)).find(title=>title.length>12) || '';

async function contextualNews(category:'local'|'regional'|'national'):Promise<NewsItem[]> {
  try {
    const result:any=await invoke('news_widget',{category});
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
  const panchang=`https://www.drikpanchang.com/panchang/day-panchang.html?date=${encodeURIComponent(localDateKey(date))}&geoname-city=${encodeURIComponent(profile.location)}&geoname-state=${encodeURIComponent(profile.region||'')}&geoname-country=${encodeURIComponent(profile.country||'')}`;
  box.innerHTML=`<div class="daybook-date"><strong>${esc(date.toLocaleDateString(undefined,{weekday:'long',day:'numeric',month:'long',year:'numeric'}))}</strong><span>${esc(hindu.saka)} · estimated ${esc(hindu.tithi)} · ${esc(hindu.phase)}</span><small>Panchang location: ${esc([profile.location,profile.region,profile.country].filter(Boolean).join(', '))}</small></div><div class="daybook-reminders"><b>${reminders.length?'Schedule':'Schedule clear'}</b>${reminders.map(item=>`<span>${esc(item.time||'09:00')} · ${esc(item.title)}</span>`).join('')||'<span>No saved reminder for this date.</span>'}</div><button id="calendar-panchang" class="source-btn" type="button">Open full location-based Panchang ↗</button>`;
  box.querySelector('#calendar-panchang')?.addEventListener('click',()=>void invoke('open_external',{url:panchang}).catch(error=>pushMessage(String(error))));
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
  return `For ${profile.location}, it is ${Math.round(Number(latestWeather.temperature))} degrees with ${latestWeather.condition}.${range}${wind}`;
}

function marketBriefing():string {
  const live=marketItems.filter(item=>item.price!==undefined);
  if(!live.length)return 'Market feeds have not returned a reliable update yet.';
  const movers=[...live].filter(item=>Number.isFinite(Number(item.change_percent))).sort((a,b)=>Math.abs(Number(b.change_percent))-Math.abs(Number(a.change_percent))).slice(0,3);
  if(!movers.length)return `${live.length} leading market indices have current quotes in Market Watch.`;
  return `Market watch: ${movers.map(item=>`${item.name} is ${Number(item.change_percent)>=0?'up':'down'} ${Math.abs(Number(item.change_percent)).toFixed(2)} percent`).join('; ')}.`;
}

const wantsDailyBriefing=(text:string)=>/\b(daily|full|today'?s?|morning|evening)?\s*briefing\b/i.test(text)||/\bbrief me\b/i.test(text);
async function runDailyBriefing(reason='On request'){
  if(responding)return;
  responding=true;briefingOfferPending=false;lastInteractionAt=Date.now();
  try{
    pushMessage('Gathering your schedule, weather, news, markets and daybook…','buddy','Daily briefing');
    await Promise.allSettled([refreshHomeWeather(),weather(),news('breaking'),markets(),contextualNews('local'),contextualNews('regional'),contextualNews('national'),localEvents(),loadCalendarContext(new Date())]);
    const openers=['All right—let me share the useful bits without making it sound like a board meeting.','You know, this is quite a day. Let us take it one feather at a time.','Listen—your BestBuddy briefing is ready, with facts first and a little sparkle at the end.'];
    await sayCompanionLine(openers[new Date().getHours()%openers.length],reason);
    const schedule=scheduleForToday();
    await sayCompanionLine(schedule.length?`First, your schedule: ${schedule.slice(0,5).join('; ')}.`:'First, your schedule is clear. That is either freedom or a suspiciously quiet calendar.','Schedule');
    await sayCompanionLine(`FYI, ${weatherBriefing()}`,'Weather');
    const breaking=firstUsefulHeadline(newsItems),local=firstUsefulHeadline(localNewsItems),regional=firstUsefulHeadline(regionalNewsItems),national=firstUsefulHeadline(nationalNewsItems);
    if(breaking)await sayCompanionLine(`You need to hear this: ${breaking}.`,'Breaking news');
    if(local)await sayCompanionLine(`Closer to home in ${profile.location}: ${local}.`,'Local news');
    if(regional)await sayCompanionLine(`Across ${profile.region}: ${regional}.`,'Regional news');
    if(national)await sayCompanionLine(`Nationally in ${profile.country}: ${national}.`,'National news');
    await sayCompanionLine(`Now the money weather—${marketBriefing()}`,'Markets');
    const event=firstUsefulHeadline(localEventItems);if(event)await sayCompanionLine(`And here is something happening nearby: ${event}.`,'Local event');
    const flourish=await invoke<string>('chatter').catch(()=>`Trust me, ${profile.name}: a clear plan, a curious mind and one good laugh can carry a day surprisingly far.`);
    await sayCompanionLine(`Before I fly: ${flourish}`,'A little lift');
  }finally{responding=false;lastChatterAt=Date.now();setParrotState('idle');}
}

async function offerScheduledBriefing(){
  if(!profile.onboarding_complete||briefingOfferPending||responding||document.hidden||!proactiveChatter||Date.now()-lastInteractionAt<10*60_000)return;
  const timezone=profile.timezone||Intl.DateTimeFormat().resolvedOptions().timeZone;
  const hour=Number(new Intl.DateTimeFormat('en-US',{timeZone:timezone,hour:'2-digit',hourCycle:'h23'}).format(new Date()));
  const slot=hour>=7&&hour<10?'morning':hour>=11&&hour<14?'midday':hour>=16&&hour<19?'afternoon':hour>=20&&hour<23?'evening':'';
  if(!slot)return;
  const day=currentDayAndTime(new Date(),timezone).day,key=`buddy.briefingOffer.${day}.${slot}`;
  if(localStorage.getItem(key))return;
  localStorage.setItem(key,'offered');briefingOfferPending=true;
  const message=`${profile.name}, would you like your ${slot} briefing? I have your schedule, weather, headlines, markets and a cheerful extra ready.`;
  pushMessage(message,'buddy','Briefing offer');await sayCompanionLine(message,'Briefing offer',false);
}

function greetingLine():string {
  const timezone=profile.timezone||Intl.DateTimeFormat().resolvedOptions().timeZone;
  const hour=Number(new Intl.DateTimeFormat('en-US',{timeZone:timezone,hour:'2-digit',hourCycle:'h23'}).format(new Date()));
  const greeting=hour<12?'Good morning':hour<18?'Good afternoon':'Good evening';
  const flourishes=['bright feathers, open ears, and ready for the day','your curious macaw is awake and on companion duty','I have fluffed my feathers and gathered your morning notes'];
  return `${greeting}, ${profile.name}. BestBuddy here—${flourishes[new Date().getDate()%flourishes.length]}.`;
}

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
  if(show){pushMessage(text,'buddy',meta);document.querySelector('#hero-line')!.textContent=text;}
  if(!lastVoice?.tts_ready)return;
  setParrotState('talk');setVoiceTop('Speaking…');
  try{await invoke('voice_speak',{text,request:meta,capability:'persona.briefing'});}catch(error){pushMessage(`Speech problem: ${String(error)}`,'buddy');}
  finally{setParrotState('idle');void refreshStatus();}
}

async function launchBriefing(){
  if(launchBriefingStarted||!profile.onboarding_complete||!document.querySelector('#onboarding')?.classList.contains('hidden'))return;
  launchBriefingStarted=true;responding=true;
  try{
    const welcome=greetingLine();
    pushMessage(welcome,'buddy','Welcome');document.querySelector('#hero-line')!.textContent=welcome;
    const context=launchContextPromise||Promise.all([refreshHomeWeather(),weather(),news('breaking'),markets(),contextualNews('local'),contextualNews('regional'),contextualNews('national'),localEvents(),loadCalendarContext(new Date())]);
    const ready=launchWarmupCompleted?launchVoiceReady:await waitForSpeechReady();
    if(ready)await sayCompanionLine(welcome,'Welcome',false);
    await context;
    const schedule=scheduleForToday();
    await sayCompanionLine(schedule.length?`On today's schedule: ${schedule.slice(0,3).join('; ')}.${schedule.length>3?` There are ${schedule.length-3} more items in your reminders.`:''}`:`Your schedule looks clear today. I am keeping the day open for something enjoyable.`,'Today');
    await sayCompanionLine(weatherBriefing(),'Weather');
    const breaking=firstUsefulHeadline(newsItems);
    if(breaking)await sayCompanionLine(`You know what happened? ${breaking}. I have the rest of the breaking headlines ready in News.`,'Breaking news');
    const nearby=firstUsefulHeadline(localNewsItems);
    if(nearby)await sayCompanionLine(`Closer to home in ${profile.location}, the headline to notice is: ${nearby}. I will keep watching for local events and useful updates.`,'Local briefing');
    const regional=firstUsefulHeadline(regionalNewsItems.filter(item=>spokenHeadline(item.title)!==nearby));
    if(regional&&profile.region)await sayCompanionLine(`Across ${profile.region}, another useful update is: ${regional}.`,'Regional briefing');
    const national=firstUsefulHeadline(nationalNewsItems.filter(item=>![nearby,regional].includes(spokenHeadline(item.title))));
    if(national&&profile.country)await sayCompanionLine(`For ${profile.country}, a leading headline is: ${national}.`,'National briefing');
    const event=firstUsefulHeadline(localEventItems);
    if(event)await sayCompanionLine(`And around ${profile.location}, this event may be worth a look: ${event}.`,'Local events');
    const bollywood=nationalNewsItems.find(item=>/bollywood|film|cinema|actor|actress|movie/i.test(`${item.title} ${item.summary||''}`));
    if(bollywood)await sayCompanionLine(`And a little Bollywood masala before I fly off: ${spokenHeadline(bollywood.title)}.`,'Bollywood');
    await sayCompanionLine(`One last feather in the forecast: ${marketBriefing()}`,'Markets');
    if(!ready)pushMessage('The briefing is on screen. Voice was not ready in time, so BestBuddy will speak the next check-in when TTS becomes available.','buddy','Voice notice');
    lastChatterAt=Date.now();
  }finally{responding=false;setParrotState('idle');}
}

function openNews(item: NewsItem, openArticle = false) {
  const request = ++coverageRequest;
  const modal = document.querySelector('#media-modal')!; const c = document.querySelector('#media-content')!; modal.classList.remove('hidden');
  if (item.video_url) {
    const u = item.video_url;
    const parsed = new URL(u);
    const youtube = ['www.youtube.com','youtube.com','m.youtube.com','youtu.be'].includes(parsed.hostname);
    const id = parsed.hostname==='youtu.be' ? parsed.pathname.slice(1) : parsed.searchParams.get('v');
    if (parsed.hostname==='player.pbs.org' && /^\/widget\/partnerplayer\/\d+\/$/.test(parsed.pathname)) {
      c.innerHTML = `<div class="media-title"><span>${icon('video')}</span><b>${esc(item.title)}</b></div><iframe class="embedded-video" src="${esc(u)}" allowfullscreen></iframe><button class="source-btn" id="media-source">Open source ${icon('link')}</button>`;
    } else if (youtube && id && /^[A-Za-z0-9_-]{11}$/.test(id)) {
      c.innerHTML = `<div class="media-title"><span>${icon('video')}</span><b>${esc(item.title)}</b></div><iframe class="embedded-video" src="https://www.youtube.com/embed/${esc(id || '')}" allowfullscreen></iframe><button class="source-btn" id="media-source">Open source ${icon('link')}</button>`;
    } else {
      c.innerHTML = `<div class="media-title"><span>${icon('video')}</span><b>${esc(item.title)}</b></div><video class="embedded-video" controls autoplay src="${esc(u)}"></video><button class="source-btn" id="media-source">Open source ${icon('link')}</button>`;
    }
  } else {
    c.innerHTML = `<div class="article-view">${item.thumbnail ? `<img src="${esc(item.thumbnail)}" class="article-image" alt=""/>` : ''}<h3>${esc(item.title)}</h3><p>${esc(item.summary || '')}</p><button class="source-btn" id="media-source">Read source ${icon('link')}</button></div>`;
  }
  document.querySelector('#media-source')?.addEventListener('click', () => void invoke('open_external', { url: item.url }).catch(error => { const status=c.querySelector('#article-open-status'); if(status)status.textContent=`Could not open the article: ${String(error)}`; }));
  if (!openArticle) return;
  c.insertAdjacentHTML('beforeend', '<p id="article-open-status" class="article-open-status" role="status">Opening the full article in your browser…</p><section class="coverage-section" aria-label="Other news outlets"><h4>Other outlets covering this story</h4><div id="coverage-list" role="status">Looking for matching coverage…</div></section>');
  void invoke('open_external', { url: item.url }).catch(error => { const status=c.querySelector('#article-open-status'); if(status)status.textContent=`Could not open the article: ${String(error)}`; });
  void invoke<{items: NewsItem[]; searched: number}>('news_coverage', { title:item.title, source:item.source||'', url:item.url }).then(result => {
    if (request !== coverageRequest) return;
    const list=c.querySelector<HTMLElement>('#coverage-list'); if(!list)return;
    const related=result.items||[];
    list.innerHTML=related.length ? related.map((story,i)=>`<button class="coverage-story" data-index="${i}" type="button"><b>${esc(story.source||'News outlet')}</b><span>${esc(story.title)}</span>${icon('link')}</button>`).join('') : '<p>Other independently matching outlets were not found in this search. Try the source article for more context.</p>';
    list.querySelectorAll<HTMLButtonElement>('.coverage-story').forEach(button=>button.addEventListener('click',()=>void invoke('open_external',{url:related[Number(button.dataset.index)].url}).catch(error=>{const status=c.querySelector('#article-open-status');if(status)status.textContent=`Could not open that outlet: ${String(error)}`;})));
  }).catch(error=>{if(request===coverageRequest){const list=c.querySelector('#coverage-list');if(list)list.textContent=`Related coverage is unavailable: ${String(error)}`;}});
}

async function loadVideos(category = activeVideoCategory) {
  if(refreshing.has('videos'))return;refreshing.add('videos');
  activeVideoCategory=category;
  document.querySelectorAll('#video-tabs button').forEach(b=>b.classList.toggle('active',(b as HTMLElement).dataset.cat===category));
  setRefreshing('video-refresh',true);
  const stage=document.querySelector<HTMLElement>('#video-stage')!;
  try {
    const result:any=await invoke('news_videos_widget',{category});
    videoItems=Array.isArray(result?.items)?result.items:[];
    if(!videoItems.length){stage.innerHTML='<div class="video-empty"><b>No recent videos returned</b><small>Try Refresh later.</small></div>';return;}
    stage.innerHTML=`<div class="video-grid">${videoItems.slice(0,8).map((item,i)=>`<button class="video-tile" data-index="${i}" title="${esc(item.title)}"><img src="${esc(item.thumbnail||'')}" alt=""><span>▶ ${esc(item.title)}</span><small>${esc(item.source||'Video')}</small></button>`).join('')}</div>`;
    stage.querySelectorAll<HTMLButtonElement>('.video-tile').forEach((tile,i)=>tile.addEventListener('click',()=>openNews(videoItems[i])));
  }catch(error){stage.innerHTML=`<div class="video-empty"><b>News videos unavailable</b><small>${esc(error)}</small></div>`;}
  finally{refreshing.delete('videos');setRefreshing('video-refresh',false);}
}

async function markets() {
  if(refreshing.has('markets'))return; refreshing.add('markets');
  setRefreshing('market-refresh', true);
  try {
    const exchange=document.querySelector<HTMLSelectElement>('#market-exchange')?.value||'all';
    const [r,newsResult]:any[]=await Promise.all([invoke('market_widget'),invoke('market_news',{exchange})]);
    marketItems = (r?.items || []).map((x:any)=>({...x,price: x.price ?? (x.close !== null && x.close !== undefined && x.close !== "" && Number.isFinite(Number(x.close)) ? Number(x.close):undefined)}));
    marketNewsItems=Array.isArray(newsResult?.items)?newsResult.items:[];
    const newsTicker=document.querySelector<HTMLElement>('#market-news-ticker');if(newsTicker)newsTicker.innerHTML=marketNewsItems.slice(0,14).map(item=>`<button type="button">${esc(item.title)} <b>${esc(item.source||'Market')}</b></button>`).join('');newsTicker?.querySelectorAll('button').forEach((button,index)=>button.addEventListener('click',()=>openNews(marketNewsItems[index],true)));
    renderMarketIndices();
  } catch(e) { document.querySelector('#market-live')!.textContent='UNAVAILABLE'; document.querySelector('#market-grid')!.textContent=String(e); } finally { refreshing.delete('markets'); setRefreshing('market-refresh', false); }
}

function filteredMarkets(){
  const region=document.querySelector<HTMLSelectElement>('#market-exchange')?.value||'all';
  return marketItems.filter(item=>region==='all'||item.region===region);
}
function renderMarketIndices(){
  const items=filteredMarkets();const grid=document.querySelector<HTMLElement>('#market-grid')!;
  grid.innerHTML=items.map(item=>{const time=item.timestamp?new Date(item.timestamp*1000).toLocaleString(undefined,{month:'short',day:'numeric',hour:'numeric',minute:'2-digit'}):'';const detail=item.price!=null?`${item.source||'Quote'} · ${time}`:item.error?'Feed unavailable':'No quote';const change=Number(item.change_percent);return `<button class="market-card" data-symbol="${esc(item.symbol)}" title="${esc(item.error||detail)}"><div class="market-symbol">${esc((item.name||item.symbol).slice(0,3))}</div><div><b>${esc(item.name||item.symbol)}</b><small>${esc(item.region||'')} · ${esc(detail)}</small></div><strong>${item.price!=null?Number(item.price).toLocaleString(undefined,{maximumFractionDigits:2}):'—'}${Number.isFinite(change)?`<small class="${change>=0?'up':'down'}">${change>=0?'+':''}${change.toFixed(2)}%</small>`:''}</strong></button>`;}).join('')||'<p>No index is available for this exchange selection.</p>';
  grid.querySelectorAll<HTMLButtonElement>('.market-card').forEach(button=>button.addEventListener('click',()=>{const item=marketItems.find(x=>x.symbol===button.dataset.symbol);if(item){selectedMarketIndex=item;void openMarketDetail(item.symbol,item.name||item.symbol);}}));
  document.querySelector('#ticker-tape')!.innerHTML=items.map(item=>`<span>${esc(item.name||item.symbol)} <b>${item.price!=null?Number(item.price).toLocaleString(undefined,{maximumFractionDigits:2}):'—'}</b></span>`).join('');
  const live=items.filter(item=>item.price!=null).length;document.querySelector('#market-live')!.textContent=live?`${live}/${items.length} QUOTES`:'UNAVAILABLE';
}

async function showMarketView(view:string){
  document.querySelectorAll<HTMLButtonElement>('[data-market-view]').forEach(button=>button.classList.toggle('active',button.dataset.marketView===view));
  const grid=document.querySelector<HTMLElement>('#market-grid')!,ticker=document.querySelector<HTMLElement>('#ticker-tape')!,browser=document.querySelector<HTMLElement>('#market-browser')!;
  if(view==='indices'){browser.classList.add('hidden');grid.classList.remove('hidden');ticker.classList.remove('hidden');renderMarketIndices();return;}
  grid.classList.add('hidden');ticker.classList.add('hidden');browser.classList.remove('hidden');
  if(view==='market-news'){
    browser.innerHTML='<p>Loading recent market news and corporate actions…</p>';
    try{const exchange=document.querySelector<HTMLSelectElement>('#market-exchange')?.value||'all';const data:any=await invoke('market_news',{exchange});const items:NewsItem[]=data.items||[];browser.innerHTML=`<h3>Market news · last month</h3><p>${esc(data.scope||'')}</p><div class="company-news-list">${items.map((item,i)=>`<button data-market-news="${i}"><b>${esc(item.title)}</b><small>${esc(item.source||item.published||'Market news')}</small></button>`).join('')||'<p>No recent matching market item was returned.</p>'}</div>`;browser.querySelectorAll<HTMLButtonElement>('[data-market-news]').forEach(button=>button.addEventListener('click',()=>openNews(items[Number(button.dataset.marketNews)],true)));}catch(error){browser.innerHTML=`<p>Market news unavailable: ${esc(error)}</p>`;}return;
  }
  if(view==='corporate-actions'){
    browser.innerHTML='<p>Loading current-week and current-month corporate actions…</p>';
    try{const exchange=document.querySelector<HTMLSelectElement>('#market-exchange')?.value||'all';const data:any=await invoke('market_corporate_actions',{exchange});const items:NewsItem[]=data.items||[];browser.innerHTML=`<h3>Corporate actions · current week and month</h3><p>${esc(data.scope||'')}</p><div class="company-news-list">${items.map((item,i)=>`<button data-corporate-news="${i}"><b>${esc(item.title)}</b><small>${esc(item.source||item.published||'Exchange notice')}</small></button>`).join('')||'<p>No recent corporate-action item was returned.</p>'}</div>`;browser.querySelectorAll<HTMLButtonElement>('[data-corporate-news]').forEach(button=>button.addEventListener('click',()=>openNews(items[Number(button.dataset.corporateNews)],true)));}catch(error){browser.innerHTML=`<p>Corporate actions unavailable: ${esc(error)}</p>`;}return;
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
    browser.querySelectorAll<HTMLButtonElement>('[data-company-news]').forEach(button=>button.addEventListener('click',()=>openNews(items[Number(button.dataset.companyNews)],true)));
  }catch(error){browser.innerHTML=`<p>Company news unavailable: ${esc(error)}</p>`;}
}

async function openMarketDetail(symbol:string,label:string){
  const modal=document.querySelector('#market-modal')!;const content=document.querySelector('#market-detail')!;
  modal.classList.remove('hidden');content.innerHTML=`<h2>${esc(label)}</h2><p>Loading latest available quote and intraday chart…</p>`;
  try{
    const data:any=await invoke('market_detail',{symbol});const quote=data.quote||{};
    const points=(Array.isArray(data.points)?data.points:[]).filter((p:any)=>Array.isArray(p)&&Number.isFinite(Number(p[1]))).map((p:any)=>Number(p[1]));
    const low=Math.min(...points),high=Math.max(...points),span=high-low||1;
    const poly=points.map((p:number,i:number)=>`${(i/Math.max(1,points.length-1)*600).toFixed(1)},${(145-(p-low)/span*125).toFixed(1)}`).join(' ');
    const change=Number(quote.change_percent);
    const tvSymbols:Record<string,string>={'^nsei':'NSE:NIFTY','^bsesn':'BSE:SENSEX','^spx':'SP:SPX','^ndq':'NASDAQ:IXIC','^dji':'TVC:DJI','^ftse':'TVC:UKX','^dax':'XETR:DAX','^nky':'TVC:NI225'};
    const tv=tvSymbols[symbol]||symbol;
    const components:Array<{name:string,symbol:string}>=data.components||[];
    const cells=components.map(c=>`<button data-symbol="${esc(c.symbol)}">${esc(c.name)} <small>${esc(c.symbol)}</small></button>`).join('');
    content.innerHTML=`<h2>${esc(label)}</h2><p class="market-detail-quote">${Number(quote.close).toLocaleString(undefined,{maximumFractionDigits:2})} ${esc(quote.currency||'')} ${Number.isFinite(change)?`<span class="${change>=0?'up':'down'}">${change>=0?'+':''}${change.toFixed(2)}%</span>`:''}</p><p>Latest available quote · ${esc(quote.source||'Yahoo Finance')} · ${quote.timestamp?new Date(Number(quote.timestamp)*1000).toLocaleString():'time unavailable'}</p>${points.length>1?`<svg class="market-chart" viewBox="0 0 600 160" role="img" aria-label="Intraday price chart"><polyline fill="none" stroke="#74e8ff" stroke-width="3" points="${poly}"/></svg>`:'<p>Intraday chart points unavailable for this symbol.</p>'}${components.length?`<h3>Index companies</h3><p>${esc(data.components_note||'')}</p><div class="component-ticker" aria-label="Index companies"><div class="component-ticker-track">${cells}${cells}</div></div><div class="market-components">${cells}</div>`:''}<button id="market-tradingview">Open on TradingView ↗</button>`;
    content.querySelectorAll<HTMLButtonElement>('.component-ticker button,.market-components button').forEach(button=>button.addEventListener('click',()=>void openMarketDetail(button.dataset.symbol||'',button.textContent||'')));
    content.querySelector('#market-tradingview')?.addEventListener('click',()=>void invoke('open_external',{url:`https://www.tradingview.com/symbols/${encodeURIComponent(tv)}/`}).catch(e=>pushMessage(String(e))));
  }catch(error){content.innerHTML=`<h2>${esc(label)}</h2><p>Market detail unavailable: ${esc(error)}</p>`;}
}

function setupCities() {
  let saved:string[]=[];try{saved=JSON.parse(localStorage.getItem('buddy.cities')||'[]');}catch{}
  cityList=[...new Set(saved.map(city=>String(city).trim()).filter(city=>city&&city.toLowerCase()!==profile.location.toLowerCase()))].slice(0,5);
  selectedWeatherCity=localStorage.getItem('buddy.weatherCity')||profile.location;
  if(selectedWeatherCity!==profile.location&&!cityList.includes(selectedWeatherCity))selectedWeatherCity=profile.location;
  const renderButtons=()=>{const bar=document.querySelector<HTMLElement>('#weather-city-buttons')!;const cities=[profile.location,...cityList];bar.innerHTML=cities.map(city=>`<button type="button" data-weather-city="${esc(city)}" class="${city===selectedWeatherCity?'active':''}">${city===profile.location?'⌂ ':''}${esc(city)}</button>`).join('');bar.querySelectorAll<HTMLButtonElement>('[data-weather-city]').forEach(button=>button.addEventListener('click',()=>{selectedWeatherCity=button.dataset.weatherCity||profile.location;localStorage.setItem('buddy.weatherCity',selectedWeatherCity);renderButtons();void weather(selectedWeatherCity,selectedWeatherCity===profile.location);}));};
  renderButtons();
  (document.querySelector<HTMLButtonElement>('#add-city')!).onclick=()=>{if(cityList.length>=5){pushMessage('Weather can keep five additional cities. Remove one from browser storage before adding another.','buddy','Weather');return;}const city=prompt('Add a weather city (your home location will not change)','');if(!city?.trim())return;const value=city.trim();if(value.toLowerCase()===profile.location.toLowerCase()||cityList.some(saved=>saved.toLowerCase()===value.toLowerCase()))return;cityList.push(value);localStorage.setItem('buddy.cities',JSON.stringify(cityList));selectedWeatherCity=value;localStorage.setItem('buddy.weatherCity',value);renderButtons();void weather(value,false);};
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
  try{profile=await invoke<Profile>('profile_save',{profile:updated});document.querySelector('#schedule-modal')!.classList.add('hidden');selectedCalendarDate=new Date(`${date}T12:00:00`);calendarCursor=new Date(selectedCalendarDate.getFullYear(),selectedCalendarDate.getMonth(),1);renderCalendar();renderDaybook();pushMessage(`Added “${title}” to your schedule.`,'buddy','Daybook');}catch(reason){error.textContent=`Could not save: ${String(reason)}`;}
}

async function sendChat() {
  const input = document.querySelector<HTMLInputElement>('#chat-input')!; const text = input.value.trim(); if (!text || responding) return; input.value=''; lastInteractionAt=Date.now(); pushMessage(text,'user');
  if(wantsDailyBriefing(text)||(briefingOfferPending&&/^(yes|yes please|sure|please|okay|ok)$/i.test(text))){await runDailyBriefing('Your requested briefing');return;}
  if(briefingOfferPending&&/^(no|no thanks|not now|later)$/i.test(text)){briefingOfferPending=false;pushMessage('Sure. Ask for a daily briefing whenever you want it.','buddy');return;}
  responding=true; setParrotState('listen');
  try { const t0=performance.now(); const r:any=await invoke('chat',{input:text}); const response=responseWithAction(r); pushMessage(response,'buddy',`${Math.round(performance.now()-t0)} ms`);showPersonaSource(r); document.querySelector('#hero-line')!.textContent=response; if(r.plugin?.plugin_id==='news'&&r.plugin?.data?.items){newsItems=r.plugin.data.items; document.querySelector('#news-live')!.textContent='READY';} setParrotState('happy'); setTimeout(()=>setParrotState('idle'),2500); } catch(e){pushMessage(String(e),'buddy'); setParrotState('idle');} finally{responding=false;}
}

async function nextContextualChatter():Promise<string>{
  const topic=chatterTopic++%5;
  if(topic===0){
    if(!localNewsItems.length||!localEventItems.length)await Promise.all([contextualNews('local'),localEvents()]);
    const headline=firstUsefulHeadline(localNewsItems);
    const event=firstUsefulHeadline(localEventItems);
    if(headline&&event)return `A quick flutter over ${profile.location}: ${headline}. And for something to do, ${event}. Want me to open either one?`;
    if(headline)return `A quick flutter over ${profile.location}: ${headline}. Want me to open the local news?`;
    if(event)return `Something happening around ${profile.location} caught my eye: ${event}. Want the details?`;
  }
  if(topic===1){
    const headline=firstUsefulHeadline(newsItems);
    if(headline)return `Psst, ${profile.name}—one headline worth knowing is: ${headline}. Shall I read more?`;
  }
  if(topic===2){
    if(!nationalNewsItems.length)await contextualNews('national');
    const story=nationalNewsItems.find(item=>/bollywood|film|cinema|actor|actress|movie|music/i.test(`${item.title} ${item.summary||''}`));
    if(story)return `A pinch of entertainment masala: ${spokenHeadline(story.title)}. Say “tell me more” if that caught your eye.`;
  }
  if(topic===3&&latestWeather?.temperature!==null)return `Feather forecast: ${weatherBriefing()} Would you like the full five-day outlook?`;
  return await invoke<string>('chatter');
}

async function offerChatter(automatic=false){
  if(responding || document.hidden || !document.querySelector('#onboarding')?.classList.contains('hidden') || !document.querySelector('#settings-panel')?.classList.contains('hidden'))return;
  if(automatic && (!proactiveChatter || Date.now()-lastInteractionAt<2*60_000 || Date.now()-lastChatterAt<5*60_000))return;
  if(automatic && (!lastVoice?.tts_ready || !['Ready','WakeListening','Stopped'].includes(lastVoice.state) || document.querySelector<HTMLAudioElement>('#audio')?.paused===false))return;
  responding=true;lastChatterAt=Date.now();
  try{
    const words=await nextContextualChatter();
    pushMessage(words,'buddy',automatic?'Companion check-in':'BestBuddy suggestion');
    document.querySelector('#hero-line')!.textContent=words;
    if(lastVoice?.tts_ready && !['Listening','Speaking','Synthesizing'].includes(lastVoice.state)){
      setParrotState('talk');await invoke('voice_speak',{text:words,request:'chatter',capability:'persona.chatter'});
    }
  }catch(error){if(!automatic)pushMessage(`BestBuddy could not start a suggestion: ${String(error)}`);}
  finally{responding=false;setParrotState('idle');}
}

function openVideoPanel(){document.querySelector<HTMLButtonElement>('[data-desktop-panel="video"]')?.click();document.querySelector<HTMLButtonElement>('[data-mobile-panel="video"]')?.click();}
function youtubeVideoId(value:string):string|null{try{const u=new URL(value);if(u.hostname==='youtu.be')return /^[A-Za-z0-9_-]{11}$/.test(u.pathname.slice(1))?u.pathname.slice(1):null;if(['youtube.com','www.youtube.com','m.youtube.com'].includes(u.hostname)){const id=u.searchParams.get('v')||(u.pathname.startsWith('/shorts/')?u.pathname.split('/')[2]:null);return id&&/^[A-Za-z0-9_-]{11}$/.test(id)?id:null;}}catch{}return null;}
function stopVideoPlayback(){
  const stopWithin=(root:Element|null)=>{root?.querySelectorAll<HTMLVideoElement>('video').forEach(video=>{video.pause();video.removeAttribute('src');video.load();});root?.querySelectorAll<HTMLIFrameElement>('iframe').forEach(frame=>{frame.src='about:blank';frame.remove();});};
  const stage=document.querySelector<HTMLElement>('#video-stage');
  stopWithin(stage);stopWithin(document.querySelector('#media-content'));
  if(stage)stage.innerHTML=`<div class="video-empty">${icon('video')}<b>Video stopped</b><small>Choose a news video or search a video source.</small></div>`;
  const content=document.querySelector<HTMLElement>('#media-content');if(content)content.replaceChildren();
  document.querySelector('#media-modal')?.classList.add('hidden');
  return 'Video playback stopped.';
}
function playOrSearchVideo(value:string,provider?:string){const query=value.trim();const selected=provider||document.querySelector<HTMLSelectElement>('#video-provider')?.value||'youtube';openVideoPanel();const id=selected==='youtube'?youtubeVideoId(query):null;if(id){stopVideoPlayback();openVideoPanel();document.querySelector('#video-stage')!.innerHTML=`<iframe class="embedded-video" src="https://www.youtube.com/embed/${esc(id)}?autoplay=1" title="YouTube video player" allow="autoplay; encrypted-media; picture-in-picture" allowfullscreen></iframe>`;return;}const bases:Record<string,{home:string,search:(q:string)=>string}>={youtube:{home:'https://www.youtube.com/',search:q=>`https://www.youtube.com/results?search_query=${encodeURIComponent(q)}`},vimeo:{home:'https://vimeo.com/',search:q=>`https://vimeo.com/search?q=${encodeURIComponent(q)}`},msn:{home:'https://www.msn.com/video',search:q=>`https://www.msn.com/search?q=${encodeURIComponent(`${q} video`)}`}};const source=bases[selected]||bases.youtube;const url=query?source.search(query):source.home;void invoke('open_external',{url}).catch(error=>pushMessage(`Could not open the video source: ${String(error)}`,'buddy'));}

function responseWithAction(response: { capability?: string; text?: string; plugin?: {data?: {query?: string;destination?:string}} }): string {
  if (response.capability === 'music.stop') return musicController?.stop() || 'Music controls are unavailable.';
  if (response.capability === 'music.play') return musicController?.play() || 'Music controls are unavailable.';
  if (response.capability === 'music.pause') return musicController?.pause() || 'Music controls are unavailable.';
  if (response.capability === 'music.next') return musicController?.next() || 'Music controls are unavailable.';
  if (response.capability === 'music.previous') return musicController?.previous() || 'Music controls are unavailable.';
  if (response.capability === 'music.mute') return musicController?.mute(true) || 'Music controls are unavailable.';
  if (response.capability === 'music.unmute') return musicController?.mute(false) || 'Music controls are unavailable.';
  if (response.capability === 'video.open') { openVideoPanel(); void loadVideos(); }
  if (response.capability === 'video.stop') return stopVideoPlayback();
  if (response.capability === 'video.youtube') playOrSearchVideo('','youtube');
  if (response.capability === 'video.search') playOrSearchVideo(response.plugin?.data?.query||'','youtube');
  if (response.capability === 'video.news') { openVideoPanel(); const item=videoItems[0]||newsItems.find(x=>x.video_url); if(item)openNews(item); else void loadVideos(); }
  if (response.capability === 'site.open') void invoke('open_external',{url:response.plugin?.data?.destination||''}).catch(error=>pushMessage(`Could not open that site: ${String(error)}`,'buddy'));
  if (response.capability === 'web.search') void invoke('open_external',{url:`https://www.google.com/search?q=${encodeURIComponent(response.plugin?.data?.destination||'')}`}).catch(error=>pushMessage(`Could not open web search: ${String(error)}`,'buddy'));
  return response.text || 'I’m here with you.';
}

function isSpokenNews(capability: string | undefined, request: string): boolean {
  return capability === 'news.read_continue' || capability === 'news.read_visible' || (capability === 'news.headlines' && /\b(read|headlines)\b/i.test(request));
}

async function handleVoiceEvent(event: VoiceEvent) {
  idleTimer = 0;
  if(event.kind==='error'){pushMessage(event.text||'Voice error');return;}
  if(event.kind==='timeout'){setVoiceTop('No clear speech · try Talk again');return;}
  if(event.kind==='barge_in'){voiceTurn++;void interruptConversation();setVoiceTop('Interrupted · listening');return;}
  if (event.kind === 'wake') { voiceTurn++;await interruptConversation();setParrotState('listen'); setVoiceTop('Listening…'); return; }
  if (event.kind !== 'transcript' || !event.text) return;
  lastInteractionAt=Date.now();
  const turn=++voiceTurn;
  pushMessage(event.text,'user',`${event.asr_ms ?? 0} ms ASR`); setVoiceTop('Thinking…'); setParrotState('happy');
  if(wantsDailyBriefing(event.text)||(briefingOfferPending&&/^(yes|yes please|sure|please|okay|ok)$/i.test(event.text.trim()))){await runDailyBriefing('Your requested briefing');return;}
  if(briefingOfferPending&&/^(no|no thanks|not now|later)$/i.test(event.text.trim())){briefingOfferPending=false;await sayCompanionLine('Sure. Ask for a daily briefing whenever you want it.','Briefing');return;}
  try {
    await conversationInterrupt;if(turn!==voiceTurn)return;const t0=performance.now(); const r:any=await invoke('chat',{input:event.text}); if(turn!==voiceTurn)return; const response=responseWithAction(r); const newsRead=isSpokenNews(r.capability,event.text); if(!newsRead){pushMessage(response,'buddy',`${Math.round(performance.now()-t0)} ms response`);showPersonaSource(r);} document.querySelector('#hero-line')!.textContent=newsRead?'Reading the headlines aloud.':response; setParrotState('talk'); setVoiceTop('Speaking…'); await invoke('voice_speak',{text:response,request:event.text,capability:r.capability||''}); void refreshStatus();
  } catch(e){ if(turn!==voiceTurn)return;pushMessage(`Voice response problem: ${String(e)}`,'buddy'); setVoiceTop('Voice error'); }
  finally { setTimeout(()=>setParrotState('idle'),900); }
}

function setVoiceTop(label: string){ const el=document.querySelector('#voice-top-state'); if(el) el.textContent=label; const summary=document.querySelector('#header-voice-state');if(summary)summary.textContent=label; }
function setupVoiceTuning(){
  const pause=document.querySelector<HTMLInputElement>('#tune-pause')!;
  const limit=document.querySelector<HTMLInputElement>('#tune-limit')!;
  const savedPause=Number(localStorage.getItem('buddy.voice.pauseMs'));
  const savedLimit=Number(localStorage.getItem('buddy.voice.limitMs'));
  if(savedPause>=400&&savedPause<=1800)pause.value=String(savedPause);
  if(savedLimit>=6000&&savedLimit<=30000)limit.value=String(savedLimit);
  const apply=()=>{const p=Number(pause.value),m=Number(limit.value);document.querySelector('#tune-pause-value')!.textContent=`${(p/1000).toFixed(1)} s`;document.querySelector('#tune-limit-value')!.textContent=`${Math.round(m/1000)} s`;localStorage.setItem('buddy.voice.pauseMs',String(p));localStorage.setItem('buddy.voice.limitMs',String(m));void invoke('voice_tune',{pauseToFinishMs:p,maxListeningMs:m}).catch(e=>setVoiceTop(`Timing error: ${e}`));};
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
    report.textContent='Running WAV through local ASR, BestBuddy and speaker…';
    try{
      const result:any=await invoke('voice_test_wav',{wavBytes:Array.from(bytes)});
      const asr=result.asr||{};
      report.textContent=[
        `WAV: ${asr.sample_rate||'—'} Hz · ${asr.frames||0} frames`,
        `Whisper ASR (raw): ${asr.raw_transcript||'(no words)'}`,
        `Whisper ASR (denoised): ${asr.enhanced_transcript??'(denoiser unavailable)'}`,
        `Accepted: ${asr.accepted?'yes':'no'} · ${asr.asr_ms??result.asr_ms??0} ms`,
        `BestBuddy: ${result.response?.text||'(no response because ASR was not accepted)'}`,
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
    const link=document.createElement('a');link.href=playbackUrl;link.download='buddy-microphone-test.wav';link.click();
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
  try {
    const s:AppStatus=await invoke('status');
    profile=s.profile||profile;
    const headerLocation=document.querySelector('#header-location');if(headerLocation)headerLocation.textContent=profile.location||'Location not set';
    const headerRegion=document.querySelector('#header-region');if(headerRegion)headerRegion.textContent=[profile.region,profile.country].filter(Boolean).join(', ');
    const v=s.voice;
    lastVoice=v;
    renderCapture(v);
    setVoiceTop(v.always_on?`${v.state} · ${v.wake_mode}`:'Preparing voice…');
    document.querySelector('#footer-log')!.textContent=`Logs: ${s.log_path}`;
    const eng=document.querySelector<HTMLElement>('#engine-pill');
    if(eng){eng.classList.toggle('warn',!v.always_on||!v.asr_ready||!v.tts_ready);eng.title=`${s.build||'Build unknown'} · ${v.always_on?'wake-word listening':'voice runtime initializing or recovering'}`;}
    const mic=document.querySelector<HTMLElement>('#mic-pill');
    if(mic){mic.classList.toggle('bad',!v.microphone);mic.classList.toggle('warn',!!v.microphone&&v.always_on&&!v.microphone_ready);mic.title=v.microphone?`${v.microphone} · ${v.microphone_ready?'capturing':v.always_on?'capture failed':'detected, capture off'}`:'No default microphone detected';}
    const sp=document.querySelector<HTMLElement>('#speaker-pill');
    if(sp){sp.classList.toggle('bad',!v.speaker);sp.classList.toggle('warn',!!v.speaker&&v.always_on&&!v.speaker_ready);sp.title=v.speaker?`${v.speaker} · ${v.speaker_ready?'output initialized':'detected, output not initialized'}`:'No default speaker detected';}
    if(v.state==='Speaking') setParrotState('talk');
    else if(v.state==='Listening'||v.state==='WakeListening') setParrotState('listen');
    const rep=document.querySelector('#device-report');
    if(rep) rep.innerHTML=`<b>Voice diagnostics · ${esc(s.build||'build unknown')}</b><div>🎙 ${esc(v.microphone || 'No default microphone')} · ${v.microphone_ready?'capture running':v.microphone?v.always_on?'capture not ready':'detected; capture off':'device not detected'}</div><div>🔊 ${esc(v.speaker || 'No default speaker')} · ${v.speaker_ready?'output initialized':v.speaker?'detected; output not initialized':'device not detected'}</div><div>VAD ${v.vad_ready?'✓':'—'} · KWS ${v.kws_ready?'✓':'—'} · denoise ${v.denoise_ready?'✓':'—'} · punctuation ${v.punctuation_ready?'✓':'—'}</div><div>Whisper ASR ${v.asr_ready?'✓':'—'} · TTS ${v.tts_ready?'✓':'—'} · barge-ins ${v.barge_in_count??0}</div><div>Runtime: ${esc(v.state)} · capture ${v.always_on?'active':'off'} · mode ${esc(v.wake_mode)}</div><div>Wake detections ${v.wake_count??0} · KWS decode passes ${v.kws_decode_count??0} · last KWS pass ${v.kws_last_decode_ms??0} ms · dropped mic buffers ${v.dropped_audio_buffers??0}</div><div>Input level ${(Number(v.input_level||0)*100).toFixed(1)}% · VAD ${v.vad_active?'speech':'quiet'} · rejected fragments ${v.rejected_fragments??0}</div><div>Recognition runs after a speech pause.</div><div>Last turn: ${esc(v.last_turn_outcome||'None')}</div><div>Last transcript: ${esc(v.last_transcript||'None')} · ${v.last_voice_ms??0} ms</div><div role="alert">${esc(v.last_error||'No runtime error reported')}</div>`;
  }catch(error){const rep=document.querySelector('#device-report');if(rep)rep.textContent=`Status request failed: ${String(error)}`;}
}

function tickClock(){
  idleTimer += 1;
  if(idleTimer > 180) setParrotState('sleep'); else if(idleTimer % 60 === 0) setParrotState('idle');
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
  if(view==='learning'){void invoke('learning_status').then(showLearning).catch(()=>{});void localLearning.refresh();}
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
  if(localOption) localOption.disabled=!(llmLatestStatus?.local_runtime_ready && (llmLatestStatus?.local_models||[]).some((entry:any)=>entry.id===model));
}
async function loadLlmSettings(syncFields=true){
  const report=document.querySelector<HTMLElement>('#llm-report')!;
  try{
    const status:any=await invoke('llm_status');const s=status.settings||{};
    llmLatestStatus=status;
    if(syncFields){
      (document.querySelector('#llm-provider') as HTMLSelectElement).value=s.provider||'auto';
      (document.querySelector('#llm-local-model') as HTMLSelectElement).value=s.local_model||'qwen3-0.6b';
      (document.querySelector('#llm-online-provider') as HTMLSelectElement).value=s.online_provider||'groq';
      (document.querySelector('#llm-online-model') as HTMLInputElement).value=s.online_model||'';
      document.querySelectorAll<HTMLElement>('[data-provider-row]').forEach(row=>{
        const provider=row.dataset.providerRow;
        const select=row.querySelector<HTMLSelectElement>('select');
        const fixed=row.dataset.fixedModel;
        const matches=provider===s.online_provider&&(fixed?fixed===s.online_model:select?.value===s.online_model);
        const radio=row.querySelector<HTMLInputElement>('input[type="radio"]');if(radio)radio.checked=!!matches;
      });
    }
    updateLocalChoice();
    const configured=Object.entries(status.provider_keys||{}).filter(([,ready])=>ready).map(([name])=>name);
    const models=(status.local_models||[]).map((model:any)=>model.id).join(', ')||'none';
    report.textContent=`Local GGUF runtime: ${status.local_runtime_ready?'found':'missing'} · reusable models: ${models}. ${status.local_preparation_status||''} Credential vault: ${status.vault_unlocked?'unlocked':status.vault_exists?'locked':'not created'} · Available provider keys: ${configured.join(', ')||'none'}.`;
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
    status.textContent=`${models.length} currently verified free text models; ${shown.length} listed. These share one OpenRouter key. If fewer than 15 are available, BestBuddy shows the actual count.`;
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

const llmProviderHelp:Record<string,{keys:string,limits:string,model:string}>={
  groq:{keys:'https://console.groq.com/keys',limits:'https://console.groq.com/docs/rate-limits',model:'openai/gpt-oss-20b'},
  openrouter:{keys:'https://openrouter.ai/keys',limits:'https://openrouter.ai/collections/free-models/',model:'openrouter/free'},
  huggingface:{keys:'https://huggingface.co/settings/tokens',limits:'https://huggingface.co/docs/inference-providers/pricing',model:'openai/gpt-oss-120b:fastest'},
  gemini:{keys:'https://aistudio.google.com/app/apikey',limits:'https://ai.google.dev/gemini-api/docs/rate-limits',model:'gemini-2.5-flash-lite'},
};

function setup(){
  const showDesktopPanel=(name:string)=>{
    document.querySelectorAll<HTMLElement>('.dashboard>.panel').forEach(panel=>panel.classList.toggle('desktop-active',panel.classList.contains(`${name}-panel`)));
    document.querySelectorAll<HTMLButtonElement>('.desktop-widget-nav [data-desktop-panel]').forEach(button=>{const active=button.dataset.desktopPanel===name;button.classList.toggle('active',active);if(active)button.setAttribute('aria-current','page');else button.removeAttribute('aria-current');});
    const selected=document.querySelector<HTMLElement>(`.dashboard>.${name}-panel`);
    if(selected){selected.classList.remove('collapsed');selected.querySelector<HTMLButtonElement>('[data-toggle-panel]')?.setAttribute('aria-expanded','true');selected.tabIndex=-1;selected.focus({preventScroll:true});}
  };
  document.querySelectorAll<HTMLButtonElement>('.desktop-widget-nav [data-desktop-panel]').forEach(button=>button.addEventListener('click',()=>showDesktopPanel(button.dataset.desktopPanel||'chatter')));
  document.querySelectorAll<HTMLElement>('.dashboard>.panel:not(.chatter-panel)').forEach(panel=>{const tools=panel.querySelector<HTMLElement>('.panel-tools');if(!tools)return;const close=document.createElement('button');close.className='widget-close';close.type='button';close.title='Close widget and return to BestBuddy';close.setAttribute('aria-label','Close widget and return to BestBuddy');close.textContent='×';close.addEventListener('click',()=>{if(panel.classList.contains('video-panel'))stopVideoPlayback();showDesktopPanel('chatter');});tools.append(close);});
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
  document.querySelector('#briefing-now')!.addEventListener('click',()=>void runDailyBriefing('Daily briefing'));
  document.querySelector('#daily-briefing-chat')!.addEventListener('click',()=>void runDailyBriefing('Daily briefing'));
  document.querySelectorAll<HTMLButtonElement>('[data-toggle-panel]').forEach(button=>button.addEventListener('click',()=>{const panel=button.closest<HTMLElement>('.collapsible-panel');if(!panel)return;const open=panel.classList.toggle('collapsed')===false;button.setAttribute('aria-expanded',String(open));button.textContent=open?'▴':'▾';}));
  document.querySelector('.market-tabs')!.addEventListener('click',event=>{const button=(event.target as HTMLElement).closest<HTMLButtonElement>('[data-market-view]');if(button)void showMarketView(button.dataset.marketView||'indices');});
  document.querySelector('#market-exchange')!.addEventListener('change',()=>{selectedMarketIndex=null;selectedMarketCompany=null;const view=document.querySelector<HTMLButtonElement>('[data-market-view].active')?.dataset.marketView||'indices';void showMarketView(view);});
  document.querySelector('#news-tabs')!.addEventListener('click',e=>{const b=(e.target as HTMLElement).closest('button'); if(b?.dataset.cat) news(b.dataset.cat);});
  document.querySelector('#video-tabs')!.addEventListener('click',e=>{const b=(e.target as HTMLElement).closest('button'); if(b?.dataset.cat) void loadVideos(b.dataset.cat);});
  document.querySelector('#news-refresh')!.addEventListener('click',()=>news()); document.querySelector('#weather-refresh')!.addEventListener('click',()=>void weather()); document.querySelector('#market-refresh')!.addEventListener('click',markets);
  document.querySelector('#video-open')!.addEventListener('click',()=>{const i=document.createElement('input');i.type='file';i.accept='video/*';i.onchange=()=>{const f=i.files?.[0];if(!f)return;document.querySelector('#video-stage')!.innerHTML=`<video controls autoplay src="${esc(URL.createObjectURL(f))}"></video>`;};i.click();});
  document.querySelector('#video-news')!.addEventListener('click',()=>{const item=videoItems[0]||newsItems.find(x=>x.video_url); if(item)openNews(item); else void loadVideos();});
  const youtubeInput=document.querySelector<HTMLInputElement>('#youtube-query')!;
  document.querySelector('#youtube-search')!.addEventListener('click',()=>playOrSearchVideo(youtubeInput.value));
  document.querySelector('#youtube-open')!.addEventListener('click',()=>playOrSearchVideo(''));
  document.querySelector('#video-stop')!.addEventListener('click',()=>{pushMessage(stopVideoPlayback(),'buddy','Video');});
  document.querySelectorAll<HTMLButtonElement>('[data-video-trend]').forEach(button=>button.addEventListener('click',()=>{youtubeInput.value=button.dataset.videoTrend||'';playOrSearchVideo(youtubeInput.value);}));
  youtubeInput.addEventListener('keydown',event=>{if(event.key==='Enter')playOrSearchVideo(youtubeInput.value);});
  document.querySelectorAll<HTMLButtonElement>('[data-music-provider]').forEach(button=>button.addEventListener('click',()=>{const query=document.querySelector<HTMLInputElement>('#music-online-query')!.value.trim()||'trending music';const provider=button.dataset.musicProvider;const urls:Record<string,string>={youtube:`https://music.youtube.com/search?q=${encodeURIComponent(query)}`,spotify:`https://open.spotify.com/search/${encodeURIComponent(query)}`,soundcloud:`https://soundcloud.com/search?q=${encodeURIComponent(query)}`};void invoke('open_external',{url:urls[provider||'youtube']}).catch(error=>pushMessage(`Could not open music search: ${String(error)}`,'buddy'));}));
  document.querySelector('#talk')!.addEventListener('click',async()=>{voiceTurn++;try{await invoke('voice_arm'); setParrotState('listen'); setVoiceTop('Listening…');}catch(e){pushMessage(String(e),'buddy');}});
  document.querySelector('#send')!.addEventListener('click',sendChat); document.querySelector<HTMLInputElement>('#chat-input')!.addEventListener('keydown',(e)=>{if(e.key==='Enter')sendChat();});
  document.querySelectorAll<HTMLButtonElement>('[data-persona]').forEach(button=>button.addEventListener('click',()=>{document.querySelector<HTMLInputElement>('#chat-input')!.value=button.dataset.persona||'';void sendChat();}));
  document.querySelector('#chatter-now')!.addEventListener('click',()=>void offerChatter());
  document.querySelector<HTMLInputElement>('#proactive-chatter')!.addEventListener('change',event=>{proactiveChatter=(event.target as HTMLInputElement).checked;localStorage.setItem('buddy.proactiveChatter',proactiveChatter?'on':'off');lastChatterAt=Date.now();});
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
    try{await invoke('piper_set_auto',{enabled});settingsResult(enabled?'Automatic voice and pace enabled.':'Fixed voice enabled.');await loadPiperVoices();}
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
    const settings={provider:value('#llm-provider'),online_provider:value('#llm-online-provider'),online_model:value('#llm-online-model'),local_model:value('#llm-local-model')};
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
  document.querySelector('#open-log-folder')!.addEventListener('click',()=>void invoke('open_logs').then(()=>settingsResult('Opened the BestBuddy log folder.')).catch(error=>settingsResult(`Could not open log folder: ${String(error)}`)));
  document.querySelector('#market-close')!.addEventListener('click',()=>document.querySelector('#market-modal')!.classList.add('hidden'));
  document.querySelector('#open-messenger')!.addEventListener('click',()=>void invoke('open_external',{url:'https://www.messenger.com/'}).then(()=>settingsResult('Messenger opened in your browser; sign in there to use your contacts.')).catch(e=>settingsResult(`Could not open Messenger: ${String(e)}`)));
  document.querySelectorAll<HTMLButtonElement>('.connection-open').forEach(button=>button.addEventListener('click',()=>void invoke('open_external',{url:button.dataset.url}).then(()=>settingsResult(`${button.dataset.name} opened in your browser; sign in there to use your account.`)).catch(e=>settingsResult(`Could not open ${button.dataset.name}: ${String(e)}`))));
  document.querySelector('#speaker-test')!.addEventListener('click',async()=>{settingsResult('Sending a test phrase to the selected speaker…');setVoiceTop('Speaking…');setParrotState('talk');try{await invoke('voice_speak',{text:`Hello ${profile.name}. This is BestBuddy. Please confirm that you heard this speaker test.`});settingsResult('Playback command completed. Confirm by listening; this cannot prove the sound was audible.');}catch(e){settingsResult(`Speaker test failed: ${String(e)}`);}finally{void refreshStatus();setParrotState('idle');}});
  document.querySelector('#prefetch')!.addEventListener('click',async()=>{settingsResult('Refreshing conversation sources and live information…');try{const r:any=await invoke('prefetch_now');document.querySelector('#footer-status')!.textContent=r.last_error?`Prefetch: ${r.last_error}`:'Background learning refreshed';await Promise.allSettled([weather(),news(),markets(),loadVideos()]);showSettingsView('learning');settingsResult(r.last_error?`Prefetch reported: ${r.last_error}`:'Prefetch completed; weather, news, markets and video were asked to refresh. Check each widget for its live status.');}catch(e){settingsResult(`Prefetch failed: ${String(e)}`);}});
  document.querySelector('#edit-profile')!.addEventListener('click',()=>{document.querySelector('#settings-panel')!.classList.add('hidden');void companion.open(true);});
}

render(); const localLearning=setupLocalLearning(); setup(); setupVoiceTuning(); setupVoiceSignalTest(); setupDisplay(); musicController=setupMusic(pushMessage);
companion=setupCompanion(()=>profile,p=>{profile=p;},pushMessage,async()=>{
  document.querySelector('#greeting')!.textContent='Hello, '+profile.name+'.';
  renderSettingsProfile();
  const select=document.querySelector<HTMLSelectElement>('#city-select')!;
  if(!Array.from(select.options).some(o=>o.value===profile.location)){const o=document.createElement('option');o.value=o.textContent=profile.location;select.add(o);}select.value=profile.location;
  renderCalendar();renderDaybook();
  setupCities();if(launchBriefingStarted)void Promise.allSettled([refreshHomeWeather(),weather(),news(),markets(),loadVideos()]);else void launchBriefing();
});
setInterval(tickClock,1000);setInterval(refreshStatus,1500);setInterval(pollVoice,250);
setInterval(()=>{if(!document.hidden){void refreshHomeWeather();void weather();void news();void markets();void loadVideos();}},180000);
setInterval(()=>{if(!document.hidden)void markets();},60000);
setInterval(()=>companion.checkReminders(),30000);
setInterval(()=>void offerChatter(true),60_000);
setInterval(()=>void offerScheduledBriefing(),60_000);
document.addEventListener('pointerdown',()=>{lastInteractionAt=Date.now();});
document.addEventListener('keydown',()=>{lastInteractionAt=Date.now();});
document.querySelector('#video-refresh')!.addEventListener('click',()=>{const v=document.querySelector<HTMLVideoElement>('#video-stage video');if(v)v.load();else void loadVideos();});
document.addEventListener('keydown',e=>{if(e.key==='Escape'){const mediaOpen=!document.querySelector('#media-modal')?.classList.contains('hidden');const openModal=Array.from(document.querySelectorAll('.modal')).some(el=>!el.classList.contains('hidden'));if(mediaOpen)stopVideoPlayback();document.querySelectorAll('.modal').forEach(el=>el.classList.add('hidden'));if(!openModal){document.querySelector<HTMLButtonElement>('[data-desktop-panel="chatter"]')?.click();}}});
(async()=>{
  const minimumSplash=new Promise(resolve=>setTimeout(resolve,3500));
  await refreshStatus();
  if(profile.onboarding_complete)launchContextPromise=Promise.all([refreshHomeWeather(),weather(),news('breaking'),markets(),contextualNews('local'),contextualNews('regional'),contextualNews('national'),localEvents(),loadCalendarContext(new Date())]);
  const message=document.querySelector<HTMLElement>('#launch-message');
  if(message)message.textContent=`Welcome, ${profile.name}. I’m tuning my voice and checking ${profile.location}.`;
  const listeningAllowed=Boolean(profile.onboarding_complete&&profile.consents?.microphone&&profile.consents?.voice_processing);
  launchVoiceReady=listeningAllowed?await waitForSpeechReady(12_000):false;
  launchWarmupCompleted=true;
  if(message)message.textContent=launchVoiceReady?'Voice ready—let’s begin.':listeningAllowed?'I’m opening now and will keep preparing voice in the background.':'Microphone listening stays off until you enable it in registration.';
  await minimumSplash;
  hideLaunchSplash();
  await new Promise(resolve=>setTimeout(resolve,650));
  await companion.open();
  companion.checkReminders();
  if(document.querySelector('#onboarding')?.classList.contains('hidden'))await launchBriefing();
})();
