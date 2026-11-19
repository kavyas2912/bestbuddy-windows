import assert from 'node:assert/strict';
import {briefingSections,briefingSelection,headlineText,categoryOrder,inTimeWindow,listeningGaps} from '../dist-test/briefing.js';
const rows=categoryOrder.map(category=>({category,items:Array.from({length:1500},(_,i)=>({title:`${category} report ${i} as council confirms new development`,url:`https://example.com/${category}/${i}`,published:new Date(1700000000000+i*1000).toISOString()}))}));
const sections=briefingSections(rows);assert.equal(sections.length,14);for(const row of sections)assert.equal(row.items.length,['world','national','local'].includes(row.category)?5:2);assert.equal(briefingSelection(rows).length,37);
const duplicate={title:'Council approves a new city transport plan',url:'https://example.com/shared'};assert.equal(briefingSelection([{category:'world',items:[duplicate]},{category:'local',items:[duplicate]}]).length,1);
assert.equal(briefingSelection([{category:'local',items:[{title:'Home - Latest News',url:'https://example.com/'}]}]).length,0);
assert.equal(headlineText({title:'A proper article - BBC',source:'BBC',url:'x'}),'A proper article');assert.ok(inTimeWindow('23:00','22:00','07:00'));assert.ok(listeningGaps[0]===5500);
console.log('Briefing per-category caps, duplicate removal and article-only selection passed');
