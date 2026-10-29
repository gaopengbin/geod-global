import test from 'node:test';import assert from 'node:assert/strict';
import {createUpdateCheckSchedule} from './update-check-policy.js';import {diagnosticError} from './usage-diagnostics.js';
test('temporary update failure gets three delayed checks before returning to the ordinary interval',()=>{
 const schedule=createUpdateCheckSchedule();let at=1000000;
 assert.equal(schedule.due(at,null),true);
 for(const delay of [60000,300000,900000]){
   schedule.started(at);schedule.failed(diagnosticError(new Error('Temporary issue'),{reason:'network'}),at);
   assert.equal(schedule.due(at+delay-1,null),false);assert.equal(schedule.due(at+delay,null),true);at+=delay;
 }
 assert.equal(schedule.retryCount,3);schedule.started(at);schedule.failed(new Error('network unavailable'),at);
 assert.equal(schedule.due(at+900000,null),false);assert.equal(schedule.due(at+6*3600000,null),true);
});
test('a successful manual check resets a pending retry and non-transient failure is not hammered',()=>{
 const schedule=createUpdateCheckSchedule(),at=Date.now();schedule.started(at);schedule.failed(new Error('network unavailable'),at);
 assert.equal(schedule.due(at+60000,new Date(at+1000).toISOString()),false);
 schedule.started(at+2000);schedule.failed(diagnosticError(new Error('Bad signature'),{reason:'signature_invalid'}),at+2000);
 assert.equal(schedule.due(at+600000,null),false);
});
