import test from 'node:test';
import assert from 'node:assert/strict';
import {conversationDeliveries} from './agent-deliveries.js';
import {validateGSCloudArchive} from './gscloud-client.js';

const id='a1234567-1234-1234-1234-123456789abc',planId='b1234567-1234-1234-1234-123456789abc';
test('GSCloud ZIP results are archives, deduplicate inspection references and stay in their conversation',()=>{
  const session={entries:[{type:'tool',status:'completed',references:[{kind:'plan',id:planId},{kind:'job',id}],summary:{kind:'file',sha256:'a'.repeat(64)}}],workspaceView:{kind:'archive',id,verified:true}};
  const plan={planId,kind:'download',archive:true,status:'submitted',format:'Original ZIP',files:[{assetKey:'gscloud_aster30v2'}],jobs:[{id,title:'GDEM V2',status:'succeeded',settled:true,bytesDownloaded:120,sha256:'a'.repeat(64)}]};
  const results=conversationDeliveries(session,[plan]);assert.equal(results.length,1);assert.equal(results[0].kind,'archive');
  assert.equal(conversationDeliveries({entries:[]},[plan]).length,0);
  assert.equal(conversationDeliveries({entries:session.entries},[{...plan,jobs:[{...plan.jobs[0],status:'running'}]}]).filter(v=>v.kind==='archive').length,0);
});
test('archive display rejects wrong tasks, unsafe members and unverified ZIPs',()=>{
  const value={kind:'archive',jobId:id,productId:'421',itemId:'ASTGTM2_N40E116',format:'Original ZIP',verified:true,rasterPreview:false,bytes:120,sha256:'a'.repeat(64),members:[{name:'ASTGTM2_N40E116_dem.tif',bytes:100,crc32:'1234abcd'}]};
  assert.equal(validateGSCloudArchive(value,id),value);
  for(const bad of [{...value,jobId:planId},{...value,verified:false},{...value,members:[{...value.members[0],name:'../outside.tif'}]},{...value,productId:'309'}])assert.throws(()=>validateGSCloudArchive(bad,id));
});
