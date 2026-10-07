const key=process.env.GEOD_AGENT_TEST_KEY;
if(!key)throw new Error('Explicit development test credential is required.');
const response=await fetch('http://127.0.0.1:19094/v1/models',{headers:{Authorization:`Bearer ${key}`},signal:AbortSignal.timeout(15_000)});
const value=await response.json();
console.log(JSON.stringify({status:response.status,configuredTestModelListed:(value.data??[]).some(model=>model.id==='deepseek-v4-flash'),notice:'A catalogue listing does not establish a tool-call round trip.'}));
if(!response.ok)process.exitCode=1;
