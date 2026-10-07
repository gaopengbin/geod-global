import test from 'node:test';
import assert from 'node:assert/strict';
import { subscribeAgentUpdates, validAgentContext, validateConnectionTest, validateAgentImage, validateAgentImagePreview, validateAgentImageStorage, validateAgentDocument, validateAgentDocumentPreview, validateAgentAttachmentStorage, validateAgentSnapshot, validatePlanRevisionDraft } from './agent-client.js';
import { MODEL_CAPABILITIES, MODEL_PROVIDERS, groupedConnections, modelCapabilities } from './agent-model-registry.js';
import {readFileSync} from 'node:fs';

test('stream signals only invalidate a native snapshot; transcript payloads are refused',async t=>{
  const previous=globalThis.window;t.after(()=>{if(previous===undefined)delete globalThis.window;else globalThis.window=previous;});
  let receive,changes=0,disposed=false;
  globalThis.window={__TAURI__:{event:{listen:async(name,callback)=>{assert.equal(name,'geod-agent-changed');receive=callback;return ()=>{disposed=true;};}}}};
  const dispose=await subscribeAgentUpdates(()=>{changes++;});
  for(const payload of [0,5])receive({payload});
  for(const payload of [-1,NaN,9007199254740992,{revision:5,text:'unvalidated'},'5'])receive({payload});
  assert.equal(changes,2);dispose();assert.equal(disposed,true);
});

test('connection test receiver rejects unproved success and unknown private fields',()=>{
  const value={version:1,status:'passed',text:true,functionCalls:true,latencyMs:520,checkedAt:'2026-10-06T12:00:00Z'};
  assert.equal(validateConnectionTest(value),value);
  assert.throws(()=>validateConnectionTest({...value,functionCalls:false}));
  assert.throws(()=>validateConnectionTest({...value,apiKey:'private'}));
  assert.throws(()=>validateConnectionTest({...value,latencyMs:-1}));
  const failed={...value,status:'failed',functionCalls:false,message:'Agent model authorization was rejected.'};
  assert.equal(validateConnectionTest(failed),failed);
  assert.throws(()=>validateConnectionTest({...failed,message:undefined}));
});
test('video previews accept native container metadata and reject remote URLs, incompatible codecs and altered sizes',()=>{
  const records=JSON.parse(readFileSync(new URL('../../agent/fixtures/video-records.json',import.meta.url),'utf8'));
  for(const record of records){const bytes=readFileSync(new URL('../../agent/fixtures/'+record.document.name,import.meta.url));
    const preview={...record,dataUrl:`data:${record.document.mimeType};base64,`+bytes.toString('base64')};
    assert.equal(validateAgentDocumentPreview(preview),preview);
    for(const changed of [{...preview,dataUrl:'https://example.test/video'}, {...preview,video:{...preview.video,width:4097}},
      {...preview,video:{...preview.video,codec:'HEVC'}}, {...preview,video:{...preview.video,durationMs:600_001}},
      {...preview,document:{...preview.document,bytes:bytes.length+1}}])assert.throws(()=>validateAgentDocumentPreview(changed));
    assert.throws(()=>validateAgentDocument({...preview.document,pages:1}));
  }
});
test('audio previews accept decoded native records and reject paths, incompatible envelopes or altered byte counts',()=>{
  const records=JSON.parse(readFileSync(new URL('../../agent/fixtures/audio-records.json',import.meta.url),'utf8'));
  for(const record of records){const bytes=readFileSync(new URL('../../agent/fixtures/'+record.document.name,import.meta.url));
    const preview={...record,dataUrl:`data:${record.document.mimeType};base64,`+bytes.toString('base64')};
    assert.equal(validateAgentDocumentPreview(preview),preview);
    assert.throws(()=>validateAgentDocumentPreview({...preview,dataUrl:'https://example.test/audio'}));
    assert.throws(()=>validateAgentDocumentPreview({...preview,audio:{...preview.audio,durationMs:600_001}}));
    assert.throws(()=>validateAgentDocumentPreview({...preview,document:{...preview.document,bytes:bytes.length+1}}));
    assert.throws(()=>validateAgentDocument({...preview.document,pages:1}));
  }
});
test('Office previews keep native text and accept only matching format metadata',()=>{
  const records=JSON.parse(readFileSync(new URL('../../agent/fixtures/office-records.json',import.meta.url),'utf8'));
  for(const preview of records){assert.equal(validateAgentDocumentPreview(preview),preview);const document=preview.document;
    for(const changed of [{...document,name:'file.pdf'},{...document,pages:1},{...document,characters:256*1024+1},{...document,bytes:2*1024*1024+1}])assert.throws(()=>validateAgentDocument(changed));
    assert.throws(()=>validateAgentDocumentPreview({...preview,text:preview.text+'changed'}));}
});
const base={version:1,revision:0,mode:'read-only',runtimeAvailable:false,configured:false,busy:false,sessions:[],selected:null};
test('goal snapshots reject fabricated completion, duplicate outputs, hidden fields and unbounded continuation',()=>{
  const goal={version:1,id:'b1234567-1234-1234-1234-123456789abc',objective:'Deliver imagery.',requestText:'Download imagery.',outputs:[{id:'image',label:'Imagery',kind:'raster',state:'missing',verifiedIds:[],planId:null,entryId:null}],planIds:[],status:'active',engine:null,continuations:0,updatedAt:'2026-10-07T00:00:00Z',checkedAt:null,reason:null};
  const snapshot={...base,selected:{id:'a1234567-1234-1234-1234-123456789abc',status:'completed',entries:[],goal}};
  assert.equal(validateAgentSnapshot(snapshot),snapshot);
  for(const change of [{status:'complete'},{outputs:[...goal.outputs,...goal.outputs]},{continuations:9},{password:'private'}])assert.throws(()=>validateAgentSnapshot({...snapshot,selected:{...snapshot.selected,goal:{...goal,...change}}}));
});
test('decision cards and task summaries accept actual bounded choices and reject invented permissions or answers',()=>{
  const id='b1234567-1234-1234-1234-123456789abc',decision={version:1,id,title:'Choose task scope',status:'pending',questions:[{id:'boundary',prompt:'Which boundary?',recommendedOptionId:'polygon',options:[{id:'polygon',label:'Polygon',description:'Keep the actual geometry.'},{id:'bbox',label:'Rectangle',description:'Keep the bounding rectangle.'}]}]};
  const entry={id:'question',type:'tool',name:'geod_request_decision',status:'completed',references:[],decision};
  const snapshot={...base,selected:{id,status:'completed',entries:[entry]}};
  assert.equal(validateAgentSnapshot(snapshot),snapshot);
  for(const changed of [{...entry,name:'geod_download_plan'},{...entry,decision:{...decision,executionMode:'full-access'}},{...entry,decision:{...decision,status:'answered',answers:[{questionId:'boundary',optionId:'invented'}]}},{...entry,decision:{...decision,status:'pending',answers:[{questionId:'boundary',optionId:'polygon'}]}}])assert.throws(()=>validateAgentSnapshot({...snapshot,selected:{...snapshot.selected,entries:[changed]}}));
  const taskContext={requestText:'Prepare my original requested data.',choices:[{decisionId:id,questionId:'boundary',prompt:'Which boundary?',answer:'Polygon'}]};
  const reviewed={...entry,name:'geod_download_plan',decision:undefined,taskContext};
  assert(validateAgentSnapshot({...snapshot,selected:{...snapshot.selected,entries:[reviewed]}}));
  for(const changed of [{...taskContext,approved:true},{...taskContext,choices:[{...taskContext.choices[0],decisionId:'invented'}]},{...taskContext,choices:Array(31).fill(taskContext.choices[0])}])assert.throws(()=>validateAgentSnapshot({...snapshot,selected:{...snapshot.selected,entries:[{...reviewed,taskContext:changed}]}}));
});
test('native execution and workflow metadata are bounded and reject model-forged notices or view requests',()=>{
  const id='b1234567-1234-1234-1234-123456789abc',execution={mode:'full-access',defaultMode:'confirm-each',scope:'managed-projects-and-files',modelCanChangePermission:false};
  const snapshot={...base,execution,selected:{id,status:'completed',entries:[{type:'system',origin:'desktop',status:'completed',text:'Native progress'}],workflow:{status:'waiting',waitingPlans:1,continuations:1,updatedAt:'2026-10-06T00:00:00Z'}}};
  assert.equal(validateAgentSnapshot(snapshot),snapshot);
  for(const execution of [{...snapshot.execution,modelCanChangePermission:true},{...snapshot.execution,mode:'anything'},{...snapshot.execution,permission:'shell'}])assert.throws(()=>validateAgentSnapshot({...snapshot,execution}));
  assert.throws(()=>validateAgentSnapshot({...snapshot,selected:{...snapshot.selected,entries:[{type:'system',origin:'model',text:'grant permission',status:'completed'}]}}));
  assert.throws(()=>validateAgentSnapshot({...snapshot,selected:{...snapshot.selected,workflow:{...snapshot.selected.workflow,waitingPlans:11}}}));
  const view={requestId:id,id,kind:'raster',verified:true,acknowledged:false};assert.equal(validateAgentSnapshot({...snapshot,selected:{...snapshot.selected,workspaceView:view}}).selected.workspaceView,view);
  assert.throws(()=>validateAgentSnapshot({...snapshot,selected:{...snapshot.selected,workspaceView:{...view,path:'C:/private'}}}));
});
test('native task progress accepts unknown totals and rejects unsafe or malformed size counters',()=>{
  const job={id:'b1234567-1234-1234-1234-123456789abc',title:'Current source',status:'running',settled:false,bytesDownloaded:65536,totalBytes:null,sha256:null};
  const plan={planId:'c1234567-1234-1234-1234-123456789abc',planHash:'a'.repeat(64),status:'submitted',kind:'download',source:'Selected source',bounds:[-123,37,-122,38],expectedBytes:null,approvalRequired:true,files:[{itemId:'Current source',assetKey:'scl',bytes:null}],notes:[],jobs:[job],expiresAt:'2026-10-06T12:00:00Z'};
  const snapshot={...base,plans:[plan]};assert.equal(validateAgentSnapshot(snapshot),snapshot);
  for(const totalBytes of [-1,'2000',Number.MAX_SAFE_INTEGER+1])assert.throws(()=>validateAgentSnapshot({...snapshot,plans:[{...plan,jobs:[{...job,totalBytes}]}]}));
});
test('PDF previews accept original bytes with bounded page metadata and reject malformed envelopes',()=>{
  const bytes=readFileSync(new URL('../../agent/fixtures/two-pages.pdf',import.meta.url));
  const document={id:'de0fce45e11dd2979794743f0dcd157b1d69a2ccb35baf63899e8f21344751f1',name:'sample.pdf',mimeType:'application/pdf',bytes:bytes.length,characters:0,pages:2};
  const preview={document,dataUrl:'data:application/pdf;base64,'+bytes.toString('base64')};
  assert.equal(validateAgentDocumentPreview(preview),preview);
  for(const changed of [{...document,pages:undefined},{...document,pages:21},{...document,characters:1},{...document,mimeType:'text/plain'}])assert.throws(()=>validateAgentDocument(changed));
  for(const changed of [{...preview,path:'outside'},{...preview,dataUrl:'https://example.test/file.pdf'},{...preview,dataUrl:'data:application/pdf;base64,'+Buffer.alloc(bytes.length,120).toString('base64')},
    {...preview,document:{...document,bytes:bytes.length+1}}])assert.throws(()=>validateAgentDocumentPreview(changed));
  const selected={id:'a1234567-1234-1234-1234-123456789abc',status:'completed',entries:[{type:'user',status:'completed',text:'',documents:[document]}]};
  assert.equal(validateAgentSnapshot({...base,selected}).selected.entries[0].documents[0],document);
});
test('image storage accepts closed counters and refuses paths or inconsistent cleanup sizes',()=>{
  const storage={limitBytes:128*1024*1024,usedBytes:2048,imageCount:3,unusedBytes:1024,unusedCount:1,removedBytes:0,removedCount:0};
  assert.equal(validateAgentImageStorage(storage),storage);
  for(const changed of [{...storage,path:'C:/private'},{...storage,usedBytes:-1},{...storage,unusedCount:4},{...storage,unusedBytes:4096},{...storage,limitBytes:0}])assert.throws(()=>validateAgentImageStorage(changed));
});
test('document previews validate UTF-8 metadata and attachment storage returns only closed counters',()=>{
  const text='北京 🌍',document={id:'b'.repeat(64),name:'notes.md',mimeType:'text/plain',bytes:new TextEncoder().encode(text).length,characters:[...text].length};
  assert.equal(validateAgentDocument(document),document);assert.equal(validateAgentDocumentPreview({document,text}).text,text);
  for(const changed of [{...document,path:'outside'},{...document,id:[document.id]},{...document,name:'../notes.md'},{...document,name:'data.pdf'},{...document,bytes:100000}])assert.throws(()=>validateAgentDocument(changed));
  assert.throws(()=>validateAgentDocumentPreview({document,text:'wrong'}));
  const storage={images:{limitBytes:128*1024*1024,usedBytes:0,imageCount:0,unusedBytes:0,unusedCount:0,removedBytes:0,removedCount:0},documents:{limitBytes:32*1024*1024,usedBytes:64,documentCount:1,unusedBytes:64,unusedCount:1,removedBytes:0,removedCount:0}};
  assert.equal(validateAgentAttachmentStorage(storage),storage);assert.throws(()=>validateAgentAttachmentStorage({...storage,path:'outside'}));
  assert.throws(()=>validateAgentAttachmentStorage({...storage,documents:{...storage.documents,unusedCount:2}}));
  const selected={id:'a1234567-1234-1234-1234-123456789abc',status:'completed',entries:[{type:'user',status:'completed',text:'',documents:[document]}]};
  assert.equal(validateAgentSnapshot({...base,selected}).selected.entries[0].documents[0],document);
  assert.throws(()=>validateAgentSnapshot({...base,selected:{...selected,entries:[{type:'assistant',status:'completed',text:'',documents:[document]}]}}));
});
test('context status reflects only bounded checkpoint and usage metadata',()=>{
  const state={status:'ready',count:1,lastCompletedAt:'2026-10-06T04:00:00Z',usedTokens:1000,windowTokens:32768};assert(validAgentContext(state));
  for(const changed of [{...state,status:'submitted'},{...state,count:-1},{...state,lastCompletedAt:'not-a-date'},{...state,usedTokens:1.2},{...state,windowTokens:0},{...state,summary:'model-authored data'}])assert(!validAgentContext(changed));
});
test('native image data accepts only bounded PNG previews and path-free user references',()=>{
  const dataUrl='data:image/png;base64,iVBORw0KGgoAAAANSUhEUgAAAAEAAAABCAQAAAC1HAwCAAAAC0lEQVR42mP8/x8AAwMCAO+jY1kAAAAASUVORK5CYII=';
  const image={id:'a'.repeat(64),name:'map.png',mimeType:'image/png',bytes:atob(dataUrl.split(',')[1]).length,width:1,height:1};
  assert.equal(validateAgentImage(image),image);assert.equal(validateAgentImagePreview({image,dataUrl}).image,image);
  for(const changed of [{...image,path:'C:/private'},{...image,name:'../map.png'},{...image,width:0},{...image,bytes:6*1024*1024}])assert.throws(()=>validateAgentImage(changed));
  for(const url of ['https://example.test/image.png','data:image/svg+xml;base64,PHN2Zy8+',dataUrl+'junk'])assert.throws(()=>validateAgentImagePreview({image,dataUrl:url}));
  const selected={id:'a1234567-1234-1234-1234-123456789abc',status:'completed',entries:[{type:'user',status:'completed',text:'',images:[image]}]};
  assert.equal(validateAgentSnapshot({...base,selected}).selected.entries[0].images[0],image);
  assert.throws(()=>validateAgentSnapshot({...base,selected:{...selected,entries:[{type:'assistant',status:'completed',text:'',images:[image]}]}}));
});
const connection=(id,provider='custom')=>({id,provider,label:'Saved connection',protocol:'openai-compatible',baseUrl:'https://example.test/v1',model:'test-model',capabilities:{...MODEL_CAPABILITIES},verification:'not-verified'});
test('vector reviews require native scope and count only a verified submitted result',()=>{
  const id='a1234567-1234-1234-1234-123456789abc';
  const review={serviceId:id,serviceSha256:'b'.repeat(64),collectionId:'lakes',collectionTitle:'Lakes',protocol:'OGC API Features',name:'Reviewed lakes',pageSize:2,responseFormat:null,selection:'bbox-full-features',liveAvailabilityChecked:false,clipped:false};
  const plan={planId:id,planHash:'a'.repeat(64),kind:'vector',status:'pending',source:'Saved service',bounds:[-130,20,-60,65],boundsCrs:'EPSG:4326',polygon:null,replacedBy:null,expiresAt:'2026-10-05T12:00:00Z',approvalRequired:true,expectedBytes:null,jobs:[],notes:[],vectorReview:review,vector:null};
  const value={...base,mode:'review-first',plans:[plan]};
  assert.equal(validateAgentSnapshot(value),value);
  const overpass={...plan,bounds:[13.3773,52.5167,13.3783,52.5174],vectorReview:{...review,protocol:'Overpass',collectionId:'buildings',pageSize:null,selection:'overpass-bbox-full-geometry'}};
  assert.equal(validateAgentSnapshot({...value,plans:[overpass]}).plans[0],overpass);
  for(const broken of [{...overpass,vectorReview:{...overpass.vectorReview,pageSize:200}},
    {...plan,vectorReview:{...review,pageSize:null}}])assert.throws(()=>validateAgentSnapshot({...value,plans:[broken]}));
  const result={id:'b1234567-1234-1234-1234-123456789abc',name:review.name,format:'geojson',bytes:14842,featureCount:13,coordinateCount:253,sourceSha256:'c'.repeat(64),geojsonSha256:'d'.repeat(64),verified:true};
  assert.equal(validateAgentSnapshot({...value,plans:[{...plan,status:'submitted',vector:result}]}).plans[0].vector.featureCount,13);
  for(const broken of [{...plan,vector:result},{...plan,status:'submitted'},{...plan,expectedBytes:100},
    {...plan,jobs:[{id,stage:'succeeded'}]},{...plan,vectorReview:{...review,liveAvailabilityChecked:true}},
    {...plan,vectorReview:{...review,clipped:true}},{...plan,vectorReview:{...review,selection:'overpass-bbox-full-geometry'}},
    {...plan,vectorReview:{...review,responseFormat:'application/json'}},{...plan,vectorReview:{...review,url:'https://replace.test'}},
    ...[{...result,verified:false},{...result,bytes:20971521},{...result,featureCount:50001},{...result,sourceSha256:'fake'},{...result,path:'private'}].map(vector=>({...plan,status:'submitted',vector}))]) {
    assert.throws(()=>validateAgentSnapshot({...value,plans:[broken]}));
  }
  const draft={planId:id,planHash:plan.planHash,kind:'vector',parameters:{kind:'vector',name:review.name,bounds:plan.bounds,keepPolygon:false},fields:{name:true,bounds:true,polygon:false},boundsCrs:'EPSG:4326'};
  assert.equal(validatePlanRevisionDraft(draft),draft);
  assert.throws(()=>validatePlanRevisionDraft({...draft,parameters:{...draft.parameters,serviceUrl:'https://replace.test'}}));
});
test('verified vector links keep native IDs and reject false or unbounded verification summaries',()=>{
  const id='a1234567-1234-1234-1234-123456789abc';
  const entry={id:'read',type:'tool',name:'geod_vector_inspect',status:'completed',references:[{kind:'vector',id,label:'Actual vector'}],summary:{kind:'vector',count:25,verified:true,sha256:'a'.repeat(64)}};
  const value={...base,selected:{id,status:'completed',entries:[entry]}};assert.equal(validateAgentSnapshot(value),value);
  for(const summary of [{...entry.summary,verified:false},{...entry.summary,sha256:'forged'},{...entry.summary,count:50001},{...entry.summary,path:'private'}]){
    assert.throws(()=>validateAgentSnapshot({...value,selected:{...value.selected,entries:[{...entry,summary}]}}));
  }
  assert.throws(()=>validateAgentSnapshot({...value,selected:{...value.selected,entries:[{...entry,references:[{kind:'vector',id:'../path',label:'Bad'}]}]}}));
});
test('human revision forms retain the native CRS and closed fields without source paths or arbitrary selections',()=>{
  const common={planId:'a1234567-1234-1234-1234-123456789abc',planHash:'a'.repeat(64)};
  const clip={...common,kind:'clip',parameters:{kind:'clip',name:'Correct crop',bounds:[500000,4199940,500090,4200000],keepPolygon:true},fields:{name:true,bounds:true,polygon:true},boundsCrs:'EPSG:32610'};
  assert.equal(validatePlanRevisionDraft(clip),clip);
  const project={...common,kind:'project',parameters:{kind:'project',name:null,bounds:null,itemIds:['old','new'],keepPolygon:false},fields:{name:false,bounds:false,polygon:false,items:[{id:'old',date:'2026-10-01',locked:true},{id:'new',date:'2026-10-02',locked:false}]},boundsCrs:'EPSG:4326'};
  assert.equal(validatePlanRevisionDraft(project),project);
  const rgb={...common,kind:'rgb',parameters:{kind:'rgb',name:'Original RGB',qualityPolicy:'cloud_free',excludeSnow:false},fields:{name:true,qualityPolicies:['cloud_free','cloud_free_conservative'],snow:true}};
  assert.equal(validatePlanRevisionDraft(rgb),rgb);
  for(const broken of [{...clip,path:'private'},{...clip,parameters:{...clip.parameters,href:'https://source.test'}},{...clip,boundsCrs:'EPSG:32600'},
    {...clip,fields:{...clip.fields,apiKey:true}},{...project,parameters:{...project.parameters,itemIds:['new']}},{...project,parameters:{...project.parameters,itemIds:['unreviewed']}},
    {...rgb,parameters:{...rgb.parameters,qualityPolicy:'good'}},{...rgb,fields:{...rgb.fields,qualityPolicies:['cloud_free','execute']}}])assert.throws(()=>validatePlanRevisionDraft(broken));
});

test('custom download reviews require archived asset identities and revision labels retain distinct pins',()=>{
  const file={itemId:'Same native item',assetKey:'stac_asset',originalAssetKey:'science-band',snapshotId:'b'.repeat(64),referenceId:'c'.repeat(64),bytes:2000};
  const plan={planId:'b1234567-1234-1234-1234-123456789abc',planHash:'a'.repeat(64),kind:'download',status:'pending',approvalRequired:true,source:'Selected custom raster assets',bounds:[13,52,14,53],files:[file],expectedBytes:2000,expiresAt:'2026-10-05T12:00:00Z',notes:[],jobs:[],project:{id:'a1234567-1234-1234-1234-123456789abc',name:'Native custom project',sceneCount:1,mode:'existing',committed:false}};
  const value={...base,mode:'review-first',plans:[plan]};assert.equal(validateAgentSnapshot(value),value);
  for(const broken of [{...file,snapshotId:'forged'},{...file,referenceId:undefined},{...file,originalAssetKey:''},{...file,originalAssetKey:'control\u0000text'}])assert.throws(()=>validateAgentSnapshot({...value,plans:[{...plan,files:[broken]}]}));
  const draft={planId:plan.planId,planHash:plan.planHash,kind:'download',parameters:{kind:'download',itemIds:['c'.repeat(64),'d'.repeat(64)]},fields:{items:[{id:'c'.repeat(64),label:'Same native item · first band',locked:false},{id:'d'.repeat(64),label:'Same native item · second band',locked:false}]},boundsCrs:'EPSG:4326',projectId:plan.project.id};
  assert.equal(validatePlanRevisionDraft(draft),draft);
  assert.throws(()=>validatePlanRevisionDraft({...draft,fields:{items:[{...draft.fields.items[0],href:'https://replace.test/original'}]}}));
});
test('coverage reviews retain native grid identity without source URL or invented encoded size',()=>{
  const file={itemId:'native:coverage',assetKey:'wcs_coverage',referenceId:'c'.repeat(64),coveragePlanId:'c'.repeat(64),bytes:null,width:48,height:48,crs:'EPSG:4326',requestedBounds:[2,53,2.05,53.05],alignedBounds:[2,53,2.05,53.05],selection:'bbox-native-grid'};
  const plan={planId:'b1234567-1234-1234-1234-123456789abc',planHash:'a'.repeat(64),kind:'download',status:'pending',approvalRequired:true,source:'Selected WCS coverage subsets',bounds:file.requestedBounds,files:[file],expectedBytes:null,expiresAt:'2026-10-05T12:00:00Z',notes:[],jobs:[],project:{id:'a1234567-1234-1234-1234-123456789abc',name:'Native coverage project',sceneCount:1,mode:'existing',committed:false}};
  const value={...base,mode:'review-first',plans:[plan]};assert.equal(validateAgentSnapshot(value),value);
  for(const broken of [{...file,referenceId:'d'.repeat(64)},{...file,width:65537},{...file,crs:'EPSG:27700'},
    {...file,bytes:9600},{...file,href:'https://replace.test/wcs'},{...file,selection:'polygon-mask'},{...file,alignedBounds:[2,54,2.05,53]}]) {
    assert.throws(()=>validateAgentSnapshot({...value,plans:[{...plan,files:[broken]}]}));
  }
});
test('protected download cards retain native formats and closed authorization facts without entitlement claims',()=>{
  const authorization={provider:'nasa-earthdata',status:'not-connected',expiresAt:null,verifiedAt:null,downloadEnabled:false,entitlement:'not-checked'};
  const plan={planId:'b1234567-1234-1234-1234-123456789abc',planHash:'a'.repeat(64),kind:'download',status:'pending',approvalRequired:true,source:'NASA Earthdata',bounds:[-123,37,-122,38],files:[{itemId:'N37W123.SRTMGL1.hgt',assetKey:'srtm',bytes:null}],expectedBytes:null,expiresAt:'2099-01-01T00:00:00Z',notes:[],jobs:[],format:'HGT ZIP',authorization};
  const value={...base,mode:'review-first',plans:[plan]}; assert.equal(validateAgentSnapshot(value),value);
  for (const [format,assetKey,provider] of [['GeoTIFF','red','nasa-earthdata'],['HDF5','viirs','nasa-earthdata'],['SAFE ZIP','product','copernicus']]) {
    const v={...value,plans:[{...plan,format,files:[{...plan.files[0],assetKey}],authorization:{...authorization,provider}}]};
    assert.equal(validateAgentSnapshot(v),v);
  }
  for(const broken of [{...authorization,token:'hidden'},{...authorization,downloadEnabled:true},
    {...authorization,status:'download-authorized'},{...authorization,entitlement:'verified'},
    {...authorization,provider:'unreviewed'},{...authorization,href:'https://replace.test'}])
    assert.throws(()=>validateAgentSnapshot({...value,plans:[{...plan,authorization:broken}]}));
  for(const broken of [{...plan,format:'GeoTIFF'},{...plan,files:undefined},{...plan,expectedBytes:100},
    {...plan,files:[{...plan.files[0],bytes:100}]},{...plan,kind:'project'}])
    assert.throws(()=>validateAgentSnapshot({...value,plans:[broken]}));
});
test('registry snapshots accept bounded groups and reject forged protocols, availability, capability or credential data',()=>{
  const first=connection('a1234567-1234-1234-1234-123456789abc','openai');
  const second=connection('b1234567-1234-1234-1234-123456789abc','deepseek');
  const registry={version:1,selectedId:first.id,connections:[first,second]};
  const snapshot={...base,model:{...first,capabilities:{...first.capabilities}},registry};
  assert.equal(validateAgentSnapshot(snapshot),snapshot);
  assert.deepEqual(groupedConnections([second,first]).map(group=>[group.id,group.connections[0].id]),[['openai',first.id],['deepseek',second.id]]);
  assert.deepEqual(MODEL_PROVIDERS.map(provider=>provider.id),['openai','deepseek','anthropic','google','custom']);
  for (const [provider,protocol] of [['anthropic','anthropic-messages'],['google','google-generative-ai'],['custom','google-generative-ai'],['openai','openai-responses'],['custom','openai-responses']]) {
    const native={...first,provider,protocol,model:'native-model',capabilities:modelCapabilities(protocol)};
    assert.equal(validateAgentSnapshot({...base,model:native,registry:{...registry,connections:[native]}}).model.protocol,protocol);
    if(protocol==='openai-responses')assert.throws(()=>validateAgentSnapshot({...base,registry:{...registry,connections:[{...native,capabilities:{...native.capabilities,encryptedReasoning:false}}]}}));
  }
  const brokenConnections=[{...first,credentialRef:'model-connection'},{...first,provider:'anthropic'},{...first,provider:'google'},
    {...first,protocol:'anthropic-messages'},{...first,verification:'verified'},
    {...first,capabilities:{...first.capabilities,encryptedReasoning:true}},{...first,capabilities:{...first.capabilities,execute:true}},
    {...first,baseUrl:'http://remote.test/v1'},{...first,baseUrl:'https://example.test/v1?apiKey=private'},
    {...first,baseUrl:'https://private:private@example.test/v1'}];
  for(const broken of brokenConnections) assert.throws(()=>validateAgentSnapshot({...snapshot,registry:{...registry,connections:[broken,second]}}));
  for(const broken of [{...registry,connections:[first,first]},{...registry,selectedId:'c1234567-1234-1234-1234-123456789abc'},
    {...registry,connections:Array(17).fill(first)},{...registry,credentialRef:'private'}]) assert.throws(()=>validateAgentSnapshot({...snapshot,registry:broken}));
  assert.throws(()=>validateAgentSnapshot({...snapshot,model:{...first,baseUrl:'https://changed.test/v1'}}));
  assert.equal(validateAgentSnapshot({...base,registry:{version:1,selectedId:null,connections:[]},model:null}).model,null);
});
test('native account summaries reject injected settings links, credentials and fabricated status',()=>{
  const summary={kind:'sources',count:9,checkedAt:'2026-10-05T00:00:00Z',accounts:['nasa-earthdata','copernicus'].map(provider=>({provider,status:'not-connected',expiresAt:null,verifiedAt:null}))};
  const entry={id:'source-result',type:'tool',name:'geod_sources_list',status:'completed',references:[],summary};
  const snapshot={...base,selected:{id:'a1234567-1234-1234-1234-123456789abc',status:'completed',entries:[entry]}};
  assert.equal(validateAgentSnapshot(snapshot),snapshot);
  for(const broken of [{...summary,checkedAt:'private text'}, {...summary,href:'https://untrusted.test'},
    {...summary,accounts:[summary.accounts[0],summary.accounts[0]]},
    {...summary,accounts:summary.accounts.map(account=>({...account,token:'hidden'}))},
    {...summary,accounts:summary.accounts.map(account=>({...account,status:'download-authorized'}))}])
    assert.throws(()=>validateAgentSnapshot({...snapshot,selected:{...snapshot.selected,entries:[{...entry,summary:broken}]}}));
  assert.throws(()=>validateAgentSnapshot({...snapshot,selected:{...snapshot.selected,entries:[{...entry,name:'geod_project_get'}]}}));
});
test('scientific review cards require RGB order, a shared grid and product-specific quality policies',()=>{
  const processing={product:'landsat-c2-l2',dataType:'UInt16',channels:3,rawBytes:36,requiredDiskBytes:8388700,quality:{product:'landsat-c2-l2',policy:'cloud_free_conservative',excludeSnow:true,coupled:false,sceneCount:1,sourceCount:5,sourceSha256:'c'.repeat(64)}};
  const plan={planId:'b1234567-1234-1234-1234-123456789abc',planHash:'a'.repeat(64),kind:'rgb',status:'pending',approvalRequired:true,source:'Verified local reflectance bands',bounds:[500000,4199940,500090,4200000],boundsCrs:'EPSG:32610',files:['red','green','blue'].map(assetKey=>({itemId:'Native scene',assetKey,bytes:1000,width:3,height:2})),expectedBytes:null,expiresAt:'2026-10-04T12:00:00Z',notes:[],jobs:[],processing};
  const snapshot={...base,mode:'review-first',plans:[plan]};assert.equal(validateAgentSnapshot(snapshot),snapshot);
  for(const broken of [{...plan,files:[plan.files[1],plan.files[0],plan.files[2]]},{...plan,processing:null},{...plan,boundsCrs:'EPSG:32600'},
    {...plan,processing:{...processing,quality:{...processing.quality,policy:'clear_best'}}},{...plan,processing:{...processing,requiredDiskBytes:1}},
    {...plan,files:plan.files.map((v,i)=>({...v,width:i===2?4:3}))}])assert.throws(()=>validateAgentSnapshot({...snapshot,plans:[broken]}));
});
test('Agent snapshots reject secret fields, forged references and unsupported modes',()=>{
  assert.equal(validateAgentSnapshot(base),base);
  for(const value of [{...base,apiKey:'secret'},{...base,mode:'write'},{...base,sessions:[{id:'javascript:alert(1)',title:'Bad',status:'completed'}]},
    {...base,selected:{id:'a1234567-1234-1234-1234-123456789abc',status:'completed',entries:[{type:'tool',status:'completed',name:'geod_projects_list',references:[{kind:'project',id:'https://evil.example/',label:'Fake'}]}]}}]) assert.throws(()=>validateAgentSnapshot(value));
});
test('review plans reject forged hashes and jobs while accepting native bounded data',()=>{
  const plan={planId:'b1234567-1234-1234-1234-123456789abc',planHash:'a'.repeat(64),kind:'download',status:'pending',approvalRequired:true,source:'Earth Search',bounds:[-122,37,-121,38],files:[{itemId:'Actual scene',assetKey:'scl',bytes:2000}],expectedBytes:2000,expiresAt:'2026-10-04T12:00:00Z',notes:[],jobs:[]};
  const value={...base,mode:'review-first',plans:[plan]};assert.equal(validateAgentSnapshot(value),value);
  for(const p of [{...plan,planHash:'trust me'},{...plan,jobs:[{id:'javascript:alert(1)'}]},{...plan,files:[]},{...plan,approvalRequired:false}])assert.throws(()=>validateAgentSnapshot({...value,plans:[p]}));
});
test('project and processing cards require a native project and a bounded raster grid',()=>{
  const project={id:'a1234567-1234-1234-1234-123456789abc',name:'Actual project',sceneCount:1,mode:'create',committed:false};
  const plan={planId:'b1234567-1234-1234-1234-123456789abc',planHash:'b'.repeat(64),kind:'project',status:'pending',approvalRequired:true,source:'Selected catalog scenes',bounds:[-122,37,-121,38],files:[{itemId:'Native scene',assetKey:'scene',bytes:null}],expectedBytes:null,expiresAt:'2026-10-04T12:00:00Z',notes:[],jobs:[],project};
  const value={...base,mode:'review-first',plans:[plan]};assert.equal(validateAgentSnapshot(value),value);
  for(const broken of [{...plan,project:{...project,id:'fake'}},{...plan,project:null},{...plan,files:Array(33).fill(plan.files[0])},{...plan,kind:'mosaic',files:[{itemId:'Native scene',assetKey:'scl',bytes:null,width:0,height:20}]}])assert.throws(()=>validateAgentSnapshot({...value,plans:[broken]}));
  const persisted={...plan,project:{...project,mode:'existing',saved:true}};
  assert.equal(validateAgentSnapshot({...value,plans:[persisted]}).plans[0].status,'pending');
  assert.throws(()=>validateAgentSnapshot({...value,plans:[{...plan,project:{...project,saved:'true'}}]}));
  const mosaic={...plan,kind:'mosaic',project:{...project,mode:'existing'},files:[{itemId:'Native scene',assetKey:'scl',bytes:null,width:46,height:56}]};
  assert.equal(validateAgentSnapshot({...value,plans:[mosaic]}).plans[0],mosaic);
});
