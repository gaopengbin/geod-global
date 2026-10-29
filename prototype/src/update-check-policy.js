import {failureDiagnostics} from './usage-diagnostics.js';
const PERIOD=6*60*60*1000,DELAYS=[60_000,300_000,900_000];
export function createUpdateCheckSchedule() {
  let lastAttempt=null,lastSuccess=0,failures=0,retryAt=null;
  return {
    get retryCount(){return Math.min(failures,3);},
    due(now,successfulCheck){
      const success=Date.parse(successfulCheck)||0;
      if(success>lastSuccess&&success>=(lastAttempt??0)){lastSuccess=success;failures=0;retryAt=null;}
      if(retryAt!==null)return now>=retryAt;
      const latest=Math.max(lastAttempt??0,lastSuccess);
      if(lastAttempt!==null&&now-latest>=PERIOD){failures=0;return true;}
      return lastAttempt===null&&!lastSuccess||now-latest>=PERIOD;
    },
    started(now){lastAttempt=now;},
    succeeded(now){lastAttempt=now;failures=0;retryAt=null;},
    failed(error,now){
      lastAttempt=now;failures++;
      const reason=failureDiagnostics(error).reason;
      retryAt=['network','timeout','upstream_unavailable'].includes(reason)&&failures<=DELAYS.length?now+DELAYS[failures-1]:null;
    },
  };
}
