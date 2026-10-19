export function sceneSuggestions(scene, datasetId) {
  if(scene?.page==='Explore' && datasetId)return ['Explain the selected data product and its supported options.','Check published files for the current product, area and form.','Prepare a download plan for the selected product files, without cropping.'];
  if(scene?.page==='Explore')return ['How many imagery scenes are in the current list?','Prepare a download plan for selected originals only, without cropping.','Find imagery covering the current area.'];
  if(scene?.page==='My Data')return [scene.subject?'Inspect the selected project and its actual results.':'Show my saved projects.'];
  if(scene?.page==='Workspace')return [scene.subject?'Inspect the selected file and its spatial resolution.':'Show my saved projects.'];
  if(scene?.page==='Tasks')return [scene.subject?'Check the selected task and explain its actual status.':'Which tasks need attention?'];
  if(scene?.page==='Imagery monitoring')return ['Show my monitoring regions.'];
  return ['Show my saved projects.','Which tasks need attention?'];
}
