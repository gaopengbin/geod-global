// Administrative acquisition defaults are based only on native candidates.
// A confirmation setting changes approval, never the requested geometry.
import {taskContext,validDecisionBounds} from './decisions.mjs';
const same=(a,b)=>validDecisionBounds(a)&&validDecisionBounds(b)&&a.every((v,i)=>Math.abs(v-b[i])<=1e-6);
const rectangle=/\b(?:bbox|bounding\s*box|rectangle|rectangular|envelope)\b|外接矩形|矩形|边界框/i;
const originals=/\b(?:without cropping|no crop|originals? only|full source tiles)\b|不(?:需要|要|用|做|进行)?(?:裁剪|裁切|掩膜)|只(?:下载|要)(?:原图|原始|完整瓦片)/i;
export const ADMIN_BOUNDARY_ERROR='Read the selected administrative boundary before preparing an area task. Use geod_boundary_read with its actual boundarySource; polygon scope is the default.';
export const AREA_PROJECT_ERROR='Administrative or attached polygon deliveries use geod_project_plan with the verified boundary, then project download and polygon mosaic. A standalone original download cannot deliver the requested area.';
export function afterAcquisitionTool(active,name,value){
  const state=active.acquisition??={candidates:[],boundaries:[],searches:new Map()};
  if(['geod_region_search','geod_place_search'].includes(name)){
    for(const c of value?.candidates??[])if(validDecisionBounds(c.bounds)&&c.boundarySource){
      state.candidates=state.candidates.filter(old=>!same(old.bounds,c.bounds));
      state.candidates.push({bounds:[...c.bounds],source:structuredClone(c.boundarySource)});
    }
  }
  if(name==='geod_boundary_read'&&value?.boundary&&validDecisionBounds(value.bounds)){
    state.boundaries=state.boundaries.filter(old=>!same(old.bounds,value.bounds));
    state.boundaries.push({bounds:[...value.bounds],reference:structuredClone(value.boundary)});
  }
  if(['geod_scene_search','geod_scene_search_more'].includes(name)&&value?.searchId&&validDecisionBounds(value.query?.bounds))state.searches.set(value.searchId,[...value.query.bounds]);
}
export function prepareAcquisitionTool(active,name,args){
  if(!['geod_scene_coverage','geod_project_plan','geod_download_plan'].includes(name)||args?.projectId)return args;
  const request=taskContext(active.session),text=request.requestText??'';
  const choice=request.choices.findLast(c=>c.questionId==='crop_area'||/裁剪范围|crop area/i.test(c.prompt));
  if(rectangle.test(text)||originals.test(text)||choice&&rectangle.test(choice.answer)&&!/(?:polygon|多边形)/i.test(choice.answer))return args;
  const state=active.acquisition,bounds=state?.searches.get(args?.searchId);
  if(!bounds)return args;
  const source=state.candidates.find(c=>same(c.bounds,bounds));
  const attached=active.context?.geometry&&same(active.context.bounds,bounds);
  if(!source&&!attached)return args;
  if(name==='geod_download_plan')throw Error(AREA_PROJECT_ERROR);
  const resolved=state.boundaries.find(c=>same(c.bounds,bounds));
  if(source&&!resolved)throw Error(ADMIN_BOUNDARY_ERROR+' '+JSON.stringify(source.source));
  if(resolved){
    if(args.boundary&&(args.boundary.id!==resolved.reference.id||args.boundary.sha256!==resolved.reference.sha256))throw Error(ADMIN_BOUNDARY_ERROR);
    return {...args,boundary:resolved.reference,useAttachedPolygon:false};
  }
  return {...args,useAttachedPolygon:true};
}
