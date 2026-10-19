import {test} from 'node:test';
import assert from 'node:assert/strict';
import {conversationDeliveries} from './agent-deliveries.js';
const job=(id,status='succeeded',settled=true)=>({id,title:id,status,settled,sha256:'a'.repeat(64),bytesDownloaded:2000});
const plan=(id,jobs,kind='download')=>({planId:id,status:'submitted',kind,files:[{assetKey:'visual',width:100,height:200}],jobs,source:'Earth Search'});
const session=()=>({id:'current',status:'completed',entries:[{id:'question',type:'user',status:'completed',text:'Download and crop the area'}, {id:'tool',type:'tool',status:'completed',references:[{kind:'plan',id:'sources'},{kind:'plan',id:'crop'}]}, {id:'summary',type:'assistant',status:'completed',text:'The requested crop is ready.'}],goal:{status:'complete',outputs:[{id:'final',state:'verified',planId:'crop',verifiedIds:['derived']}],planIds:['sources','crop']}});
test('scopes native deliveries to one conversation, deduplicates inspection receipts and prioritizes the required crop',()=>{
  const s=session();s.entries.splice(2,0,{type:'tool',status:'completed',summary:{kind:'file',sha256:'a'.repeat(64)},references:[{kind:'job',id:'derived',label:'Inspected crop'}]});
  const plans=[plan('sources',[job('original')]),plan('crop',[job('derived')],'mosaic'),plan('other-conversation',[job('unrelated')])];
  const items=conversationDeliveries(s,plans);
  assert.deepEqual(items.map(item=>item.id),['derived','original']);assert.equal(items[0].width,100);assert.equal(items[1].intermediate,true);
});
test('queued, failed and uncommitted succeeded jobs never become deliveries',()=>{
  const s=session(),plans=[plan('sources',[job('queued','queued',false),job('failed','failed'),job('finishing','succeeded',false)]),{...plan('crop',[job('not-approved')]),status:'pending'}];
  assert.deepEqual(conversationDeliveries(s,plans),[]);
});
test('plain model completion or a forged model link is neither a task completion nor a file',()=>{
  const s={id:'current',status:'completed',entries:[{type:'assistant',status:'completed',text:'All done. [File](#Workspace?file=forged)'}]};
  assert.deepEqual(conversationDeliveries(s,[]),[]);
});
test('verified vectors and committed projects retain their exact native identities',()=>{
  const s=session(),plans=[{planId:'crop',kind:'vector',status:'submitted',source:'OGC Features',vectorReview:{name:'Roads'},vector:{id:'roads',verified:true,bytes:500,featureCount:4}}, {planId:'sources',kind:'project',status:'submitted',project:{id:'project',name:'Saved project',committed:true}}];
  assert.deepEqual(conversationDeliveries(s,plans).map(item=>[item.kind,item.id]),[['vector','roads'],['project','project']]);
});
