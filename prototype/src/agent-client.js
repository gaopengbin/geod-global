import { desktopAvailable } from './runtime-client.js';
import { MODEL_CAPABILITIES, MODEL_PROVIDERS, validProviderProtocol, validModelId, modelCapabilities } from './agent-model-registry.js';
import {officeExtension,audioExtension,audioSignature,videoExtension,videoSignature} from '../../agent/file-types.mjs';
import { validDecision, validTaskContext } from '../../agent/decisions.mjs';
import { validGoal } from '../../agent/goal-contract.mjs';
import { validatePlanMapPreview } from './agent-map-preview.js';
// Change notifications are hints. All displayed state still passes through
// the native snapshot validator and its permission-checked result cards.
export async function subscribeAgentUpdates(callback) {
  if (!window.__TAURI__?.event?.listen) return () => {};
  return window.__TAURI__.event.listen('geod-agent-changed', event => {
    if (Number.isSafeInteger(event.payload) && event.payload >= 0) callback();
  });
}
const UUID = /^[a-f0-9]{8}-[a-f0-9]{4}-[a-f0-9]{4}-[a-f0-9]{4}-[a-f0-9]{12}$/;
const STATUSES = ['starting', 'running', 'stopping', 'completed', 'failed', 'interrupted'];
const ASSET_KEYS = ['scl','visual','red','green','blue','elevation','srtm','aerial','vv','vh','hh','hv','ndvi','evi','vi_quality','vi_reliability','vi_doy','vi_red','vi_nir','vi_blue','vi_mir','vi_view_zenith','vi_sun_zenith','vi_relative_azimuth','modis_qc','modis_state','qa_pixel','qa_radsat','product','viirs'];
export function validAgentImage(value) {
  return value && Object.keys(value).every(key=>['id','name','mimeType','bytes','width','height'].includes(key))
    && /^[a-f0-9]{64}$/.test(value.id) && typeof value.name==='string' && value.name.trim() && [...value.name].length<=80 && !/[\u0000-\u001f\u007f/\\]/.test(value.name)
    && value.mimeType==='image/png' && Number.isInteger(value.bytes) && value.bytes>0 && value.bytes<=5*1024*1024
    && [value.width,value.height].every(n=>Number.isInteger(n) && n>0 && n<=1024);
}
export function validAgentContext(value) {
  return value && Object.keys(value).every(key=>['status','count','lastCompletedAt','usedTokens','windowTokens'].includes(key))
    && ['idle','organizing','ready','failed','interrupted'].includes(value.status) && Number.isSafeInteger(value.count) && value.count>=0 && value.count<=10000
    && (value.lastCompletedAt===null || typeof value.lastCompletedAt==='string' && value.lastCompletedAt.length<=64 && Number.isFinite(Date.parse(value.lastCompletedAt)))
    && (value.usedTokens===null || Number.isSafeInteger(value.usedTokens) && value.usedTokens>=0 && value.usedTokens<=1000000)
    && (value.windowTokens===null || Number.isSafeInteger(value.windowTokens) && value.windowTokens>0 && value.windowTokens<=1000000);
}
export function validateAgentImage(value) {
  if (!validAgentImage(value)) throw new Error('Agent returned invalid image data.');
  return value;
}
export function validAgentDocument(value) {
  return value && Object.keys(value).every(key=>['id','name','mimeType','bytes','characters','pages'].includes(key))
    && typeof value.id==='string' && /^[a-f0-9]{64}$/.test(value.id) && typeof value.name==='string' && value.name.trim() && [...value.name].length<=80
    && !/[\u0000-\u001f\u007f-\u009f/\\]/.test(value.name) && Number.isInteger(value.bytes) && value.bytes>0
    && (value.mimeType==='text/plain' && /\.(txt|md|csv|json|geojson)$/i.test(value.name) && value.bytes<=64*1024
      && Number.isInteger(value.characters) && value.characters>0 && value.characters<=value.bytes && value.pages===undefined
      || value.mimeType==='application/pdf' && /\.pdf$/i.test(value.name) && value.bytes<=2*1024*1024 && value.characters===0 && Number.isInteger(value.pages) && value.pages>0 && value.pages<=20
      || officeExtension(value) && value.name.toLowerCase().endsWith('.'+officeExtension(value)) && value.bytes<=2*1024*1024 && value.characters===0 && value.pages===undefined
      || audioExtension(value) && value.name.toLowerCase().endsWith('.'+audioExtension(value)) && value.bytes<=2*1024*1024 && value.characters===0 && value.pages===undefined
      || videoExtension(value) && value.name.toLowerCase().endsWith('.'+videoExtension(value)) && value.bytes<=8*1024*1024 && value.characters===0 && value.pages===undefined);
}
export function validateAgentDocument(value) {
  if(!validAgentDocument(value))throw Error('Agent returned invalid document data.');return value;
}
export function validateAgentDocumentPreview(value) {
  if(videoExtension(value?.document)){
    const info=value.video,prefix=`data:${value.document.mimeType};base64,`;
    if(Object.keys(value).some(key=>!['document','dataUrl','video'].includes(key)) || !validAgentDocument(value.document)
      || !info || Object.keys(info).some(key=>!['durationMs','width','height','hasAudio','codec'].includes(key))
      || !Number.isSafeInteger(info.durationMs) || info.durationMs<1 || info.durationMs>600_000
      || ![info.width,info.height].every(side=>Number.isInteger(side) && side>0 && side<=4096)
      || typeof info.hasAudio!=='boolean' || !(value.document.mimeType==='video/mp4'?info.codec==='H.264':['VP8','VP9'].includes(info.codec))
      || typeof value.dataUrl!=='string' || !value.dataUrl.startsWith(prefix) || value.dataUrl.length>11_300_000)throw Error('Agent returned invalid document data.');
    const encoded=value.dataUrl.slice(prefix.length);let binary;
    try{binary=atob(encoded);}catch{throw Error('Agent returned invalid document data.');}
    if(binary.length!==value.document.bytes || btoa(binary)!==encoded
      || !videoSignature(Uint8Array.from(binary,character=>character.charCodeAt(0)),videoExtension(value.document)))throw Error('Agent returned invalid document data.');
    return value;
  }
  if(audioExtension(value?.document)){
    const info=value.audio,prefix=`data:${value.document.mimeType};base64,`;
    if(Object.keys(value).some(key=>!['document','dataUrl','audio'].includes(key)) || !validAgentDocument(value.document)
      || !info || Object.keys(info).some(key=>!['durationMs','sampleRate','channels'].includes(key))
      || !Number.isSafeInteger(info.durationMs) || info.durationMs<1 || info.durationMs>600_000
      || !Number.isInteger(info.sampleRate) || info.sampleRate<8000 || info.sampleRate>192000
      || !Number.isInteger(info.channels) || info.channels<1 || info.channels>8
      || typeof value.dataUrl!=='string' || !value.dataUrl.startsWith(prefix) || value.dataUrl.length>2_800_000)throw Error('Agent returned invalid document data.');
    const encoded=value.dataUrl.slice(prefix.length);let binary;
    try{binary=atob(encoded);}catch{throw Error('Agent returned invalid document data.');}
    if(binary.length!==value.document.bytes || btoa(binary)!==encoded
      || !audioSignature(Uint8Array.from(binary,character=>character.charCodeAt(0)),audioExtension(value.document)))throw Error('Agent returned invalid document data.');
    return value;
  }
  if(officeExtension(value?.document)){
    if(Object.keys(value).some(key=>!['document','text','previewCharacters'].includes(key)) || !validAgentDocument(value.document) || typeof value.text!=='string'
      || !Number.isInteger(value.previewCharacters) || value.previewCharacters<0 || value.previewCharacters>256*1024 || [...value.text].length!==value.previewCharacters || new TextEncoder().encode(value.text).length>256*1024
      || /[\u0000-\u0008\u000b\u000c\u000e-\u001f\u007f-\u009f]/.test(value.text))throw Error('Agent returned invalid document data.');
    return value;
  }
  if(value?.document?.mimeType==='application/pdf'){
    if(Object.keys(value).some(key=>!['document','dataUrl'].includes(key)) || !validAgentDocument(value.document) || typeof value.dataUrl!=='string'
      || !/^data:application\/pdf;base64,[A-Za-z0-9+/]+={0,2}$/.test(value.dataUrl) || value.dataUrl.length>2_800_000)throw Error('Agent returned invalid document data.');
    const encoded=value.dataUrl.split(',')[1];let bytes;try{bytes=atob(encoded);}catch{throw Error('Agent returned invalid document data.');}
    if(bytes.length!==value.document.bytes || btoa(bytes)!==encoded || !bytes.startsWith('%PDF-'))throw Error('Agent returned invalid document data.');
    return value;
  }
  if(!value || Object.keys(value).some(key=>!['document','text'].includes(key)) || !validAgentDocument(value.document)
    || typeof value.text!=='string' || [...value.text].length!==value.document.characters || new TextEncoder().encode(value.text).length!==value.document.bytes
    || !value.text.trim() || /[\u0000-\u0008\u000b\u000c\u000e-\u001f\u007f-\u009f]/.test(value.text))throw Error('Agent returned invalid document data.');
  return value;
}
export function validateAgentImagePreview(value) {
  if (!value || Object.keys(value).some(key=>!['image','dataUrl'].includes(key)) || !validAgentImage(value.image)
    || typeof value.dataUrl!=='string' || value.dataUrl.length>7_000_000 || !/^data:image\/png;base64,[A-Za-z0-9+/]+={0,2}$/.test(value.dataUrl)) throw new Error('Agent returned invalid image data.');
  const encoded=value.dataUrl.slice('data:image/png;base64,'.length);
  let bytes; try { bytes=atob(encoded); } catch { throw new Error('Agent returned invalid image data.'); }
  if (bytes.length!==value.image.bytes || btoa(bytes)!==encoded || !bytes.startsWith('\x89PNG\r\n\x1a\n')) throw new Error('Agent returned invalid image data.');
  return value;
}
export function validateAgentSnapshot(value) {
  if (!value || value.version !== 1 || !['read-only','review-first'].includes(value.mode) || typeof value.configured !== 'boolean'
    || typeof value.runtimeAvailable !== 'boolean' || typeof value.busy !== 'boolean'
    || !Array.isArray(value.sessions) || value.sessions.length > 50 || !Number.isSafeInteger(value.revision)
    || value.sessions.some(session => !UUID.test(session.id) || typeof session.title !== 'string' || !STATUSES.includes(session.status))
    || JSON.stringify(value).length > 1_000_000 || /"(?:apiKey|password|credential|secret)"\s*:/.test(JSON.stringify(value))) throw new Error('Agent returned invalid conversation data.');
  if (value.selected && (!UUID.test(value.selected.id) || !STATUSES.includes(value.selected.status) || !Array.isArray(value.selected.entries)
    || value.selected.contextState!==undefined && !validAgentContext(value.selected.contextState)
    || value.selected.entries.length > 200 || value.selected.entries.some(entry => !['user', 'assistant', 'tool', 'system'].includes(entry.type)
      || entry.decision!==undefined && (entry.type!=='tool' || entry.name!=='geod_request_decision' || !validDecision(entry.decision))
      || entry.taskContext!==undefined && (entry.type!=='tool' || !validTaskContext(entry.taskContext))
      || entry.type==='system' && (entry.origin!=='desktop' || entry.status!=='completed' || typeof entry.text!=='string' || entry.text.length>400)
      || !['running', 'completed', 'failed', 'interrupted'].includes(entry.status)
      || (entry.type !== 'tool' && typeof entry.text !== 'string')
      || entry.images!==undefined && (entry.type!=='user' || !Array.isArray(entry.images) || entry.images.length>3 || entry.images.some(image=>!validAgentImage(image)) || new Set(entry.images.map(image=>image.id)).size!==entry.images.length)
      || entry.documents!==undefined && (entry.type!=='user' || !Array.isArray(entry.documents) || entry.documents.length>3 || entry.documents.some(document=>!validAgentDocument(document)) || new Set(entry.documents.map(document=>document.id)).size!==entry.documents.length)
      || (entry.images?.length??0)+(entry.documents?.length??0)>3
      || (entry.type === 'tool' && (!/^geod_[a-z_]+$/.test(entry.name) || !Array.isArray(entry.references)
        || entry.references.some(reference => !['job', 'project', 'plan', 'vector'].includes(reference.kind) || !UUID.test(reference.id) || typeof reference.label !== 'string')
        || entry.summary?.kind === 'sources' && (entry.name !== 'geod_sources_list' || !validSourceSummary(entry.summary))
        || ['vector','vectors'].includes(entry.summary?.kind) && !validVectorSummary(entry)))))) throw new Error('Agent returned invalid conversation data.');
  if (value.plans !== undefined && (!Array.isArray(value.plans) || value.plans.length > 10 || value.plans.some(plan => !validPlan(plan)))) throw new Error('Agent returned invalid plan data.');
  if(value.execution!==undefined&&(!value.execution||Object.keys(value.execution).some(key=>!['mode','defaultMode','scope','modelCanChangePermission'].includes(key))
    || ![value.execution.mode,value.execution.defaultMode].every(mode=>['confirm-each','full-access'].includes(mode))
    || value.execution.scope!=='managed-projects-and-files'||value.execution.modelCanChangePermission!==false))throw Error('Agent returned invalid execution permission.');
  if([...value.sessions,...(value.selected?[value.selected]:[])].some(session=>session.workflow!==undefined&&!validAgentWorkflow(session.workflow)))throw Error('Agent returned invalid workflow state.');
  if([...value.sessions,...(value.selected?[value.selected]:[])].some(session=>session.goal!==undefined&&!validGoal(session.goal)))throw Error('Agent returned invalid goal state.');
  if(value.selected?.workspaceView!==undefined&&!validWorkspaceView(value.selected.workspaceView))throw Error('Agent returned invalid workspace view request.');
  if (value.registry !== undefined) {
    const registry = value.registry;
    if (!registry || Object.keys(registry).some(key => !['version','selectedId','connections'].includes(key)) || registry.version !== 1
      || !Array.isArray(registry.connections) || registry.connections.length > 16 || registry.connections.some(connection => !validConnection(connection))
      || new Set(registry.connections.map(connection => connection.id)).size !== registry.connections.length
      || registry.selectedId !== null && !registry.connections.some(connection => connection.id === registry.selectedId)
      || value.model != null && (!validConnection(value.model) || value.model.id !== registry.selectedId
        || ['provider','label','protocol','baseUrl','model','verification'].some(key => value.model[key] !== registry.connections.find(connection => connection.id === registry.selectedId)?.[key]))) throw new Error('Agent returned invalid connection data.');
  }
  return value;
}
function validAgentWorkflow(value){
  return value&&Object.keys(value).every(key=>['status','waitingPlans','continuations','updatedAt'].includes(key))
    && ['waiting','ready','paused','failed','completed'].includes(value.status)
    && Number.isSafeInteger(value.waitingPlans)&&value.waitingPlans>=0&&value.waitingPlans<=10
    && Number.isSafeInteger(value.continuations)&&value.continuations>=0&&value.continuations<=8
    && typeof value.updatedAt==='string'&&value.updatedAt.length<=64&&Number.isFinite(Date.parse(value.updatedAt));
}
function validWorkspaceView(value){return value&&Object.keys(value).every(key=>['requestId','kind','id','verified','acknowledged'].includes(key))&&UUID.test(value.requestId)&&UUID.test(value.id)&&['raster','vector'].includes(value.kind)&&value.verified===true&&typeof value.acknowledged==='boolean';}
function validConnection(value) {
  if (!value || Object.keys(value).some(key => !['id','provider','label','protocol','baseUrl','model','capabilities','verification'].includes(key))
    || !UUID.test(value.id) || !MODEL_PROVIDERS.some(provider => provider.id === value.provider)
    || !validProviderProtocol(value.provider,value.protocol) || typeof value.label !== 'string' || !value.label.trim() || value.label.length > 80
    || !validModelId(value.protocol,value.model)
    || typeof value.baseUrl !== 'string' || value.baseUrl.length > 1000 || value.verification !== 'not-verified'
    || !value.capabilities || Object.keys(value.capabilities).length !== Object.keys(MODEL_CAPABILITIES).length
    || Object.entries(modelCapabilities(value.protocol)).some(([key,enabled]) => value.capabilities[key] !== enabled)) return false;
  try { const url = new URL(value.baseUrl); return !url.username && !url.password && !url.search && !url.hash && (url.protocol === 'https:' || url.protocol === 'http:' && ['127.0.0.1','[::1]'].includes(url.hostname)); } catch { return false; }
}
function validSourceSummary(value) {
  const timestamp = value => typeof value === 'string' && value.length <= 64 && Number.isFinite(Date.parse(value));
  return Object.keys(value).every(key => ['kind','count','checkedAt','accounts'].includes(key))
    && Number.isInteger(value.count) && value.count >= 0 && value.count <= 32 && timestamp(value.checkedAt)
    && Array.isArray(value.accounts) && value.accounts.length === 2 && new Set(value.accounts.map(account => account?.provider)).size === 2
    && value.accounts.every(account => account && Object.keys(account).every(key => ['provider','status','expiresAt','verifiedAt'].includes(key))
      && ['nasa-earthdata','copernicus'].includes(account.provider)
      && ['not-connected','saved','connected','expired','storage-error','unsupported','unavailable'].includes(account.status)
      && (account.expiresAt === null || timestamp(account.expiresAt)) && (account.verifiedAt === null || timestamp(account.verifiedAt)));
}
function validVectorSummary(entry) {
  const value=entry.summary;
  if (!Number.isInteger(value.count) || value.count < 0) return false;
  if (value.kind==='vectors') return entry.name==='geod_vectors_list' && value.verified===false && value.count<=20
    && Number.isInteger(value.total) && value.total>=value.count && value.total<=1024
    && Object.keys(value).every(key=>['kind','count','total','verified'].includes(key));
  return entry.name==='geod_vector_inspect' && value.verified===true && value.count<=50000 && /^[a-f0-9]{64}$/.test(value.sha256 ?? '')
    && Object.keys(value).every(key=>['kind','count','verified','sha256'].includes(key));
}
function validPlan(plan) {
  if (!plan || !UUID.test(plan.planId)) return false;
  if (plan.status === 'unavailable') return Object.keys(plan).every(key => ['planId','status'].includes(key));
  if (plan.kind === 'vector') return validVectorPlan(plan);
  const projectValid = plan.project == null || UUID.test(plan.project.id) && typeof plan.project.name === 'string'
    && ['create','append','existing'].includes(plan.project.mode) && typeof plan.project.committed === 'boolean'
    && (plan.project.saved === undefined || typeof plan.project.saved === 'boolean')
    && Number.isInteger(plan.project.sceneCount) && plan.project.sceneCount >= (plan.project.mode === 'existing' ? 0 : 1) && plan.project.sceneCount <= 32;
  return /^[a-f0-9]{64}$/.test(plan.planHash) && ['download','clip','project','mosaic','rgb'].includes(plan.kind)
    && ['pending','submitted','expired','superseded'].includes(plan.status) && plan.approvalRequired === true
    && (plan.status === 'superseded' ? UUID.test(plan.replacedBy) && plan.replacedBy !== plan.planId : plan.replacedBy == null)
    && typeof plan.source === 'string' && Array.isArray(plan.bounds) && plan.bounds.length === 4 && plan.bounds.every(Number.isFinite)
    && (plan.boundsCrs === undefined || ['EPSG:4326','MODIS:Sinusoidal','VIIRS:Sinusoidal'].includes(plan.boundsCrs) || /^EPSG:(?:326|327)(?:0[1-9]|[1-5]\d|60)$/.test(plan.boundsCrs))
    && (plan.polygon == null || /^[a-f0-9]{64}$/.test(plan.polygon.sha256) && Array.isArray(plan.polygon.bounds) && plan.polygon.bounds.length === 4 && plan.polygon.bounds.every(Number.isFinite))
    && (plan.areaCoverage == null || validAreaCoverage(plan.areaCoverage))
    && projectValid && (!['project','mosaic'].includes(plan.kind) || plan.project != null)
    && Array.isArray(plan.files) && plan.files.length >= 1 && plan.files.length <= 32
    && (plan.authorization == null || plan.kind === 'download' && validSourceAuthorization(plan.authorization)
      && plan.expectedBytes === null && plan.files.every(file => file.bytes === null)
      && (plan.authorization.provider === 'copernicus'
        ? plan.format === 'SAFE ZIP' && plan.files.every(file => file.assetKey === 'product')
        : ['GeoTIFF','HGT ZIP','HDF5'].includes(plan.format) && plan.files.every(file =>
          (plan.format === 'GeoTIFF' ? ['red','green','blue'] : plan.format === 'HGT ZIP' ? ['srtm'] : ['viirs']).includes(file.assetKey))))
    && plan.files.every(file => typeof file.itemId === 'string' && (plan.kind === 'project' ? file.assetKey === 'scene' : ASSET_KEYS.includes(file.assetKey) || plan.kind === 'download' && (file.assetKey === 'stac_asset' && validCustomFile(file) || file.assetKey === 'wcs_coverage' && validCoverageFile(file))) && (file.bytes === null || Number.isSafeInteger(file.bytes) && file.bytes > 0)
      && (file.referenceId === undefined || validCustomFile(file) || validCoverageFile(file))
      && (!['clip','mosaic','rgb'].includes(plan.kind) || Number.isInteger(file.width) && file.width > 0 && Number.isInteger(file.height) && file.height > 0))
    && (plan.kind !== 'rgb' || plan.files.length === 3 && plan.files.every((file,index) => file.assetKey === ['red','green','blue'][index] && file.width === plan.files[0].width && file.height === plan.files[0].height) && plan.processing?.channels === 3)
    && (plan.processing == null || validProcessing(plan.processing))
    && (plan.expectedBytes === null || Number.isSafeInteger(plan.expectedBytes) && plan.expectedBytes > 0)
    && typeof plan.expiresAt === 'string' && Number.isFinite(Date.parse(plan.expiresAt))
    && Array.isArray(plan.notes) && plan.notes.every(note => typeof note === 'string')
    && Array.isArray(plan.jobs) && plan.jobs.length <= 32 && plan.jobs.every(job => UUID.test(job.id) && typeof job.title === 'string'
      && ['queued','running','succeeded','failed','cancelled','interrupted'].includes(job.status) && typeof job.settled === 'boolean'
      && (job.totalBytes==null || Number.isSafeInteger(job.totalBytes) && job.totalBytes>=0)
      && Number.isSafeInteger(job.bytesDownloaded) && job.bytesDownloaded >= 0 && (job.sha256 === null || /^[a-f0-9]{64}$/.test(job.sha256)));
}
function validAreaCoverage(value) {
  const extent=b=>Array.isArray(b)&&b.length===4&&b.every(Number.isFinite)&&b[0]<b[2]&&b[1]<b[3];
  return value&&['complete','partial','unknown'].includes(value.status)&&value.basis==='catalog-footprints'
    && ['polygon','rectangle'].includes(value.target)&&value.areaMethod==='planar-wgs84'
    && /^[a-f0-9]{64}$/.test(value.scopeSha256)&&Number.isFinite(value.coveredFraction)&&value.coveredFraction>=0&&value.coveredFraction<=1
    && (value.status!=='complete'||value.coveredFraction===1&&value.missingParts===0)
    && Number.isInteger(value.sceneCount)&&value.sceneCount>=0&&value.sceneCount<=200
    && Array.isArray(value.unknownItemIds)&&value.unknownItemIds.length<=200&&value.unknownItemIds.every(id=>typeof id==='string'&&id.length<=300)
    && Array.isArray(value.missingBounds)&&value.missingBounds.length<=8&&value.missingBounds.every(extent)
    && Number.isInteger(value.missingParts)&&value.missingParts>=0&&typeof value.note==='string';
}
function validSourceAuthorization(value) {
  const timestamp = v => v == null || typeof v === 'string' && v.length <= 64 && Number.isFinite(Date.parse(v));
  return value && Object.keys(value).every(key => ['provider','status','expiresAt','verifiedAt','downloadEnabled','entitlement'].includes(key))
    && ['nasa-earthdata','copernicus'].includes(value.provider)
    && ['not-connected','saved','connected','expired','storage-error','unsupported','unavailable'].includes(value.status)
    && timestamp(value.expiresAt) && timestamp(value.verifiedAt) && value.entitlement === 'not-checked'
    && typeof value.downloadEnabled === 'boolean'
    && (!value.downloadEnabled || ['saved','connected'].includes(value.status) && value.expiresAt != null && value.verifiedAt != null);
}
function validVectorPlan(plan) {
  const hash=value=>typeof value==='string' && /^[a-f0-9]{64}$/.test(value);
  const bounds=value=>Array.isArray(value) && value.length===4 && value.every(Number.isFinite) && value[0]<value[2] && value[1]<value[3] && value[0]>=-180 && value[2]<=180 && value[1]>=-90 && value[3]<=90;
  const review=plan.vectorReview, result=plan.vector;
  return Object.keys(plan).every(key=>['planId','planHash','kind','status','source','bounds','boundsCrs','polygon','replacedBy','expiresAt','approvalRequired','expectedBytes','jobs','notes','vectorReview','vector'].includes(key))
    && hash(plan.planHash) && ['pending','submitted','expired','superseded'].includes(plan.status) && plan.approvalRequired===true
    && (plan.status==='superseded' ? UUID.test(plan.replacedBy) && plan.replacedBy!==plan.planId : plan.replacedBy==null)
    && typeof plan.source==='string' && bounds(plan.bounds) && plan.boundsCrs==='EPSG:4326'
    && (plan.polygon==null || hash(plan.polygon.sha256) && bounds(plan.polygon.bounds))
    && typeof plan.expiresAt==='string' && Number.isFinite(Date.parse(plan.expiresAt)) && plan.expectedBytes===null
    && Array.isArray(plan.jobs) && plan.jobs.length===0 && Array.isArray(plan.notes) && plan.notes.every(note=>typeof note==='string')
    && review && Object.keys(review).every(key=>['serviceId','serviceSha256','collectionId','collectionTitle','protocol','name','pageSize','responseFormat','selection','liveAvailabilityChecked','clipped'].includes(key))
    && UUID.test(review.serviceId) && hash(review.serviceSha256) && typeof review.collectionId==='string' && review.collectionId.length>0 && review.collectionId.length<=160
    && typeof review.collectionTitle==='string' && review.collectionTitle.length<=480 && typeof review.name==='string' && review.name.trim().length>0 && review.name.length<=240
    && ['OGC API Features','ArcGIS','WFS 2','Overpass'].includes(review.protocol)
    && (review.protocol==='Overpass' ? review.pageSize===null : Number.isInteger(review.pageSize) && review.pageSize>=1 && review.pageSize<=200)
    && (review.responseFormat==null || review.protocol==='WFS 2' && typeof review.responseFormat==='string' && review.responseFormat.length>0 && review.responseFormat.length<=512)
    && review.selection===({Overpass:'overpass-bbox-full-geometry','WFS 2':'wfs-bbox-full-features'}[review.protocol] || 'bbox-full-features') && review.liveAvailabilityChecked===false && review.clipped===false
    && (plan.status==='submitted' ? result && Object.keys(result).every(key=>['id','name','format','bytes','featureCount','coordinateCount','sourceSha256','geojsonSha256','verified'].includes(key))
      && UUID.test(result.id) && result.name===review.name && ['geojson','wfs-snapshot','overpass-json'].includes(result.format) && result.verified===true && hash(result.sourceSha256) && hash(result.geojsonSha256)
      && Number.isInteger(result.bytes) && result.bytes>0 && result.bytes<=20971520 && Number.isInteger(result.featureCount) && result.featureCount>=0 && result.featureCount<=50000
      && Number.isInteger(result.coordinateCount) && result.coordinateCount>=0 && result.coordinateCount<=500000 : result===null);
}
function validCustomFile(file) {
  return /^[a-f0-9]{64}$/.test(file.referenceId) && /^[a-f0-9]{64}$/.test(file.snapshotId)
    && typeof file.originalAssetKey === 'string' && file.originalAssetKey.length > 0 && file.originalAssetKey.length <= 512 && !/[\u0000-\u001f\u007f]/.test(file.originalAssetKey);
}
function validCoverageFile(file) {
  const bounds=value=>Array.isArray(value) && value.length===4 && value.every(Number.isFinite) && value[0]<value[2] && value[1]<value[3]
    && value[0]>=-180 && value[2]<=180 && value[1]>=-90 && value[3]<=90;
  return Object.keys(file).every(key=>['itemId','assetKey','referenceId','coveragePlanId','bytes','width','height','crs','requestedBounds','alignedBounds','selection'].includes(key))
    && /^[a-f0-9]{64}$/.test(file.coveragePlanId) && file.referenceId===file.coveragePlanId
    && ['scene','wcs_coverage'].includes(file.assetKey) && file.bytes===null && file.itemId.length>0 && file.itemId.length<=512
    && [file.width,file.height].every(value=>Number.isInteger(value) && value>0 && value<=65536)
    && (['EPSG:4326','EPSG:3857'].includes(file.crs) || /^EPSG:(?:326|327)(?:0[1-9]|[1-5]\d|60)$/.test(file.crs))
    && bounds(file.requestedBounds) && bounds(file.alignedBounds) && file.selection==='bbox-native-grid';
}
function validProcessing(value) {
  const products = ['landsat-c2-l2','hls-l30-v2','modis-09a1-v061','viirs-09a1-v002','modis-13q1-v061'];
  if (!products.includes(value.product) || !['Int16','UInt16'].includes(value.dataType) || ![1,3].includes(value.channels)
    || !Number.isSafeInteger(value.rawBytes) || value.rawBytes <= 0
    || value.requiredDiskBytes !== null && (!Number.isSafeInteger(value.requiredDiskBytes) || value.requiredDiskBytes < value.rawBytes)) return false;
  const quality = value.quality;
  if (quality == null) return true;
  const policies = {'landsat-c2-l2':['cloud_free','cloud_free_conservative'],'modis-09a1-v061':['clear','clear_best'],'modis-13q1-v061':['good','usable']};
  return quality.product === value.product && policies[quality.product]?.includes(quality.policy)
    && typeof quality.excludeSnow === 'boolean' && typeof quality.coupled === 'boolean'
    && Number.isInteger(quality.sceneCount) && quality.sceneCount >= 1 && quality.sceneCount <= 32
    && Number.isInteger(quality.sourceCount) && quality.sourceCount >= 4 && quality.sourceCount <= 160
    && /^[a-f0-9]{64}$/.test(quality.sourceSha256);
}
export function validateConnectionTest(value) {
  if (!value || Object.keys(value).some(key=>!['version','status','text','functionCalls','latencyMs','checkedAt','message'].includes(key))
    || value.version!==1 || !['passed','failed'].includes(value.status)
    || typeof value.text!=='boolean' || typeof value.functionCalls!=='boolean'
    || !Number.isSafeInteger(value.latencyMs) || value.latencyMs<0 || value.latencyMs>30_000
    || typeof value.checkedAt!=='string' || value.checkedAt.length>64 || !Number.isFinite(Date.parse(value.checkedAt))
    || value.status==='passed' && (!value.text || !value.functionCalls || value.message!==undefined)
    || value.status==='failed' && (value.functionCalls || typeof value.message!=='string' || !value.message.trim() || value.message.length>240)) {
    throw Error('Agent returned invalid connection test data.');
  }
  return value;
}
export async function agentRequest(operation, args = {}) {
  if (!desktopAvailable()) throw new Error('Agent is available in the desktop app.');
  const methods = { snapshot: 'agent_snapshot', saveModel: 'agent_save_model', testModel:'agent_test_model', send: 'agent_send', select: 'agent_select', interrupt: 'agent_interrupt', approvePlan: 'agent_approve_plan', revisionDraft:'agent_approve_plan', revisePlan:'agent_approve_plan', attachImage:'agent_attach_image', imagePreview:'agent_image_preview', imageStorage:'agent_image_storage', attachDocument:'agent_attach_document', documentPreview:'agent_document_preview', attachmentStorage:'agent_attachment_storage', compact:'agent_compact' };
  methods.executionMode='agent_execution_mode';
  methods.acknowledgeView='agent_acknowledge_view';
  methods.goalControl='agent_goal_control';
  methods.planMapPreview='agent_plan_map_preview';
  const method = methods[operation]; if (!method) throw new Error('Unknown Agent operation.');
  const request=operation==='revisionDraft'?{...args,action:'draft'}:operation==='revisePlan'?{...args,action:'revise'}:args;
  const result=await window.__TAURI__.core.invoke(method, request);
  if(operation==='planMapPreview')return validatePlanMapPreview(result);
  return operation==='testModel'?validateConnectionTest(result):operation==='attachImage'?validateAgentImage(result):operation==='imagePreview'?validateAgentImagePreview(result):operation==='imageStorage'?validateAgentImageStorage(result):operation==='attachDocument'?validateAgentDocument(result):operation==='documentPreview'?validateAgentDocumentPreview(result):operation==='attachmentStorage'?validateAgentAttachmentStorage(result):operation==='revisionDraft'?validatePlanRevisionDraft(result):validateAgentSnapshot(result);
}
export function validateAgentImageStorage(value) {
  const keys=['limitBytes','usedBytes','imageCount','unusedBytes','unusedCount','removedBytes','removedCount'];
  if (!value || Object.keys(value).some(key=>!keys.includes(key)) || !keys.every(key=>Number.isSafeInteger(value[key]) && value[key]>=0)
    || value.limitBytes!==128*1024*1024 || value.unusedBytes>value.usedBytes || value.unusedCount>value.imageCount) throw Error('Agent returned invalid image storage data.');
  return value;
}
export function validateAgentAttachmentStorage(value) {
  if(!value || Object.keys(value).some(key=>!['images','documents'].includes(key)))throw Error('Agent returned invalid attachment storage data.');
  validateAgentImageStorage(value.images);
  const document=value.documents,keys=['limitBytes','usedBytes','documentCount','unusedBytes','unusedCount','removedBytes','removedCount'];
  if(!document || Object.keys(document).some(key=>!keys.includes(key)) || !keys.every(key=>Number.isSafeInteger(document[key]) && document[key]>=0)
    || document.limitBytes!==32*1024*1024 || document.unusedBytes>document.usedBytes || document.unusedCount>document.documentCount)throw Error('Agent returned invalid attachment storage data.');
  return value;
}
export function validatePlanRevisionDraft(value) {
  const fail=()=>{throw new Error('Agent returned invalid review form data.');};
  const keys=(value,allowed)=>value && typeof value==='object' && !Array.isArray(value) && Object.keys(value).every(key=>allowed.includes(key));
  const text=value=>typeof value==='string' && value.length>0 && value.length<=300;
  const bounds=value=>Array.isArray(value) && value.length===4 && value.every(Number.isFinite) && value[0]<value[2] && value[1]<value[3];
  const crs=value=>['EPSG:4326','MODIS:Sinusoidal','VIIRS:Sinusoidal'].includes(value) || /^EPSG:(?:326|327)(?:0[1-9]|[1-5]\d|60)$/.test(value);
  if(!keys(value,['planId','planHash','kind','parameters','fields','boundsCrs','projectId']) || !UUID.test(value.planId) || !/^[a-f0-9]{64}$/.test(value.planHash)
    || !['clip','download','project','mosaic','rgb','vector'].includes(value.kind) || JSON.stringify(value).length>50_000
    || value.boundsCrs!==undefined && !crs(value.boundsCrs) || value.projectId!=null && !UUID.test(value.projectId)
    || !keys(value.fields,['name','bounds','polygon','items','assetKeys','qualityPolicies','snow'])) fail();
  const p=value.parameters,f=value.fields;
  for(const flag of ['name','bounds','polygon','snow']) if(f[flag]!==undefined && typeof f[flag]!=='boolean') fail();
  if(f.items!==undefined && (!Array.isArray(f.items) || f.items.length<1 || f.items.length>32 || f.items.some(item=>!keys(item,['id','date','locked','assetKey','label']) || !text(item.id) || typeof item.locked!=='boolean'
    || item.label!==undefined && (typeof item.label!=='string' || !item.label.length || item.label.length>1024)
    || item.date!=null && (typeof item.date!=='string' || !Number.isFinite(Date.parse(item.date))) || item.assetKey!==undefined && !ASSET_KEYS.includes(item.assetKey)))) fail();
  if(f.assetKeys!==undefined && (!Array.isArray(f.assetKeys) || !f.assetKeys.length || f.assetKeys.length>ASSET_KEYS.length || new Set(f.assetKeys).size!==f.assetKeys.length || f.assetKeys.some(key=>!ASSET_KEYS.includes(key)))) fail();
  if(f.qualityPolicies!==undefined && (!Array.isArray(f.qualityPolicies) || !f.qualityPolicies.length || f.qualityPolicies.length>2 || new Set(f.qualityPolicies).size!==f.qualityPolicies.length
    || f.qualityPolicies.some(policy=>!['cloud_free','cloud_free_conservative','clear','clear_best','good','usable'].includes(policy)))) fail();
  const shape={clip:['kind','bounds','name','keepPolygon'],vector:['kind','bounds','name','keepPolygon'],download:['kind','itemIds'],project:['kind','itemIds','name','bounds','keepPolygon'],mosaic:['kind','assetKey','qualityPolicy'],rgb:['kind','name','qualityPolicy','excludeSnow']}[value.kind];
  if(!keys(p,shape) || p.kind!==value.kind || f.name && !text(p.name) || f.bounds && (!bounds(p.bounds) || !crs(value.boundsCrs))
    || ['clip','project','vector'].includes(value.kind) && typeof p.keepPolygon!=='boolean'
    || ['download','project'].includes(value.kind) && (!f.items || !Array.isArray(p.itemIds) || !p.itemIds.length || p.itemIds.some(id=>!f.items.some(item=>item.id===id)) || f.items.some(item=>item.locked && !p.itemIds.includes(item.id)))
    || value.kind==='mosaic' && (!f.assetKeys?.includes(p.assetKey))
    || f.qualityPolicies && !f.qualityPolicies.includes(p.qualityPolicy) || !f.qualityPolicies && p.qualityPolicy!=null
    || f.snow && typeof p.excludeSnow!=='boolean' || !f.snow && p.excludeSnow!=null) fail();
  return value;
}
