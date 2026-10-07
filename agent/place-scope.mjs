// A model's attempted city lookup establishes a bounded acquisition scope for
// this turn. Only a successful native city result can unlock imagery search;
// a same-named state or the unrelated current map cannot replace it.
export const STORAGE_SCOPE_ERROR='Geographic lookup storage failed for this turn. Stop geographic retries.';
export const CITY_SCOPE_ERROR='Resolve the requested city extent before imagery search. Do not substitute a state or current map.';
const geographic=new Set(['geod_place_search','geod_region_search','geod_region_levels','geod_scene_search']);
const storageErrors=new Set(['Agent record directory was redirected.','Managed storage root changed']);
const bounds=value=>Array.isArray(value)&&value.length===4&&value.every(Number.isFinite)
  &&value[0]>=-180&&value[0]<value[2]&&value[2]<=180&&value[1]>=-90&&value[1]<value[3]&&value[3]<=90;
export function beforePlaceTool(active,name,args){
  const scope=active.placeScope??={cityRequested:false,cityBounds:[],storageFailed:false};
  if(scope.storageFailed&&geographic.has(name))throw Error(STORAGE_SCOPE_ERROR);
  if(name==='geod_scene_search'&&scope.cityRequested
    &&(!bounds(args?.bounds)||!scope.cityBounds.some(extent=>extent.every((n,i)=>Math.abs(n-args.bounds[i])<=1e-6))))throw Error(CITY_SCOPE_ERROR);
  if(name==='geod_place_search'&&args?.kind==='city'){scope.cityRequested=true;scope.cityBounds=[];}
}
export function afterPlaceTool(active,name,result){
  if(name!=='geod_place_search'||!active.placeScope?.cityRequested)return;
  active.placeScope.cityBounds=(result?.candidates??[]).filter(c=>c.kind==='city'&&bounds(c.bounds)).map(c=>[...c.bounds]);
}
export function failedPlaceTool(active,error){
  if(storageErrors.has(error?.message)&&active.placeScope)active.placeScope.storageFailed=true;
}
// Persist only fixed error identifiers. Provider bodies, private paths and
// arbitrary error strings never become visible tool labels or saved history.
export function toolFailureCode(error){
  const message=error?.message;
  if(['Answer the pending decision card before preparing or executing dependent plans.','Answer the pending decision card before confirming the download task.'].includes(message))return 'pending-decision';
  if(message==='Revise the persisted task goal for the latest crop request before preparing its project.')return 'task-goal';
  if(['Choose the crop area through a decision card before preparing the project.','The requested boundary crop needs a verified native polygon before preparing the project.'].includes(message))return 'crop-area';
  if(message==='Scene search is stale or belongs to another conversation. Search again.')return 'stale-search';
  if(['Agent plan expired. Create a new plan before confirming.','Agent plan expired. Create a new plan.'].includes(message))return 'expired-plan';
  if(storageErrors.has(message)||message===STORAGE_SCOPE_ERROR)return 'record-storage';
  if(message===CITY_SCOPE_ERROR)return 'city-scope';
  if(['Place lookup timed out.','Place lookup is temporarily unreachable.','Administrative lookup timed out.',
    'Administrative lookup is temporarily unreachable.','GeoD tool timed out.','Place response interrupted.'].includes(message))return 'source-network';
  return 'tool-failed';
}
