import { invoke } from '@tauri-apps/api/core';

type Source = { name: string; url: string; format: string };
type Policy = { enabled: boolean; interval_minutes: number; sources: Source[] };
type Status = { policy: Policy; catalogue: Source[]; consented: boolean;
  progress: { documents: number; last_success?: string; running: boolean; errors: string[] };
  documents: { title: string; publisher: string; source_url: string; kind: string }[];
  intelligence?: { route: string; elapsed_us: number } };
const escape = (s: unknown) => String(s ?? '').replace(/[&<>"']/g, c => ({'&':'&amp;','<':'&lt;','>':'&gt;','"':'&quot;',"'":'&#39;'}[c]!));

export function setupLocalLearning() {
  const page = document.querySelector<HTMLElement>('[data-settings-page="learning"]')!;
  const panel = document.createElement('div'); panel.className = 'device-report'; page.append(panel);
  let status: Status | null = null;
  let sources: Source[] = [];
  let busy = false;
  const message = (text: string) => { panel.querySelector<HTMLElement>('[data-learning-message]')!.textContent = text; };
  function render(value: Status) {
    status = value;
    sources = [...value.catalogue, ...value.policy.sources.filter(s => !value.catalogue.some(c => c.url === s.url))];
    const consent = value.consented ? '' : '<p>Enable personalization and local learning in Profile &amp; people before importing or fetching material.</p>';
    panel.innerHTML = `<h4>Local intelligence learning</h4><p>Learn vocabulary and retrieve attributed excerpts from interviews, discussions and debates. Source material does not change app actions or become independently verified fact.</p>${consent}
      <form data-learning-policy><label><input data-learning-enabled type="checkbox" ${value.policy.enabled ? 'checked' : ''}> Refresh selected sources while BestBuddy is open</label>
      <label>Minutes between updates<input data-learning-interval type="number" min="15" max="1440" value="${value.policy.interval_minutes}"></label>
      <div>${sources.map((s, i) => `<label class="check"><input data-learning-source="${i}" type="checkbox" ${value.policy.sources.some(p => p.url === s.url) ? 'checked' : ''}>${escape(s.name)}${s.format === 'hn' ? ' · unverified community discussion' : ''}</label>`).join('')}</div>
      <button class="action-btn" type="submit">Save learning choices</button></form>
      <div class="settings-page-actions"><button class="action-btn ghost" data-learning-refresh type="button">Learn now</button><button class="action-btn ghost" data-learning-clear type="button">Clear learned transcripts</button></div>
      <p>${value.progress.documents} learned documents · Last successful update: ${escape(value.progress.last_success ? new Date(value.progress.last_success).toLocaleString() : 'Not run yet')}${value.progress.running ? ' · Updating sources' : ''}</p>
      <div data-learning-message role="status">${escape(value.progress.errors.join(' · '))}</div>
      <details><summary>Add a transcript source</summary><form data-learning-add><label>Publisher or source name<input data-learning-name required maxlength="150"></label><label>Public transcript URL<input data-learning-url type="url" required placeholder="https://"></label><label>Source format<select data-learning-format><option value="text">Plain transcript text</option><option value="json">Structured transcript feed</option></select></label><button class="action-btn ghost" type="submit">Add source</button></form></details>
      <details><summary>Import a transcript</summary><p>Choose a text transcript or a structured transcript JSON file. Add attribution for text imports.</p><label>Transcript title<input data-import-title maxlength="500"></label><label>Publisher<input data-import-publisher maxlength="150"></label><label>Original public URL<input data-import-url type="url" placeholder="https://"></label><label>Type<select data-import-kind><option value="transcript">Transcript</option><option value="debate">Debate</option><option value="discussion">Discussion</option><option value="interview">Interview</option><option value="social">Social discussion</option></select></label><input data-learning-file type="file" accept=".txt,.json,text/plain,application/json"></details>
      <details><summary>Learned source links</summary>${value.documents.map(d => `<p>${escape(d.title)} · ${escape(d.publisher)} · ${escape(d.kind)}<br><a href="${escape(d.source_url)}" data-learning-link>${escape(d.source_url)}</a></p>`).join('') || '<p>No transcripts imported yet.</p>'}</details>`;
    panel.querySelector<HTMLFormElement>('[data-learning-policy]')!.onsubmit = async e => {
      e.preventDefault(); await run(() => invoke<Status>('local_learning_configure', { policy: readPolicy() }));
    };
    panel.querySelector<HTMLFormElement>('[data-learning-add]')!.onsubmit = async e => {
      e.preventDefault(); const policy = readPolicy();
      const source = { name: input('[data-learning-name]'), url: input('[data-learning-url]'), format: input('[data-learning-format]') };
      if (policy.sources.some(s => s.url === source.url)) { message('That source is already selected.'); return; }
      policy.sources.push(source); await run(() => invoke<Status>('local_learning_configure', { policy }));
    };
    panel.querySelector<HTMLButtonElement>('[data-learning-refresh]')!.onclick = () => void run(() => invoke<Status>('local_learning_refresh'));
    panel.querySelector<HTMLButtonElement>('[data-learning-clear]')!.onclick = () => void run(() => invoke<Status>('local_learning_clear'));
    panel.querySelector<HTMLInputElement>('[data-learning-file]')!.onchange = async e => {
      const file = (e.target as HTMLInputElement).files?.[0]; if (!file) return;
      if (file.size > 5_000_000) { message('Choose a file smaller than 5 MB.'); return; }
      await run(async () => {
        const text = await file.text();
        let documents;
        if (file.name.toLowerCase().endsWith('.json')) { const value = JSON.parse(text); documents = Array.isArray(value) ? value : value.documents; }
        else documents = [{ id: crypto.randomUUID(), title: input('[data-import-title]') || file.name, publisher: input('[data-import-publisher]'), source_url: input('[data-import-url]'), kind: input('[data-import-kind]'), published_at: '', text }];
        return invoke<Status>('local_learning_import', { documents });
      });
    };
    panel.querySelectorAll<HTMLAnchorElement>('[data-learning-link]').forEach(link => { link.onclick = e => { e.preventDefault(); void invoke('open_external', { url: link.href }).catch(error => message(String(error))); }; });
  }
  function input(selector: string) { return panel.querySelector<HTMLInputElement | HTMLSelectElement>(selector)!.value.trim(); }
  function readPolicy(): Policy { return { enabled: panel.querySelector<HTMLInputElement>('[data-learning-enabled]')!.checked, interval_minutes: Number(input('[data-learning-interval]')), sources: Array.from(panel.querySelectorAll<HTMLInputElement>('[data-learning-source]:checked')).map(el => sources[Number(el.dataset.learningSource)]) }; }
  async function run(action: () => Promise<Status>) {
    if (busy) return; busy = true; message('Working on local learning…');
    panel.querySelectorAll<HTMLButtonElement>('button').forEach(button => { button.disabled = true; });
    try { render(await action()); } catch (error) { message(String(error)); }
    finally { busy = false; panel.querySelectorAll<HTMLButtonElement>('button').forEach(button => { button.disabled = false; }); }
  }
  async function refresh() { if (busy) return; try { render(await invoke<Status>('local_learning_status')); } catch (error) { panel.textContent = `Learning status unavailable: ${String(error)}`; } }
  // Keep the source progress visible without replacing unsaved form choices.
  setInterval(() => { if (status?.progress.running && !busy && !page.classList.contains('hidden')) void refresh(); }, 5000);
  return { refresh };
}
