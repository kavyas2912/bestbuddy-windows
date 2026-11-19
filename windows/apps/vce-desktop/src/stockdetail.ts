export type PricePoint = [number,number];
export function tradingViewSymbol(symbol:string):string {
  const indices:Record<string,string>={'^NSEI':'NSE:NIFTY','^BSESN':'BSE:SENSEX','^GSPC':'SP:SPX','^SPX':'SP:SPX','^IXIC':'NASDAQ:IXIC','^NDQ':'NASDAQ:IXIC','^DJI':'TVC:DJI','^FTSE':'TVC:UKX','^GDAXI':'XETR:DAX','^N225':'TVC:NI225'};
  const code=symbol.toUpperCase();if(indices[code])return indices[code];
  const suffixes:Record<string,string>={NS:'NSE',BO:'BSE',L:'LSE',AX:'ASX',TO:'TSX',DE:'XETR',HK:'HKEX',T:'TSE',PA:'EURONEXT',SI:'SGX'};
  const at=code.lastIndexOf('.');const exchange=suffixes[code.slice(at+1)];
  return at>0&&exchange?`${exchange}:${code.slice(0,at)}`:code;
}
export function pricePoints(value:unknown):PricePoint[] {
  if(!Array.isArray(value))return [];
  return value.filter((p):p is PricePoint=>Array.isArray(p)&&p.length>=2&&typeof p[0]==='number'&&typeof p[1]==='number'&&Number.isFinite(p[0])&&Number.isFinite(p[1])&&p[1]>0).sort((a,b)=>a[0]-b[0]);
}
export function dailyIndicators(points:PricePoint[]) {
  const closes=points.map(p=>p[1]);
  const sma=(period:number)=>closes.length>=period?closes.slice(-period).reduce((sum,p)=>sum+p,0)/period:null;
  let rsi:number|null=null;
  if(closes.length>=15) {
    let gain=0,loss=0;
    for(let i=1;i<=14;i++){const delta=closes[i]-closes[i-1];gain+=Math.max(delta,0);loss+=Math.max(-delta,0);}
    gain/=14;loss/=14;
    for(let i=15;i<closes.length;i++){const delta=closes[i]-closes[i-1];gain=(gain*13+Math.max(delta,0))/14;loss=(loss*13+Math.max(-delta,0))/14;}
    rsi=loss===0?(gain===0?50:100):100-100/(1+gain/loss);
  }
  const sma20=sma(20),sma50=sma(50),last=closes.at(-1);
  const trend=last!=null&&sma20!=null?last>sma20?'Above 20-day average':last<sma20?'Below 20-day average':'At 20-day average':'Not enough daily bars';
  return {sma20,sma50,rsi,trend,bars:closes.length,asOf:points.at(-1)?.[0]};
}
