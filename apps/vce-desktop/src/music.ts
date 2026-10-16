type SavedLibrary = { id: 'library'; files: File[]; savedAt: number };

function libraryDb(): Promise<IDBDatabase> {
  return new Promise((resolve, reject) => {
    const request = indexedDB.open('buddy-music-library', 1);
    request.onupgradeneeded = () => request.result.createObjectStore('libraries', { keyPath: 'id' });
    request.onsuccess = () => resolve(request.result);
    request.onerror = () => reject(request.error);
  });
}

async function libraryStore(value?: SavedLibrary | null): Promise<SavedLibrary | undefined> {
  const db = await libraryDb();
  try {
    return await new Promise((resolve, reject) => {
      const tx = db.transaction('libraries', value === undefined ? 'readonly' : 'readwrite');
      const store = tx.objectStore('libraries');
      const request = value === undefined ? store.get('library') : value === null ? store.delete('library') : store.put(value);
      request.onsuccess = () => resolve(value === undefined ? request.result as SavedLibrary | undefined : undefined);
      request.onerror = () => reject(request.error);
      tx.onabort = () => reject(tx.error);
    });
  } finally { db.close(); }
}

export function setupMusic(notify: (message: string) => void) {
  const audio = document.querySelector<HTMLAudioElement>('#audio')!;
  let tracks: File[] = [];
  let index = -1;
  let url = '';
  let shuffle = false;
  let repeat: 'off' | 'all' | 'one' = 'off';
  const panel = document.querySelector('.music-panel')!;
  panel.insertAdjacentHTML('beforeend', `<div class="library-controls"><button id="music-folder">Open music folder</button><button id="music-save">Save library on this device</button><button id="music-clear">Clear saved library</button><button id="music-previous">Previous</button><button id="music-shuffle" aria-pressed="false">Shuffle</button><button id="music-repeat">Repeat: off</button><label>Seek<input id="music-seek" type="range" min="0" max="100" value="0"></label><label>Volume<input id="music-volume" type="range" min="0" max="1" step="0.01" value="1"></label><button id="music-mute" aria-pressed="false">Mute</button><label class="music-search-label">Find track, lyric phrase or film<input id="music-search" type="search" placeholder="Search your library or open a service"></label><button id="music-spotify">Search Spotify ↗</button><button id="music-youtube">Search YouTube Music ↗</button><label>Queue<select id="music-queue" size="4"></select></label><span id="music-time">0:00 / 0:00</span></div>`);
  const queue = document.querySelector<HTMLSelectElement>('#music-queue')!;
  const search = document.querySelector<HTMLInputElement>('#music-search')!;
  const seek = document.querySelector<HTMLInputElement>('#music-seek')!;
  const renderQueue = () => {
    const query=search.value.trim().toLocaleLowerCase();
    queue.replaceChildren(...tracks.map((file, position) => {
      if(query && !(file.webkitRelativePath || file.name).toLocaleLowerCase().includes(query)) return null;
      const option = document.createElement('option');
      option.value = String(position);
      option.textContent = file.webkitRelativePath || file.name;
      return option;
    }).filter((option):option is HTMLOptionElement=>option!==null));
    if(index>=0) queue.value=String(index);
  };
  const setQueue = (files: File[]) => {
    tracks = files.filter(file => file.type.startsWith('audio/') || /\.(mp3|wav|flac|ogg|m4a|aac|opus)$/i.test(file.name))
      .sort((a, b) => (a.webkitRelativePath || a.name).localeCompare(b.webkitRelativePath || b.name));
    renderQueue();
    document.querySelector('#track-meta')!.textContent = `${tracks.length} tracks in queue`;
  };
  const play = async (position: number) => {
    if (position < 0 || position >= tracks.length) return;
    audio.pause();
    if (url) URL.revokeObjectURL(url);
    index = position;
    url = URL.createObjectURL(tracks[position]);
    audio.src = url;
    queue.value = String(position);
    document.querySelector('#track-name')!.textContent = tracks[position].name;
    document.querySelector('#track-meta')!.textContent = `${position + 1} of ${tracks.length} · ${tracks[position].webkitRelativePath || 'Local file'}`;
    try { await audio.play(); } catch (error) { notify(`Cannot play ${tracks[position].name}: ${error}`); }
  };
  const next = (automatic = false) => {
    if (!tracks.length) return;
    if (automatic && repeat === 'one') { void play(index); return; }
    let position = index + 1;
    if (shuffle && tracks.length > 1) position = (index + 1 + Math.floor(Math.random() * (tracks.length - 1))) % tracks.length;
    if (position >= tracks.length) {
      if (repeat === 'all' || !automatic) position = 0;
      else return;
    }
    void play(position);
  };
  const choose = (folder: boolean) => {
    const input = document.createElement('input');
    input.type = 'file';
    input.multiple = true;
    input.accept = 'audio/*,.mp3,.wav,.flac,.ogg,.m4a,.aac,.opus';
    if (folder) input.setAttribute('webkitdirectory', '');
    input.onchange = () => {
      setQueue(Array.from(input.files || []));
      if (tracks.length) void play(0);
      else notify('No supported audio files were found in that selection.');
    };
    input.click();
  };
  document.querySelector('#music-open')!.addEventListener('click', () => choose(false));
  document.querySelector('#music-folder')!.addEventListener('click', () => choose(true));
  search.addEventListener('input',renderQueue);
  const openSearch=(site:'spotify'|'youtube')=>{
    const query=search.value.trim();
    if(!query){notify('Enter a track, lyric phrase, artist or film to search.');return;}
    const url=site==='spotify' ? `https://open.spotify.com/search/${encodeURIComponent(query)}` : `https://music.youtube.com/search?q=${encodeURIComponent(query)}`;
    void invoke('open_external',{url}).catch(error=>notify(`Could not open ${site}: ${error}`));
  };
  document.querySelector('#music-spotify')!.addEventListener('click',()=>openSearch('spotify'));
  document.querySelector('#music-youtube')!.addEventListener('click',()=>openSearch('youtube'));
  document.querySelector('#music-refresh')!.addEventListener('click', () => choose(true));
  document.querySelector('#music-save')!.addEventListener('click', async () => {
    if (!tracks.length) { notify('Choose tracks or a folder before saving the library.'); return; }
    try { await libraryStore({ id: 'library', files: tracks, savedAt: Date.now() }); notify(`Saved ${tracks.length} tracks on this device.`); }
    catch (error) { notify(`Could not save library; the device may be out of storage: ${error}`); }
  });
  document.querySelector('#music-clear')!.addEventListener('click', async () => {
    try { await libraryStore(null); notify('Saved music library removed from this device.'); }
    catch (error) { notify(`Could not clear saved library: ${error}`); }
  });
  void libraryStore().then(saved => {
    if (saved?.files.length && !tracks.length) { setQueue(saved.files); notify(`Restored ${tracks.length} saved tracks. Press Play to start.`); }
  }).catch(error => notify(`Saved music library is unavailable: ${error}`));
  document.querySelector('#music-next')!.addEventListener('click', () => next());
  document.querySelector('#music-previous')!.addEventListener('click', () => { if (audio.currentTime > 3) audio.currentTime = 0; else void play(Math.max(0, index - 1)); });
  document.querySelector('#music-play')!.addEventListener('click', () => { if (!audio.src) { if (tracks.length) void play(0); else choose(false); return; } if (audio.paused) audio.play().catch(error => notify(String(error))); else audio.pause(); });
  document.querySelector('#music-shuffle')!.addEventListener('click', event => { shuffle = !shuffle; (event.currentTarget as HTMLElement).setAttribute('aria-pressed', String(shuffle)); });
  document.querySelector('#music-repeat')!.addEventListener('click', event => { repeat = repeat === 'off' ? 'all' : repeat === 'all' ? 'one' : 'off'; (event.currentTarget as HTMLElement).textContent = `Repeat: ${repeat}`; });
  document.querySelector('#music-mute')!.addEventListener('click', event => { audio.muted = !audio.muted; (event.currentTarget as HTMLElement).setAttribute('aria-pressed', String(audio.muted)); });
  document.querySelector('#music-volume')!.addEventListener('input', event => audio.volume = Number((event.target as HTMLInputElement).value));
  seek.addEventListener('input', () => { if (Number.isFinite(audio.duration)) audio.currentTime = audio.duration * Number(seek.value) / 100; });
  queue.addEventListener('change', () => void play(Number(queue.value)));
  const time = (seconds: number) => Number.isFinite(seconds) ? `${Math.floor(seconds / 60)}:${String(Math.floor(seconds % 60)).padStart(2, '0')}` : '0:00';
  audio.addEventListener('timeupdate', () => { seek.value = String(audio.duration ? audio.currentTime / audio.duration * 100 : 0); document.querySelector('#music-time')!.textContent = `${time(audio.currentTime)} / ${time(audio.duration)}`; });
  audio.addEventListener('ended', () => next(true));
  audio.addEventListener('error', () => notify(`Audio format unavailable: ${tracks[index]?.name || 'track'}. Choose another track in the queue.`));
  audio.addEventListener('play', () => { document.querySelector('#eq')!.classList.add('playing'); document.querySelector('#music-play')!.textContent = 'Ⅱ'; });
  audio.addEventListener('pause', () => { document.querySelector('#eq')!.classList.remove('playing'); document.querySelector('#music-play')!.textContent = '▶'; });
  window.addEventListener('beforeunload', () => { if (url) URL.revokeObjectURL(url); });
  return {
    play: () => {
      if(!tracks.length)return 'Load a local music folder first.';
      if(!audio.src)void play(0);
      else void audio.play().catch(error=>notify(`Could not play music: ${error}`));
      return 'Starting local music.';
    },
    pause: () => {
      if(!audio.src||audio.paused)return 'No local music is playing.';
      audio.pause();
      return 'Music paused.';
    },
    next: () => {
      if(!tracks.length)return 'Load a local music folder first.';
      next();
      return 'Selecting the next track.';
    },
    previous: () => {
      if(!tracks.length)return 'Load a local music folder first.';
      if(audio.currentTime>3)audio.currentTime=0;
      else void play(Math.max(0,index-1));
      return 'Selecting the previous track.';
    },
    mute: (muted:boolean) => {
      if(!audio.src)return 'No local music is loaded.';
      audio.muted=muted;
      document.querySelector('#music-mute')!.setAttribute('aria-pressed',String(muted));
      return muted?'Music muted.':'Music unmuted.';
    },
    stop: () => {
      if (!audio.src || audio.paused && audio.currentTime === 0) return 'No local music is playing.';
      audio.pause();
      audio.currentTime = 0;
      seek.value = '0';
      return 'Music stopped.';
    },
  };
}
import { invoke } from '@tauri-apps/api/core';
