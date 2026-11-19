import assert from 'node:assert/strict';
import {quietAt,quietPeriods,validQuietPeriods} from '../dist-test/settings_policy.js';
const r={quiet_periods:[{enabled:true,start:'22:00',end:'06:00'},{enabled:true,start:'13:00',end:'14:00'},{enabled:false,start:'',end:''}]};
assert.equal(quietAt(r,'23:30'),true);assert.equal(quietAt(r,'05:59'),true);assert.equal(quietAt(r,'06:00'),false);assert.equal(quietAt(r,'13:30'),true);assert.equal(quietAt(r,'16:00'),false);
assert.equal(validQuietPeriods(r.quiet_periods),true);assert.equal(validQuietPeriods([{enabled:true,start:'25:00',end:'06:00'}]),false);assert.equal(validQuietPeriods([{enabled:true,start:'12:00',end:'12:00'}]),false);assert.equal(quietPeriods({quiet_enabled:true,quiet_start:'10:00',quiet_end:'11:00'})[0].enabled,true);
console.log('Three quiet periods, overnight boundaries and legacy migration passed');
