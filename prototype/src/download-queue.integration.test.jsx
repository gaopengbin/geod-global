import React from 'react';
import {beforeEach,expect,it,vi} from 'vitest';
import {render,screen,within} from '@testing-library/react';
import {I18nProvider} from './i18n.jsx';
import {RuntimeContext} from './runtime-context.js';
import {DeliveryReceipt} from './acquisition-ui.jsx';
import {AgentDownloadTasks} from './agent-download-tasks.jsx';
beforeEach(()=>Object.defineProperty(window,'localStorage',{configurable:true,value:{getItem:()=> 'en',setItem:vi.fn()}}));
const jobs=Array.from({length:9},(_,i)=>({id:'batch-'+i,kind:'download',status:i<2?'running':'queued',assetKey:'visual',bytesDownloaded:i<2?8192:0,totalBytes:null}));
it('manual delivery counts only its expected files and reports the same queue split as Agent downloads',()=>{
 const plan={planId:'batch',kind:'download',status:'submitted',jobs,files:jobs.map(()=>({assetKey:'visual'}))};
 render(<I18nProvider><RuntimeContext.Provider value={{jobs:[...jobs,{id:'unrelated',kind:'download',status:'succeeded'}]}}>
   <DeliveryReceipt jobIds={jobs.map(j=>j.id)} submitted/>
   <AgentDownloadTasks plans={[plan]} onOpenTasks={vi.fn()} renderPlan={()=>null}/>
 </RuntimeContext.Provider></I18nProvider>);
 const receipt=screen.getByRole('region',{name:'Download and results'});
 expect(within(receipt).getByRole('status').textContent).toBe('0 of 9 files downloaded · 2 downloading · 7 queued');
 expect(screen.getByRole('region',{name:'Download tasks'}).textContent).toContain('2 downloading · 7 queued');
});
