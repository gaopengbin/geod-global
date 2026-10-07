// Only stable, read-only lookups within one bounded human turn. Permission,
// authorization, live jobs, plans and every mutation always reach native code.
const READS=new Set(['geod_workspace_context','geod_place_search','geod_region_search','geod_region_levels','geod_scene_search','geod_boundary_read','geod_scene_coverage','geod_scene_search_more']);
export function turnReadKey(name,args) {
  if(!READS.has(name))return null;
  const text=JSON.stringify(args??{},(_key,value)=>value&&typeof value==='object'&&!Array.isArray(value)
    ?Object.fromEntries(Object.keys(value).sort().map(key=>[key,value[key]])):value);
  return text.length<=8000?`${name}:${text}`:null;
}
