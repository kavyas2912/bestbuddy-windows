type DisplayChoice = 'auto' | 'compact' | 'comfortable';

export function setupDisplay() {
  const settings = document.querySelector('[data-settings-page="status"]');
  const control = document.createElement('label');
  control.className = 'display-setting';
  control.innerHTML = '<span>Display density</span><select id="display-density"><option value="auto">Automatic</option><option value="compact">Compact</option><option value="comfortable">Comfortable</option></select><small id="display-detection"></small>';
  const report = settings?.querySelector('#device-report');
  if (report) settings?.insertBefore(control, report);
  else settings?.append(control);
  const select = control.querySelector<HTMLSelectElement>('#display-density')!;
  const detection = control.querySelector<HTMLElement>('#display-detection')!;
  const saved = localStorage.getItem('buddy.displayDensity');
  select.value = saved === 'compact' || saved === 'comfortable' ? saved : 'auto';
  const update = () => {
    const width = Math.round(window.visualViewport?.width || window.innerWidth);
    const height = Math.round(window.visualViewport?.height || window.innerHeight);
    const automatic = width < 1280 || height < 800 ? 'compact' : 'comfortable';
    const choice = select.value as DisplayChoice;
    const root = document.documentElement;
    root.dataset.density = choice === 'auto' ? automatic : choice;
    root.dataset.viewport = height < 720 ? 'short' : height < 900 ? 'standard' : 'tall';
    root.dataset.desktopLayout = width >= 901 ? 'dashboard' : 'compact-navigation';
    root.style.setProperty('--buddy-viewport-width', `${width}px`);
    root.style.setProperty('--buddy-viewport-height', `${height}px`);
    detection.textContent = `${width} × ${height} usable pixels · ${Math.round(window.devicePixelRatio * 100)}% display scale · ${choice === 'auto' ? automatic : 'manual'} layout · ${root.dataset.viewport} screen`;
  };
  select.addEventListener('change', () => {
    if (select.value === 'auto') localStorage.removeItem('buddy.displayDensity');
    else localStorage.setItem('buddy.displayDensity', select.value);
    update();
  });
  window.addEventListener('resize', update);
  window.visualViewport?.addEventListener('resize', update);
  update();
}
