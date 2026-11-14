import assert from 'node:assert/strict';
import {dailyIndicators,pricePoints,tradingViewSymbol} from '../dist-test/stockdetail.js';
assert.equal(tradingViewSymbol('RELIANCE.NS'),'NSE:RELIANCE');assert.equal(tradingViewSymbol('^NSEI'),'NSE:NIFTY');assert.equal(tradingViewSymbol('BRK.B'),'BRK.B');
assert.deepEqual(pricePoints([[3,2],[1,null],[2,4],[4,NaN],[5,-1]]),[[2,4],[3,2]]);
let result=dailyIndicators(Array.from({length:50},(_,i)=>[i+1,i+1]));
assert.equal(result.sma20,40.5);assert.equal(result.sma50,25.5);assert.equal(result.rsi,100);assert.equal(result.trend,'Above 20-day average');
assert.equal(dailyIndicators(Array.from({length:30},(_,i)=>[i,100-i])).rsi,0);
assert.equal(dailyIndicators(Array.from({length:30},(_,i)=>[i,10])).rsi,50);
assert.equal(dailyIndicators([[1,10]]).rsi,null);assert.equal(dailyIndicators([]).sma20,null);
// Wilder's standard 14-period RSI worksheet gives 70.4641 at the first eligible bar.
const worksheet=[44.34,44.09,44.15,43.61,44.33,44.83,45.10,45.42,45.84,46.08,45.89,46.03,45.61,46.28,46.28];
assert.ok(Math.abs(dailyIndicators(worksheet.map((p,i)=>[i,p])).rsi-70.464135)<0.00001);
console.log('Daily SMA and Wilder RSI calculations, insufficient history and invalid points passed');
