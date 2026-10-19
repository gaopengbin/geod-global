// Discovery changes model context only. All execution still passes the same
// pinned native definitions, conversation scope and permission checks.
const groups = {
  imagery: ['scene_search','scene_search_more','scene_coverage','catalog_selection','download_plan','project_plan','project_download_plan','project_mosaic_plan','clip_plan','raster_inspect','raster_pixel','rgb_inspect','rgb_pixel'],
  gscloud: ['gscloud_products','gscloud_search','gscloud_detail','gscloud_download_plan','gscloud_archive_inspect'],
  places: ['region_search','region_levels','place_search','boundary_read'],
  projects: ['projects_list','project_get','jobs_list','job_status','job_control','health'],
  science: ['recipes_list','recipe_plan','recipe_review_plan','scientific_rgb_plan'],
  stac: ['dataset_catalog','dataset_resolve','dataset_connect','stac_connections','stac_catalog','stac_snapshot','stac_assets','stac_search','stac_project_plan','stac_download_plan','stac_inspect','stac_pixel'],
  wcs: ['wcs_connections','wcs_coverages','wcs_description','wcs_plan','wcs_describe','wcs_prepare','wcs_project_plan','wcs_download_plan','wcs_inspect','wcs_pixel'],
  vectors: ['feature_services','feature_collections','vectors_list','vector_inspect','vector_features','vector_node','vector_extract_plan'],
  monitoring: ['official_services','monitor_list','monitor_notifications','monitor_quote'],
};
export const TOOL_GROUPS = Object.freeze(Object.keys(groups));
const core = new Set(['tools_load','workspace_context','sources_list','execution_policy','request_decision','goal_define','goal_bind','goal_check','plan_status','plan_execute','workspace_open'].map(n=>`geod_${n}`));
const membership = new Map(Object.entries(groups).flatMap(([group,names])=>names.map(n=>[`geod_${n}`,group])));
export const TOOL_LOADER = Object.freeze({name:'geod_tools_load',description:'Load only groups needed for the current step, together in one call. gscloud: 地理空间数据云 product/search/detail and confirmed original ZIP download/inspection, separate from STAC; imagery: public satellite search, coverage, download, SCL classification and ordinary crop/mosaic; places: administrative lookup and polygons; projects: saved projects, jobs and recovery; science: ONLY derived scientific RGB/recipes, NOT SCL download or crop; stac: custom STAC; wcs: service subsets; vectors: vector inspection/extraction; monitoring: Pro imagery monitoring. Do not preload unrelated groups. Discovery never executes or approves work.',inputSchema:{type:'object',properties:{groups:{type:'array',minItems:1,maxItems:8,uniqueItems:true,items:{type:'string',enum:TOOL_GROUPS}}},required:['groups'],additionalProperties:false}});
export function loadToolGroups(input, definitions) {
  if(!input || Object.keys(input).some(k=>k!=='groups') || !Array.isArray(input.groups) || !input.groups.length || input.groups.length>8 || input.groups.some(g=>!TOOL_GROUPS.includes(g)))throw Error('Choose available native tool groups.');
  return {loadedGroups:[...new Set(input.groups)],tools:definitions.filter(d=>input.groups.includes(membership.get(d.name))).map(d=>d.name),note:'These pinned tools are available on the next model request. Discovery is not authorization.'};
}
export const WORKFLOW_TOOLS = ['project_get','job_status','project_download_plan','project_mosaic_plan','clip_plan','raster_inspect','rgb_inspect','gscloud_archive_inspect'].map(n=>`geod_${n}`);
// Adapted from GeoD Agent 0.2.5 model-input-context.mjs (693d6a5).
// Selection changes prompt size only; registered native permissions stay intact.
const patterns={
  imagery:/影像|卫星|遥感|哨兵|landsat|sentinel|imagery|satellite|raster|下载.*(?:高程|地形)|(?:高程|地形).*下载/i,
  gscloud:/地理空间数据云|数据云|gscloud|gdem|srtmslope|srtmtpi/i,
  places:/行政区|边界|多边形|地点|地名|城市|boundary|boundaries|polygon|\bplace\b|\bcity\b/i,
  projects:/工程|项目|任务|下载进度|失败|重试|project|\bjobs?\b|\btasks?\b|retry/i,
  science:/配方|科学.*(?:合成|波段)|波段.*合成|recipe|scientific.?rgb/i,
  stac:/\bstac\b|自定义.*(?:栅格|数据源)|专题.*(?:产品|数据)|dataset|worldpop|worldcover|chirps|jaxa|人口|降水|土壤水分|地表温度/i,
  wcs:/\bwcs\b|覆盖服务|coverage.?service/i,
  vectors:/矢量|要素|geojson|shapefile|\bwfs\b|\bvector\b/i,
  monitoring:/监测|monitor/i,
};
const inputText=item=>typeof item?.content==='string'?item.content:(item?.content??[]).map(part=>part?.text??'').join('\n');
export const greeting=text=>/^(?:你好|您好|嗨|哈喽|谢谢|多谢|hello|hi|hey|thanks|thank you)(?:[，,、\s]+(?:请简短回复|不调用工具|简短回复|reply briefly|no tools))*[!！。.?？]*$/iu.test(text.trim());
export function visibleTools(input, declared, preferred=null, task=null) {
  // Old native threads do not have the loader registered. Keep their full
  // original tool contract rather than making capabilities unreachable.
  if(!declared.has(TOOL_LOADER.name))return declared;
  const enabled=new Set(task?.loadedGroups??[]), calls=new Map(), retained=new Set([...(preferred??[]),...(task?.calledTools??[])]);
  if(task?.source==='gscloud')enabled.add('gscloud');
  // Continuation enters a known native execution phase. Earlier lookup groups
  // need not be resent. Explicit discovery during this turn still expands them.
  const start=Math.max(0,input.findLastIndex(i=>i.role==='user'));
  const text=task?.text??inputText(input[start]);
  const currentCalls=input.slice(start).filter(item=>item.type==='function_call');
  const closed=new Set(input.filter(item=>item.type==='function_call_output').map(item=>item.call_id));
  const pendingCalls=input.filter(item=>item.type==='function_call'&&!closed.has(item.call_id)&&declared.has(item.name));
  if(!preferred&&!task?.hasAttachments&&!task?.compacting&&!retained.size&&!currentCalls.length&&!pendingCalls.length&&greeting(text))return new Set();
  if(!preferred){
    let intent=text;
    if(/^(?:继续|确认|好[的吧]?|改成|改为|保持|其余|continue|resume|confirm|change\b)/i.test(text.trim())){
      intent+='\n'+(task?.previousText??inputText(input.slice(0,start).findLast(item=>item.role==='user'&&!greeting(inputText(item)))));
    }
    for(const [group,pattern]of Object.entries(patterns))if(pattern.test(intent))enabled.add(group);
  }
  for(const item of input.slice(start)) {
    if(item.type==='function_call' && item.name===TOOL_LOADER.name)calls.set(item.call_id,true);
    if(item.type==='function_call_output' && calls.has(item.call_id)) {
      try {const value=typeof item.output==='string'?JSON.parse(item.output):item.output;
        for(const group of value?.loadedGroups??[])if(TOOL_GROUPS.includes(group))enabled.add(group);
      }catch{/* Failed discovery never activates tools. */}
    }
    // Only the current request activates schemas. Closed calls from an older
    // human task are still valid history, not an instruction to resend schemas.
    if(item.type==='function_call' && declared.has(item.name))retained.add(item.name);
  }
  for(const item of pendingCalls)retained.add(item.name);
  return new Set([...declared].filter(n=>core.has(n)||retained.has(n)||!membership.has(n)||enabled.has(membership.get(n))));
}
