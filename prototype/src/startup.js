import { validateBounds } from './catalog.js';

export function selectedSearchBounds(input, appliedSearch = null) {
  if (appliedSearch?.bbox) return appliedSearch.bbox;
  try { return validateBounds(input.bbox); } catch { return null; }
}

function catalogSummary({catalog, liveState, filteredScenes, selectedIds = [], loadedIds = [], visibleLoadedIds = [], focusedItemId = null, activeDay = null, query = '', sort = 'date', projectId}) {
  if (!catalog) return null;
  const scenes = catalog.scenes || [], ids = new Set(scenes.map(scene => scene.id));
  const dates = new Map();
  for (const scene of scenes) {
    const day = scene.date?.slice(0, 10);
    if (day) dates.set(day, (dates.get(day) || 0) + 1);
  }
  const count = values => new Set(values.filter(id => ids.has(id))).size;
  return {
    state: liveState || (catalog.complete ? 'ready' : 'loading'),
    totalCount: scenes.length, filteredCount: (filteredScenes || scenes).length,
    selectedCount: count(selectedIds), loadedCount: count(loadedIds), visibleLoadedCount: count(visibleLoadedIds),
    selectedItemIds:[...new Set(selectedIds.filter(id=>ids.has(id)))].slice(0,32),selectionTruncated:count(selectedIds)>32,
    visibleItemIds:[...new Set(visibleLoadedIds.filter(id=>ids.has(id)))].slice(0,16),focusedItemId:ids.has(focusedItemId)?focusedItemId:null,
    complete: catalog.complete === true, pages: catalog.pages || 0,
    retrievedAt: catalog.retrievedAt || null, includesSavedProjectScenes: Boolean(projectId),
    activeDate: activeDay, textFilter: query.slice(0, 200), sort,
    dateCounts: [...dates].sort(([a], [b]) => b.localeCompare(a)).slice(0, 31).map(([date, count]) => ({date, count})),
    dateCountsTruncated: dates.size > 31,
  };
}

export function agentMapContext({ page, input, appliedSearch, areaPolygon, projectId, datasetId, datasetRequest, ...results }) {
  const bounds = selectedSearchBounds(input, datasetId ? null : appliedSearch);
  // The conversation homepage has no selected map. Native tools supply current
  // UTC search defaults and resolve the place described by the human.
  // Other workspace pages have their own native subject; an old Explore map is
  // not their implicit AOI. Only the actual project area may carry through.
  if (page !== 'Explore' || !bounds) return null;
  if (datasetId) return {page,provider:`dataset-${datasetId}`,bounds,start:'',end:'',cloudMin:0,cloudMax:100,
    projectId:projectId || null,geometry:areaPolygon?.geometry || null,catalog:null,
    ...(datasetRequest?.datasetId === datasetId && datasetRequest.bounds.every((value,index) => value === bounds[index]) ? {dataset:datasetRequest} : {})};
  const search = appliedSearch || input;
  return { page, provider:search.provider || 'earth-search', bounds, start:search.start, end:search.end,
    cloudMin:search.cloudMin ?? 0, cloudMax:search.cloud ?? 100, projectId:projectId || null, geometry:areaPolygon?.geometry || null,
    catalog:page === 'Explore' ? catalogSummary({...results, projectId}) : null };
}

export function agentScene({page,projectId,hash=''}) {
  if(page==='Home')return {kind:'conversation',page:'Home'};
  const params=new URLSearchParams(hash.split('?')[1]||'');
  const id=key=>/^[a-f0-9]{8}-[a-f0-9]{4}-[a-f0-9]{4}-[a-f0-9]{4}-[a-f0-9]{12}$/.test(params.get(key)||'')?params.get(key):null;
  const selected=page==='My Data'&&projectId?{kind:'project',id:projectId}:page==='Tasks'&&id('job')?{kind:'job',id:id('job')}:
    page==='Workspace'? [['vector','vector'],['tiles','tiles'],['map','map-image'],['file','raster'],['rgb','raster'],['project','project']].map(([key,kind])=>id(key)?{kind,id:id(key)}:null).find(Boolean):page==='Explore'&&projectId?{kind:'project',id:projectId}:null;
  return {kind:'workspace',page,...(selected?{subject:selected}:{})};
}

export function gscloudMapContext({page,entry,context,bounds,geometry,projectId}) {
  const request=context?.request;
  if(page!=='Explore'||entry?.kind!=='gscloud'||!request||!bounds||request.bounds.join(',')!==bounds.join(','))return null;
  return {page,provider:'gscloud',bounds,start:request.start,end:request.end,cloudMin:0,cloudMax:request.cloudMax??100,
    projectId:projectId||null,geometry:geometry||null,catalog:context.catalog,gscloud:request};
}
