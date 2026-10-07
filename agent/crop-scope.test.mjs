import test from 'node:test';
import assert from 'node:assert/strict';
import { randomUUID } from 'node:crypto';
import { beforeCropTool,afterBoundaryTool,CROP_CHOICE_ERROR,CROP_BOUNDARY_ERROR,CROP_GOAL_ERROR } from './crop-scope.mjs';
const active=text=>({origin:'human',context:null,session:{executionMode:'confirm-each',entries:[{type:'user',text}]}});
const fallback=()=>({version:1,id:randomUUID(),title:'纽约市裁剪范围',status:'pending',questions:[{id:'crop_area',prompt:'系统当前只有外接矩形，没有经核验的行政区多边形。请选择裁剪方式：',options:[
  {id:'rectangle',label:'按外接矩形裁剪',description:'西 -74.258843，南 40.476578，东 -73.700233，北 40.91763。'},
  {id:'boundary',label:'提供/选择多边形边界',description:'提供真实行政区边界。'},
]}]});
const source=()=>({boundary:{id:randomUUID(),sha256:'a'.repeat(64)},bounds:[-74.258843,40.476578,-73.700169,40.917705],geometryType:'MultiPolygon'});
test('only the same verified source geometry supersedes an obsolete missing-polygon fallback, preserving actual choices and confirmation',()=>{
  const scope=active('要按纽约市区范围裁剪出来'),decision=fallback(),value=source();
  scope.session.entries.push({name:'geod_request_decision',decision},{type:'user',text:'再试试'});
  assert.equal(afterBoundaryTool(scope,'geod_boundary_read',{...value,bounds:[0,0,1,1]}),undefined);assert.equal(decision.status,'pending');
  const result=afterBoundaryTool(scope,'geod_boundary_read',value);
  assert.equal(decision.status,'superseded');assert.equal(decision.answers,undefined);assert.deepEqual(result.resolvedDecisions,[decision.id]);
  assert.equal(scope.session.executionMode,'confirm-each');assert.doesNotThrow(()=>beforeCropTool(scope,'geod_project_plan',{boundary:value.boundary}));
  assert.throws(()=>beforeCropTool(scope,'geod_project_plan',{}),{message:CROP_CHOICE_ERROR});
  for(const mutate of [d=>{d.status='answered';d.answers=[{questionId:'crop_area',optionId:'rectangle'}];},d=>d.questions.push(d.questions[0]),d=>d.questions[0].prompt='选择更适合你的裁剪精度。',d=>d.questions[0].options[0].description='没有可核验的范围',d=>d.boundaryScope={bounds:[0,0,1,1]}]){
    const other=active('要按纽约市区范围裁剪出来'),d=fallback();mutate(d);other.session.entries.push({name:'geod_request_decision',decision:d});
    afterBoundaryTool(other,'geod_boundary_read',source());assert.notEqual(d.status,'superseded');
  }
  for(const text of ['改成按巴黎行政区裁剪','改成按外接矩形裁剪']){
    const other=active('要按纽约市区范围裁剪出来'),d=fallback();other.session.entries.push({name:'geod_request_decision',decision:d},{type:'user',text});
    afterBoundaryTool(other,'geod_boundary_read',source());assert.equal(d.status,'pending');
  }
});
test('a verified native source polygon unlocks administrative crops without invented geometry or redundant upload questions',()=>{
  for(const text of ['按纽约市行政区裁剪','Crop Paris imagery to its administrative boundary','按东京范围裁剪哨兵影像']) {
    const scope=active(text);const reference={id:'source-reference',sha256:'a'.repeat(64)};
    assert.throws(()=>beforeCropTool(scope,'geod_project_plan',{boundary:reference}),{message:CROP_CHOICE_ERROR});
    afterBoundaryTool(scope,'geod_boundary_read',{boundary:reference});
    assert.doesNotThrow(()=>beforeCropTool(scope,'geod_project_plan',{boundary:reference}));
    assert.throws(()=>beforeCropTool(scope,'geod_project_plan',{boundary:{...reference,sha256:'b'.repeat(64)}}),{message:CROP_CHOICE_ERROR});
    scope.session.goal={requestText:'下载原始影像'};
    assert.throws(()=>beforeCropTool(scope,'geod_project_plan',{boundary:reference}),{message:CROP_GOAL_ERROR});
  }
  const scope=active('Crop to a rectangle');const reference={id:'source',sha256:'a'.repeat(64)};
  afterBoundaryTool(scope,'geod_boundary_read',{boundary:reference});
  assert.throws(()=>beforeCropTool(scope,'geod_project_plan',{boundary:reference}),{message:CROP_CHOICE_ERROR});
});
test('unresolved crop extents cannot become rectangular projects for different places or sensors',()=>{
  for(const text of ['按纽约市区范围裁剪出来','Crop Sentinel imagery to Paris','Clip the Landsat imagery to Nairobi','按县域掩膜']){
    const scope=active(text);assert.throws(()=>beforeCropTool(scope,'geod_project_plan',{}),{message:CROP_CHOICE_ERROR});
    assert.throws(()=>beforeCropTool(scope,'geod_project_mosaic_plan',{}),{message:CROP_CHOICE_ERROR});
    scope.session.entries.push({type:'user',text:'重试'});
    assert.throws(()=>beforeCropTool(scope,'geod_project_plan',{}),{message:CROP_CHOICE_ERROR});
    assert.doesNotThrow(()=>beforeCropTool(scope,'geod_scene_search',{}));
  }
  for(const text of ['下载最新哨兵影像','不用裁剪，下载原图','Download imagery without cropping','按外接矩形裁剪','Crop to this bbox'])assert.doesNotThrow(()=>beforeCropTool(active(text),'geod_project_plan',{}));
});
test('actual crop choices and geometry are honored without becoming execution approval',()=>{
  const scope=active('按市区范围裁剪');
  scope.context={geometry:{type:'Polygon'}};
  assert.throws(()=>beforeCropTool(scope,'geod_project_plan',{}),{message:CROP_BOUNDARY_ERROR});
  assert.doesNotThrow(()=>beforeCropTool(scope,'geod_project_plan',{useAttachedPolygon:true}));
  scope.context=null;const decision={id:'native-choice',status:'answered',questions:[],answers:[{questionId:'crop_area',optionId:'rectangle'}]};
  decision.questions=[{id:'crop_area',prompt:'范围精度？',options:[{id:'rectangle',label:'矩形'},{id:'boundary',label:'行政区多边形'}]}];
  scope.session.entries.push({name:'geod_request_decision',decision});
  assert.doesNotThrow(()=>beforeCropTool(scope,'geod_project_plan',{}));
  scope.session.goal={requestText:'下载最新影像'};
  assert.throws(()=>beforeCropTool(scope,'geod_project_plan',{}),{message:CROP_GOAL_ERROR});
  scope.session.goal.requestText='按市区范围裁剪\nHuman selected preferences: 外接矩形';
  assert.doesNotThrow(()=>beforeCropTool(scope,'geod_project_plan',{}));
  delete scope.session.goal;
  decision.questions[0].id='crop_geometry';decision.answers[0].questionId='crop_geometry';decision.questions[0].prompt='选择裁剪范围';
  decision.questions[0].options[0].id='outer_rect';decision.questions[0].options[1].id='exact_polygon';decision.answers[0].optionId='outer_rect';
  assert.doesNotThrow(()=>beforeCropTool(scope,'geod_project_plan',{}));
  decision.answers[0].optionId='exact_polygon';assert.throws(()=>beforeCropTool(scope,'geod_project_plan',{}),{message:CROP_BOUNDARY_ERROR});
  decision.questions[0].id='crop_area';decision.answers[0].questionId='crop_area';decision.questions[0].options[0].id='rectangle';decision.questions[0].options[1].id='boundary';
  decision.answers[0].optionId='boundary';assert.throws(()=>beforeCropTool(scope,'geod_project_plan',{}),{message:CROP_BOUNDARY_ERROR});
  scope.session.entries.push({type:'user',text:'改成按另一个市区裁剪'});
  assert.throws(()=>beforeCropTool(scope,'geod_project_plan',{}),{message:CROP_CHOICE_ERROR});
  scope.session.executionMode='full-access';assert.doesNotThrow(()=>beforeCropTool(scope,'geod_project_plan',{}));
});
