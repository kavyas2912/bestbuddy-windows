export type WatchEntry = {symbol:string;name:string;exchange:string};
export type Exchange = {id:string;name:string;country:string;symbol_suffix?:string};
export type CsvReview = {entries:WatchEntry[];warnings:string[];duplicates:number};

function csvRows(input:string):string[][] {
  if(input.length>262144) throw new Error('Choose a CSV smaller than 256 KB.');
  const text=input.replace(/^\uFEFF/,'');
  const firstLine=text.split(/\r?\n/,1)[0];
  const separator=firstLine.includes('\t')?'\t':firstLine.includes(';')&&!firstLine.includes(',')?';':',';
  const rows:string[][]=[];let row:string[]=[];let cell='';let quoted=false;let closed=false;
  const finishCell=()=>{row.push(cell.trim());cell='';closed=false;};
  const finishRow=()=>{finishCell();if(row.some(value=>value))rows.push(row);row=[];if(rows.length>1001)throw new Error('Choose a CSV with at most 1,000 stock rows.');};
  for(let i=0;i<text.length;i++) {
    const ch=text[i];
    if(quoted){if(ch==='"'){if(text[i+1]==='"'){cell+='"';i++;}else{quoted=false;closed=true;}}else cell+=ch;continue;}
    if(ch==='"'){if(cell.trim()||closed)throw new Error('Unexpected quote in CSV. Quote a complete field.');cell='';quoted=true;}
    else if(ch===separator)finishCell();
    else if(ch==='\r'||ch==='\n'){finishRow();if(ch==='\r'&&text[i+1]==='\n')i++;}
    else if(closed&&!/\s/.test(ch))throw new Error('Unexpected text after a quoted CSV field.');
    else cell+=ch;
  }
  if(quoted)throw new Error('The CSV has an unclosed quoted field.');
  if(cell||row.length)finishRow();
  return rows;
}

function exchangeFor(value:string,exchanges:Exchange[]):Exchange|undefined {
  const key=value.trim().toLowerCase();
  const aliases:Record<string,string>={nse:'India',bse:'India',in:'India',nasdaq:'USA',nyse:'USA',us:'USA','united states':'USA',lse:'UK',gb:'UK','united kingdom':'UK',asx:'Australia',tsx:'Canada',xetra:'Germany',hkex:'Hong Kong',sgx:'Singapore',tse:'Japan',b3:'Brazil',euronext:'France'};
  const id=(aliases[key]||value).toLowerCase();
  return exchanges.find(row=>row.id.toLowerCase()===id||row.country.toLowerCase()===id||row.name.toLowerCase()===key);
}

export function normalizeWatchEntry(raw:string,name:string,exchange:string,exchanges:Exchange[]):WatchEntry {
  let symbol=raw.trim().toUpperCase();let selected=exchangeFor(exchange,exchanges);
  if(symbol.includes(':')) {
    const parts=symbol.split(':');
    if(parts.length!==2)throw new Error('Use a ticker such as RELIANCE.NS or NSE:RELIANCE.');
    selected=exchangeFor(parts[0],exchanges);
    if(!selected)throw new Error(`Unknown exchange ${parts[0]}.`);
    exchange=parts[0];symbol=parts[1];
  }
  if(!selected)throw new Error(`Unknown exchange ${exchange}. Choose a supported exchange.`);
  if(!/^[A-Z0-9.^=\-]{1,32}$/.test(symbol)||!/[A-Z0-9]/.test(symbol))throw new Error(`Invalid ticker ${symbol||'(empty)'}.`);
  const suffix=exchange.trim().toLowerCase()==='bse'?'.BO':selected.symbol_suffix||'';
  if(selected.id==='Hong Kong'&&/^\d{1,4}$/.test(symbol))symbol=symbol.padStart(4,'0');
  if(!symbol.startsWith('^')&&!symbol.includes('.')&&!symbol.includes('='))symbol+=suffix;
  if(symbol.length>32)throw new Error('Ticker exceeds 32 characters after adding the exchange suffix.');
  return {symbol,name:(name.trim()||symbol).slice(0,120),exchange:selected.id};
}

export function reviewWatchlistCsv(text:string,defaultExchange:string,exchanges:Exchange[]):CsvReview {
  const rows=csvRows(text);if(!rows.length)throw new Error('This CSV is empty.');
  const key=(s:string)=>s.toLowerCase().replace(/[\s_\-]/g,'');
  const headers=rows[0].map(key);
  const symbolColumn=headers.findIndex(s=>['symbol','ticker','tradingsymbol','stocksymbol'].includes(s));
  const hasHeader=symbolColumn>=0;
  const nameColumn=hasHeader?headers.findIndex(s=>['name','company','companyname','displayname','description'].includes(s)):-1;
  const exchangeColumn=hasHeader?headers.findIndex(s=>['exchange','market','stockexchange'].includes(s)):-1;
  const entries:WatchEntry[]=[];const warnings:string[]=[];const seen=new Set<string>();let duplicates=0;
  rows.slice(hasHeader?1:0).forEach((row,index)=>{
    try {
      if(!hasHeader&&row.length>1)throw new Error('For multiple columns, include a Symbol header.');
      const item=normalizeWatchEntry(row[hasHeader?symbolColumn:0]||'',nameColumn>=0?row[nameColumn]||'':'',exchangeColumn>=0?row[exchangeColumn]||defaultExchange:defaultExchange,exchanges);
      if(seen.has(item.symbol)){duplicates++;return;}
      seen.add(item.symbol);entries.push(item);
    }catch(error){warnings.push(`Row ${index+(hasHeader?2:1)}: ${(error as Error).message}`);}
  });
  if(!entries.length)throw new Error(warnings.join(' ')||'No stock symbols were found.');
  return {entries,warnings,duplicates};
}

export function mergeWatchlist(existing:WatchEntry[],incoming:WatchEntry[],replace=false):WatchEntry[] {
  const merged=new Map<string,WatchEntry>();
  for(const entry of [...(replace?[]:existing),...incoming])if(!merged.has(entry.symbol.toUpperCase()))merged.set(entry.symbol.toUpperCase(),entry);
  if(merged.size>50)throw new Error(`This would create ${merged.size} symbols. Each watchlist supports 50; reduce the CSV or choose another list.`);
  return [...merged.values()];
}
