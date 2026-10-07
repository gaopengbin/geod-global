import test from 'node:test';
import assert from 'node:assert/strict';
import {afterAcquisitionTool,prepareAcquisitionTool,ADMIN_BOUNDARY_ERROR,AREA_PROJECT_ERROR} from './acquisition-scope.mjs';
const bounds=[115.4,39.4,117.5,41.1],reference={id:'a1234567-1234-1234-1234-123456789abc',sha256:'a'.repeat(64)};
function active(text='下载北京影像，云量小于5%',mode='confirm-each') {return {session:{executionMode:mode,entries:[{type:'user',text}]},context:null};}
function search(a){afterAcquisitionTool(a,'geod_region_search',{candidates:[{bounds,boundarySource:{provider:'natural-earth',candidateId:'native-beijing',countryCode:'CHN',adminLevel:1}}]});afterAcquisitionTool(a,'geod_scene_search',{searchId:'search',query:{bounds}});}
test('administrative downloads default to the native polygon with no crop keyword, in both approval modes',()=>{
  for(const mode of ['confirm-each','full-access']){
    const a=active(undefined,mode);search(a);
    assert.throws(()=>prepareAcquisitionTool(a,'geod_project_plan',{searchId:'search'}),e=>e.message.startsWith(ADMIN_BOUNDARY_ERROR));
    afterAcquisitionTool(a,'geod_boundary_read',{bounds,boundary:reference});
    assert.deepEqual(prepareAcquisitionTool(a,'geod_scene_coverage',{searchId:'search'}),{searchId:'search',boundary:reference,useAttachedPolygon:false});
    assert.deepEqual(prepareAcquisitionTool(a,'geod_project_plan',{searchId:'search'}),{searchId:'search',boundary:reference,useAttachedPolygon:false});
    assert.throws(()=>prepareAcquisitionTool(a,'geod_download_plan',{searchId:'search'}),{message:AREA_PROJECT_ERROR});
  }
});
test('explicit rectangles, originals and answered rectangle choices override the default, unrelated geometry does not',()=>{
  for(const text of ['按北京外接矩形下载','北京只下载原图，不裁剪']){const a=active(text);search(a);assert.deepEqual(prepareAcquisitionTool(a,'geod_download_plan',{searchId:'search'}),{searchId:'search'});}
  const a=active();search(a);afterAcquisitionTool(a,'geod_boundary_read',{bounds:[0,0,1,1],boundary:reference});
  assert.throws(()=>prepareAcquisitionTool(a,'geod_project_plan',{searchId:'search'}));
  assert.deepEqual(prepareAcquisitionTool(a,'geod_project_plan',{searchId:'search',projectId:'existing'}),{searchId:'search',projectId:'existing'});
});
test('continuation retains selected area and an attached polygon takes the same final-area workflow',()=>{
  const a=active();a.context={bounds,geometry:{type:'Polygon',coordinates:[]}};
  afterAcquisitionTool(a,'geod_scene_search_more',{searchId:'later',query:{bounds}});
  assert.deepEqual(prepareAcquisitionTool(a,'geod_project_plan',{searchId:'later'}),{searchId:'later',useAttachedPolygon:true});
  assert.throws(()=>prepareAcquisitionTool(a,'geod_download_plan',{searchId:'later'}),{message:AREA_PROJECT_ERROR});
});
