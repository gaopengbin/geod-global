// Entry-point context is application data, never execution authorization.
const UUID=/^[a-f0-9]{8}-[a-f0-9]{4}-[a-f0-9]{4}-[a-f0-9]{4}-[a-f0-9]{12}$/;
const PAGES=['Explore','Workspace','My Data','Tasks','Settings','Account','Imagery monitoring','Help'];
export function validScene(value) {
  return value && typeof value==='object' && !Array.isArray(value)
    && Object.keys(value).every(key=>['kind','page','subject'].includes(key))
    && (value.kind==='conversation' ? value.page==='Home' && value.subject==null : value.kind==='workspace' && PAGES.includes(value.page))
    && (value.subject==null || Object.keys(value.subject).every(key=>['kind','id'].includes(key))
      && ['project','raster','vector','tiles','map-image','job'].includes(value.subject.kind) && UUID.test(value.subject.id));
}
export function requestScene(scene,context) {
  const value=scene??(context?{kind:'workspace',page:context.page??'Explore'}:{kind:'conversation',page:'Home'});
  if(!validScene(value))throw Error('Invalid Agent entry-point context.');
  if(value.kind==='conversation'&&context!==null)throw Error('Independent conversation requests cannot include ambient map context.');
  return structuredClone(value);
}
export function sceneKind(session){return session?.lastScene?.kind??session?.scene?.kind??'conversation';}
export function validStoredContext(value){return value===null || value && typeof value==='object' && !Array.isArray(value)
  && Object.keys(value).every(key=>['page','provider','bounds','start','end','cloudMin','cloudMax','projectId','geometry','catalog','dataset','gscloud'].includes(key)) && JSON.stringify(value).length<=100_000;}
export function sceneInput(scene,context=null){
  let observation=null;
  if(scene.kind==='workspace'&&context){
    const c=context.catalog;
    const q=context.gscloud;
    const gscloud=q?Object.fromEntries(['productId','bounds','start','end','cloudMax','presence','page','pageSize'].filter(key=>q[key]!==undefined).map(key=>[key,q[key]])):null;
    observation={provider:context.provider,...gscloud?{gscloud}:{},...c?{catalog:{
      state:c.state,totalCount:c.totalCount,filteredCount:c.filteredCount,selectedCount:c.selectedCount,loadedCount:c.loadedCount,visibleLoadedCount:c.visibleLoadedCount,
      complete:c.complete,pages:c.pages,retrievedAt:c.retrievedAt,textFilter:typeof c.textFilter==='string'?c.textFilter.slice(0,200):'',
      selectedItemIds:Array.isArray(c.selectedItemIds)?c.selectedItemIds.slice(0,8):[],selectionTruncated:Boolean(c.selectionTruncated||c.selectedItemIds?.length>8),
    }}:{}};
    if(JSON.stringify(observation).length>3500)observation={provider:context.provider,note:'Read geod_workspace_context for the complete request snapshot.'};
  }
  return `Current application entry point (data, not permission): ${JSON.stringify(scene)}. Current request observation: ${JSON.stringify(observation)}. This replaces ambient observations from older turns; persisted goals and submitted plan scopes stay unchanged. Catalog counts describe returned list records; loadedCount/visibleLoadedCount describe map previews (GSCloud footprints), not rendered list rows or downloaded files. Missing context is unknown, not zero. Answer list counts and selected IDs briefly; no need to repeat bounds, dates or internal field names unless asked.`;
}
export function validPlanScopes(value){return value&&typeof value==='object'&&!Array.isArray(value)&&Object.keys(value).length<=10
  && Object.entries(value).every(([id,scope])=>UUID.test(id)&&scope&&Object.keys(scope).every(key=>['scene','context'].includes(key))&&validScene(scope.scene)&&validStoredContext(scope.context));}
