export type AppTheme=string;
const names=['signature','darkly','flatly','sapphire-gold','solar-yellow','ice-blue','emerald-glass','midnight-gold','electric-blue','champagne','aurora','marigold-ocean','jade-mist','rose-quartz'];
const light=['flatly','solar-yellow','ice-blue','champagne','marigold-ocean','jade-mist','rose-quartz'];
export function applyTheme(value:string):AppTheme{const theme=names.includes(value)?value:'signature';document.documentElement.dataset.appTheme=theme;document.documentElement.dataset.bsTheme=light.includes(theme)?'light':'dark';try{localStorage.setItem('app-appearance-v1',theme);}catch{}return theme;}
export function initializeTheme(){let value='signature';try{value=localStorage.getItem('app-appearance-v1')||value;}catch{}applyTheme(value);}
export function bindThemePicker(){const select=document.querySelector<HTMLSelectElement>('#app-theme');if(!select)return;select.value=document.documentElement.dataset.appTheme||'signature';select.addEventListener('change',()=>applyTheme(select.value));}
