// A model's attempted city lookup establishes a bounded acquisition scope for
// this turn. Correcting a district lookup requires matching administrative
// provenance and its verified polygon; a state/current map cannot replace it.
export const STORAGE_SCOPE_ERROR='Geographic lookup storage failed for this turn. Stop geographic retries.';
export const CITY_SCOPE_ERROR='Resolve the requested city extent before imagery search. Do not substitute a state or current map.';
const searches=new Set(['geod_scene_search','geod_stac_search','geod_dataset_resolve','geod_dataset_connect']);
const geographic=new Set(['geod_place_search','geod_region_search','geod_region_levels',...searches]);
const storageErrors=new Set(['Agent record directory was redirected.','Managed storage root changed']);
const bounds=value=>Array.isArray(value)&&value.length===4&&value.every(Number.isFinite)
  &&value[0]>=-180&&value[0]<value[2]&&value[2]<=180&&value[1]>=-90&&value[1]<value[3]&&value[3]<=90;
const placeName=value=>typeof value==='string'?value.normalize('NFKC').trim().toLowerCase():'';
export function beforePlaceTool(active,name,args){
  const scope=active.placeScope??={cityRequested:false,cityBounds:[],storageFailed:false};
  if(scope.storageFailed&&geographic.has(name))throw Error(STORAGE_SCOPE_ERROR);
  if(searches.has(name)&&scope.cityRequested){
    const exact=extent=>extent.every((n,i)=>Math.abs(n-args.bounds[i])<=1e-6);
    const contained=area=>scope.cityBounds.some(city=>area[0]>=city[0]-1e-6&&area[1]>=city[1]-1e-6&&area[2]<=city[2]+1e-6&&area[3]<=city[3]+1e-6);
    if(!bounds(args?.bounds)||!(scope.cityBounds.some(exact) || (scope.resolvedAdministrativeBounds??[]).some(exact)
      ||(scope.boundaryBounds??[]).some(area=>exact(area)&&contained(area))))throw Error(CITY_SCOPE_ERROR);
  }
  if(name==='geod_place_search')scope.lookupKind=args?.kind;
  if(name==='geod_place_search'&&args?.kind==='city'){
    if(scope.cityQuery!==placeName(args.query)&&scope.administrativeQuery!==placeName(args.query)){scope.administrativeCandidates=[];scope.resolvedAdministrativeBounds=[];}
    scope.cityQuery=placeName(args.query);scope.cityRequested=true;scope.cityBounds=[];
  }
}
export function afterPlaceTool(active,name,result){
  const scope=active.placeScope;
  if(name==='geod_region_search'&&scope&&placeName(result?.query)&&(!scope.cityRequested||placeName(result.query)===scope.cityQuery)){
    // A county/district correction must resolve the same canonical name at a
    // detailed level and later return its matching verified native polygon.
    scope.administrativeQuery=placeName(result.query);
    scope.administrativeCandidates=(result.candidates??[]).filter(c=>Number.isSafeInteger(c.adminLevel)&&c.adminLevel>=2
      &&typeof c.id==='string'&&typeof c.countryCode==='string'&&placeName(c.name)===scope.administrativeQuery&&bounds(c.bounds));
  }
  if(name==='geod_boundary_read'&&active.placeScope&&bounds(result?.bounds)
    &&['Polygon','MultiPolygon'].includes(result.geometryType)
    &&typeof result.boundary?.id==='string'&&/^[a-f0-9]{64}$/i.test(result.boundary?.sha256??'')){
    const areas=active.placeScope.boundaryBounds??=[];
    if(!areas.some(a=>a.every((n,i)=>n===result.bounds[i])))areas.push([...result.bounds]);
    if(scope.administrativeCandidates?.some(c=>c.countryCode===result.provenance?.countryCode
      &&c.adminLevel===result.provenance?.adminLevel&&placeName(c.name)===placeName(result.name)
      &&c.bounds.every((n,i)=>Math.abs(n-result.bounds[i])<=1e-6))){
      const resolved=scope.resolvedAdministrativeBounds??=[];
      if(!resolved.some(a=>a.every((n,i)=>n===result.bounds[i])))resolved.push([...result.bounds]);
    }
  }
  if(name!=='geod_place_search'||!active.placeScope?.cityRequested)return;
  // Resolving a borough/POI after its parent city must not erase that city's
  // successful native scope. Only a city lookup can replace the city results.
  if((result?.kind??active.placeScope.lookupKind)&&(result?.kind??active.placeScope.lookupKind)!=='city')return;
  active.placeScope.cityBounds=(result?.candidates??[]).filter(c=>c.kind==='city'&&bounds(c.bounds)).map(c=>[...c.bounds]);
}
export function failedPlaceTool(active,error){
  if(storageErrors.has(error?.message)&&active.placeScope)active.placeScope.storageFailed=true;
}
// Persist only fixed error identifiers. Provider bodies, private paths and
// arbitrary error strings never become visible tool labels or saved history.
export function toolFailureCode(error){
  const message=error?.message;
  if(['Invalid Agent tool arguments.','Invalid GeoD tool arguments','Invalid dataset connector arguments.'].includes(message))return 'argument-schema';
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
