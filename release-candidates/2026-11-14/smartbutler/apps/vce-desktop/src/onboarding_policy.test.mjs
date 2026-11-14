import assert from 'node:assert/strict';
import {restoreSetup,routingFor,stageSnapshot} from '../dist-test/onboarding_policy.js';
const none=restoreSetup(null);assert.equal(none.local_model,'');assert.deepEqual(routingFor(none),{provider:'rules',local_model:'',online_enabled:false,online_provider:'groq',online_model:'openai/gpt-oss-20b'});
const local=restoreSetup({provider:'rules',local_model:'ollama:smollm2:1.7b'});assert.equal(routingFor(local).provider,'auto');assert.equal(routingFor(local).online_enabled,false);
const mixed=restoreSetup({provider:'openrouter',local_model:'phi3.5-mini-q4',online_model:'openrouter/free'});assert.equal(routingFor(mixed).online_enabled,true);
const draft={name:'Test',consents:{profile_storage:true,microphone:false},contacts:[],reminders:[]};const snapshot=stageSnapshot(draft,5,{...local,voice:'hindi',apiKey:'must-not-save',password:'must-not-save'});const resumed=JSON.parse(JSON.stringify(snapshot));assert.equal(restoreSetup(resumed.setup).voice,'hindi');assert.equal(resumed.draft.consents.microphone,false);assert.ok(!JSON.stringify(snapshot).includes('must-not-save'));draft.name='changed';assert.equal(snapshot.draft.name,'Test');
console.log('Optional model routing, offline-only mode and resumable secret-free stage snapshots passed');
