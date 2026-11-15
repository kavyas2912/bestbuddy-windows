import assert from 'node:assert/strict';
import {groupStories as rawGroups,articleExcerpt,isReadableStory,newestStories,sameStory,quoteAge,isArticleHeadline,canonicalStoryUrl,partitionStories} from '../dist-test/newsroom.js';
const excerpt='The authority published its decision following a detailed review of current economic conditions and the expected impact on households.';
const groupStories=items=>rawGroups(items.map(item=>({summary:excerpt,...item})));
const partitionWithContent=feeds=>partitionStories(feeds.map(feed=>({...feed,items:feed.items.map(item=>({summary:excerpt,...item}))})));
const a={title:'Central bank raises interest rate to 6 percent - BBC',url:'https://bbc.com/a',source:'BBC'};
const b={title:'Central bank raises interest rate to 6 percent - Reuters',url:'https://reuters.com/b',source:'Reuters'};
const groups=groupStories([a,b,a]);assert.equal(groups.length,1);assert.equal(groups[0].coverage.length,2);
assert.equal(sameStory(a,{...b,title:'Central bank raises interest rate to 7 percent'}),false);
assert.equal(sameStory(a,{...b,title:'Central bank denies interest rate rise to 6 percent'}),false);
assert.equal(sameStory({title:'Bank profits rise - London',url:'https://one.com'},{title:'Bank profits rise - Paris',url:'https://two.com'}),false);
assert.equal(quoteAge(undefined),'Quote time unavailable');assert.equal(quoteAge(100,160000),'1 min old');
assert.equal(groupStories([{title:'Science archive',url:'https://science.com'}]).length,0);
console.log('Related coverage, distinct-story and quote-age regressions passed');

const bulk=Array.from({length:2000},(_,index)=>({title:`Report ${index} industry sector ${index+10000} update`,url:`https://publisher.test/${index}`}));assert.equal(groupStories(bulk).length,2000);

assert.equal(isArticleHeadline({title:'Latest News - BBC',source:'BBC',url:'https://bbc.com/news'}),false);
assert.equal(groupStories([{title:'Business news',url:'https://example.com/business'}]).length,0);
assert.equal(isArticleHeadline({title:'Bank cuts rates after inflation eases',url:'https://example.com/article/123'}),true);

// Navigation pages cannot masquerade as articles through a convincing title.
for(const url of ['https://example.com/','https://example.com/news/world/','https://example.com/section/business/','https://example.com/tag/science','https://example.com/search?q=bank','javascript:alert(1)'])assert.equal(isArticleHeadline({title:'Government announces major investment in transport',url}),false,url);
assert.equal(isArticleHeadline({title:'Markets fall',url:'https://example.com/?p=42'}),true);
assert.equal(isArticleHeadline({title:'चंद्रमा पर नए खनिज मिले',url:'https://example.com/science/article/123'}),true);
assert.equal(isArticleHeadline({title:'Headline with confirmed missing article',url:'https://example.com/report/123',article_status:'unavailable'}),false);
const tracked={title:'Factory announces new battery production facility',url:'https://www.example.com/story/factory?utm_source=mail&article_id=42#comments'};
assert.equal(canonicalStoryUrl(tracked.url),'https://example.com/story/factory?article_id=42');
assert.equal(groupStories([tracked,{...tracked,url:'https://example.com/story/factory?article_id=42&fbclid=abc'}]).length,1);
assert.equal(groupStories([tracked,{...tracked,url:'https://example.com/story/factory?article_id=42&fbclid=abc'}])[0].coverage.length,1);
assert.notEqual(canonicalStoryUrl('https://example.com/?p=42'),canonicalStoryUrl('https://example.com/?p=43'));
assert.equal(sameStory({...tracked,title:'Exports rise by 5 percent'}, {...tracked,url:'https://other.com/report/43',title:'Exports fall by 5 percent'}),false);
const report={title:'Factory announces new battery production facility',url:'https://publisher.com/story/factory'};
const feeds=[{category:'national',items:[report]},{category:'business',items:[{...report,url:'https://tv.com/reports/factory'}]}];
const first=partitionWithContent(feeds),reversed=partitionWithContent([...feeds].reverse());
assert.equal(first.get('national').length,0);assert.equal(first.get('business').length,1);assert.equal(first.get('business')[0].coverage.length,2);assert.deepEqual([...first.get('business')],[...reversed.get('business')]);
assert.equal(partitionWithContent([{category:'science',items:[{title:'Scientists discover mineral deposits on the Moon',url:'https://paper.com/report/moon'}]},{category:'business',items:[{title:'Central bank cuts rates after inflation eases',url:'https://paper.com/report/rates'}]}]).get('science').length,1,'Same publisher does not imply same story or topic');
console.log('Article URLs, tracking identity, multilingual reports and deterministic cross-tab coverage passed');

const valid={title:'Council approves transport funding after public consultation',url:'https://publisher.test/report/transport',source:'Publisher',summary:excerpt,published:'2026-10-26T12:00:00Z'};
assert.equal(rawGroups([{...valid,summary:undefined}]).length,0);
assert.equal(rawGroups([{...valid,summary:valid.title+' - Publisher'}]).length,0);
assert.equal(articleExcerpt({...valid,summary:'<ol><li><a href="/x">'+excerpt+'</a></li></ol>'}), '');
assert.equal(isReadableStory({...valid,summary:'Subscribe to read more of our latest exclusive reporting and news stories from across the world.'}),false);
assert.equal(rawGroups([valid]).length,1);
assert.equal(isArticleHeadline({...valid,title:'Breaking News - Latest National International News | Publisher'}),false);
const newer={...valid,url:'https://other.test/report/transport',published:'2026-10-26T13:00:00Z'};
assert.equal(rawGroups([valid,newer])[0].lead.url,newer.url);
assert.equal(newestStories([valid,{...newer,published:'2099-01-01'}],Date.parse('2026-10-27'))[0].url,valid.url);
const owned=partitionStories([{category:'national',items:[valid]},{category:'breaking',items:[newer]}],Date.parse('2026-10-27T00:00:00Z'));
assert.equal(owned.get('breaking').length,1);assert.equal(owned.get('national').length,0);
assert.equal(rawGroups([valid,{...newer,title:'Council rejects transport funding after public consultation'}]).length,2);
console.log('Content eligibility, link-list exclusion, freshness and Breaking ownership passed');

assert.equal(partitionStories([{category:'breaking',items:[valid]}],Date.parse('2026-10-28T00:00:00Z')).get('breaking').length,0);

assert.equal(articleExcerpt({...valid,summary:'<p>'+excerpt+'</p><ul><li>Related headline</li></ul>'}),excerpt);
