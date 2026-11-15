/** Android date fields support both a normal keyboard and the native calendar. */
export function installKeyboardDates(){
 const enhance=()=>document.querySelectorAll<HTMLInputElement>('input[type="date"]:not([data-native-calendar])').forEach(input=>{
  const picker=document.createElement('input');picker.type='date';picker.dataset.nativeCalendar='true';picker.tabIndex=-1;picker.className='native-calendar-input';picker.setAttribute('aria-label','Choose date from calendar');
  const button=document.createElement('button');button.type='button';button.textContent='Calendar';button.className='date-picker-button';button.setAttribute('aria-label','Choose date from calendar');
  const row=document.createElement('span');row.className='keyboard-date-row';input.before(row);row.append(input,button,picker);
  input.type='text';input.inputMode='text';input.placeholder='YYYY-MM-DD';input.pattern='[0-9]{4}-[0-9]{2}-[0-9]{2}';input.title='Enter a date as YYYY-MM-DD, for example 1990-05-24, or choose Calendar.';
  const validate=()=>{const value=input.value;const date=new Date(value+'T12:00:00Z');const valid=!value||(/^\d{4}-\d{2}-\d{2}$/.test(value)&&Number.isFinite(date.getTime())&&date.toISOString().slice(0,10)===value&&(!input.min||value>=input.min)&&(!input.max||value<=input.max));input.setCustomValidity(valid?'':'Enter a valid date as YYYY-MM-DD within the allowed dates.');};
  input.addEventListener('input',validate);input.addEventListener('change',validate);
  button.onclick=()=>{picker.min=input.min;picker.max=input.max;picker.value=input.value;try{picker.showPicker();}catch{picker.click();}};
  picker.onchange=()=>{input.value=picker.value;input.dispatchEvent(new Event('input',{bubbles:true}));input.dispatchEvent(new Event('change',{bubbles:true}));input.focus();};
 });
 enhance();new MutationObserver(enhance).observe(document.body,{childList:true,subtree:true});
}
