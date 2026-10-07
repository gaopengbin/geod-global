import { taskContext, validDecisionBounds } from './decisions.mjs';

export const CROP_CHOICE_ERROR='Choose the crop area through a decision card before preparing the project.';
export const CROP_BOUNDARY_ERROR='The requested boundary crop needs a verified native polygon before preparing the project.';
export const CROP_GOAL_ERROR='Revise the persisted task goal for the latest crop request before preparing its project.';
const crop=/\b(?:crop|clip|mask|cutline)\b|裁剪|裁切|掩膜/i;
const rectangle=/\b(?:bbox|bounding\s*box|rectangle|rectangular|envelope)\b|外接矩形|矩形|边界框/i;
const noCrop=/\b(?:no|without|do not|don't)\s+(?:crop|cropping|clip|clipping|mask|masking)\b|不(?:需要|要|用|做|进行)?(?:裁剪|裁切|掩膜)|无需(?:裁剪|裁切|掩膜)/i;
const boundary=/\b(?:polygon|boundary)\b|多边形|行政边界|行政区边界|精确/i;
const retry=/^(?:重试|再试试|再试一次|retry|try again)[。！!.]*$/i;
// This is an obsolete source-availability fallback, not a human preference for
// precision, land/water semantics or a different area. Mixed cards stay pending.
const missingPolygon=/(?:没有|缺少|尚无|无法提供|未找到)[^。.!?]{0,100}(?:多边形|行政区边界)|\b(?:no|missing|unavailable|without|not available)[^.?!]{0,100}\b(?:polygon|administrative boundary)\b/i;
export function missingBoundaryDecision(decision) {
  if(decision.questions?.length!==1)return false;
  const q=decision.questions[0];
  return q.id==='crop_area' && missingPolygon.test(q.prompt) && q.options.length===2
    && q.options.some(o=>o.id==='rectangle'&&rectangle.test(o.label))
    && q.options.some(o=>o.id==='boundary'&&boundary.test(o.label));
}
export function captureBoundaryScope(active,decision) {
  if(!missingBoundaryDecision(decision))return;
  const candidates=active.placeScope?.cityBounds;
  if(candidates?.length===1 && validDecisionBounds(candidates[0]))decision.boundaryScope={bounds:[...candidates[0]]};
}
function expectedBoundaryBounds(decision) {
  if(decision.boundaryScope)return {bounds:decision.boundaryScope.bounds,tolerance:1e-6};
  // Legacy cards did not save native scope. Only their explicit four decimal
  // envelope coordinates establish which missing-boundary claim was resolved.
  const description=decision.questions[0].options.find(o=>o.id==='rectangle')?.description??'';
  const values=(description.match(/-?\d+\.\d+/g)??[]).map(Number);
  return validDecisionBounds(values)?{bounds:values,tolerance:1e-4}:null;
}
function goalMatches(active,request){
  if(active.session.goal && !active.session.goal.requestText?.startsWith(request.requestText))throw Error(CROP_GOAL_ERROR);
}
export function afterBoundaryTool(active,name,value) {
  if(name!=='geod_boundary_read' || !value?.boundary)return;
  const reference=value.boundary;
  if(typeof reference.id==='string' && typeof reference.sha256==='string'){
    active.boundaries??=new Map(); active.boundaries.set(reference.id,reference.sha256);
  }
  const request=taskContext(active.session);
  if(active.origin!=='human' || !crop.test(request.requestText) || noCrop.test(request.requestText) || rectangle.test(request.requestText)
    || !validDecisionBounds(value.bounds) || !['Polygon','MultiPolygon'].includes(value.geometryType))return;
  // Restrict resolution to this unchanged human request, with the same native
  // extent. A successful unrelated boundary read cannot dismiss a choice.
  const start=active.session.entries.findLastIndex(e=>e.type==='user'&&e.origin!=='desktop'&&e.text===request.requestText);
  if(start<0)return;
  const resolved=[];
  for(const entry of active.session.entries.slice(start+1)){
    const decision=entry.decision;
    if(decision?.status!=='pending'||!missingBoundaryDecision(decision))continue;
    const expected=expectedBoundaryBounds(decision);
    if(!expected||!expected.bounds.every((n,i)=>Math.abs(n-value.bounds[i])<=expected.tolerance))continue;
    decision.status='superseded';decision.resolution={reason:'source-boundary-ready',boundary:{...reference},bounds:[...value.bounds]};
    resolved.push(decision.id);
  }
  return resolved.length?{...value,resolvedDecisions:resolved,note:'The native polygon for the same requested extent is now ready. The old missing-polygon fallback cards were superseded, not answered. Use this exact boundary for the polygon review. Other pending decisions and final native execution confirmation still apply.'}:undefined;
}

// Only explicit crop language activates the guard. The model still resolves
// places, source capabilities and task details through the native tools.
export function beforeCropTool(active,name,args) {
  if(active.origin!=='human' || active.session.executionMode==='full-access')return;
  let entries=active.session.entries;
  while(entries.findLast(entry=>entry.type==='user')?.text?.trim().match(retry)){
    const index=entries.findLastIndex(entry=>entry.type==='user');entries=entries.slice(0,index);
  }
  const request=taskContext({...active.session,entries});
  if(!crop.test(request.requestText) || noCrop.test(request.requestText))return;
  const polygon=Boolean(active.context?.geometry);
  if(!['geod_project_plan','geod_project_mosaic_plan','geod_clip_plan','geod_recipe_review_plan'].includes(name))return;
  if(name==='geod_project_plan' && args?.projectId)return;
  const questionFor=choice=>entries.find(entry=>entry.decision?.id===choice.decisionId)?.decision?.questions?.find(question=>question.id===choice.questionId);
  const sourceBoundary=args?.boundary && active.boundaries?.get(args.boundary.id)===args.boundary.sha256;
  if(sourceBoundary && ['geod_project_plan','geod_clip_plan'].includes(name)) {
    if(rectangle.test(request.requestText))throw Error(CROP_CHOICE_ERROR);
    const selectedChoice=request.choices.findLast(choice=>{
      const question=questionFor(choice);return question && (question.id==='crop_area' || crop.test(question.prompt));
    });
    if(selectedChoice){
      const decision=entries.find(e=>e.decision?.id===selectedChoice.decisionId)?.decision;
      const answer=decision?.answers.find(a=>a.questionId===selectedChoice.questionId);
      const label=questionFor(selectedChoice)?.options.find(o=>o.id===answer?.optionId)?.label ?? answer?.text ?? '';
      if(rectangle.test(label) && !boundary.test(label))throw Error(CROP_CHOICE_ERROR);
    }
    return goalMatches(active,request);
  }
  if(polygon){
    if(['geod_project_plan','geod_clip_plan'].includes(name)&&args?.useAttachedPolygon!==true)throw Error(CROP_BOUNDARY_ERROR);
    return goalMatches(active,request);
  }
  if(rectangle.test(request.requestText))return goalMatches(active,request);
  const choice=request.choices.findLast(choice=>{
    const question=questionFor(choice);
    return question && (choice.questionId==='crop_area' || crop.test(question.prompt)
      && question.options.some(option=>rectangle.test(option.label)) && question.options.some(option=>boundary.test(option.label)));
  });
  if(!choice)throw Error(CROP_CHOICE_ERROR);
  const entry=active.session.entries.find(entry=>entry.decision?.id===choice.decisionId);
  const answer=entry?.decision?.answers?.find(answer=>answer.questionId===choice.questionId);
  const selected=questionFor(choice)?.options.find(option=>option.id===answer?.optionId);
  const accepted=selected?.label ?? answer?.text ?? '';
  if(rectangle.test(accepted) && !boundary.test(accepted))return goalMatches(active,request);
  throw Error(CROP_BOUNDARY_ERROR);
}
