import assert from 'node:assert/strict';
import {quietDuration,sourceIds} from '../dist-test/discovery.js';
assert.equal(quietDuration('Keep quiet for fifteen minutes'),900000);
assert.equal(quietDuration('Butler, please keep quiet for 2 hours.'),7200000);
assert.equal(quietDuration('be quiet for half an hour'),1800000);
assert.equal(quietDuration('silence for 30 seconds'),30000);
assert.equal(quietDuration('keep quiet for 0 minutes'),null);
assert.equal(quietDuration('Tell me a story about quiet forests'),null);
for(const category of ['breaking','world','national','regional','local','political','business','science','ai-tech','sports','entertainment','viral'])assert.ok(sourceIds(category).filter(id=>id!=='all').length>=5,category);
assert.ok(sourceIds('science').includes('nasa'));
assert.ok(sourceIds('business','India').includes('et'));
assert.ok(!sourceIds('business','India').includes('cnbc'));
assert.ok(sourceIds('local','India','Maharashtra').includes('loksatta'));
assert.ok(sourceIds('regional','India','Kerala').includes('manorama'));
assert.ok(sourceIds('national','UK').includes('sky'));
assert.ok(!sourceIds('national','USA').includes('hindu'));
assert.ok(sourceIds('business-world','India').includes('cnbc'));
assert.ok(!sourceIds('science','India').includes('ndtv'));
console.log('Quiet duration and category source tests passed');

for(const category of ['world','local','business','science','ai-tech']){assert.ok(sourceIds(category,'India').includes('google'));assert.ok(sourceIds(category,'India').includes('msn'));}
assert.ok(sourceIds('business','India').includes('cnbctv18'));
