import assert from 'node:assert/strict';
import { dueReminders, deliveryKey, currentDayAndTime } from '../dist-test/reminders.js';

const inZone = date => currentDayAndTime(new Date(date), 'Asia/Kolkata');
assert.deepEqual(inZone('2026-09-22T18:45:00Z'), { day: '2026-09-23', time: '00:15' });

const birthday = {id:'birthday',title:'Birthday',date:'2000-02-29',time:'09:00',annual:true,lead_days:0,enabled:true};
let due = dueReminders([birthday], new Date('2027-02-28T04:00:00Z'), 'Asia/Kolkata', null, new Set());
assert.equal(due.length, 1);
assert.equal(deliveryKey(due[0]), 'birthday:2027-02-28');
assert.equal(dueReminders([birthday], new Date('2027-02-28T04:00:00Z'), 'Asia/Kolkata', null, new Set([deliveryKey(due[0])])).length, 0);

const annual = {id:'new-year',title:'New Year',date:'2020-01-01',time:'09:00',annual:true,lead_days:2,enabled:true};
due = dueReminders([annual], new Date('2026-12-31T06:00:00Z'), 'Asia/Kolkata', '2026-12-29', new Set());
assert.equal(due.length, 1);
assert.equal(due[0].dueDate, '2026-12-30');
assert.equal(due[0].missed, true);

const future = {id:'future',title:'Future',date:'2028-01-01',time:'09:00',annual:true,lead_days:2,enabled:true};
assert.equal(dueReminders([future], new Date('2026-12-30T06:00:00Z'), 'Asia/Kolkata', null, new Set()).length, 0);
assert.equal(dueReminders([{...annual,enabled:false}], new Date('2026-12-30T06:00:00Z'), 'Asia/Kolkata', null, new Set()).length, 0);
assert.equal(dueReminders([{...annual,date:'2020-02-31'}], new Date('2026-12-30T06:00:00Z'), 'Asia/Kolkata', null, new Set()).length, 0);
console.log('Reminder schedule tests passed');
