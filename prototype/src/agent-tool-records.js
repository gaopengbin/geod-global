// Only returned tool receipts and native review data populate these observations.
export function toolObservations(entry, plans, {t, number}) {
  const record=entry.record, result=record?.result ?? {}, input=record?.input ?? {}, summary=entry.summary;
  const observations=[];
  const add=(label,value)=>{if(value!==undefined&&value!==null&&value!=='')observations.push({label:t(label),value:String(value)});};
  const names=values=>values.filter(Boolean).join(' · ');
  const bounds=value=>Array.isArray(value)&&value.length===4&&value.every(Number.isFinite)?value.map(v=>number(v,{maximumFractionDigits:5})).join(', '):undefined;
  let outcome='';
  const catalog=result.context?.catalog;
  if(entry.name==='geod_workspace_context' && catalog && Number.isSafeInteger(catalog.totalCount)) {
    outcome=t('Current imagery list · {count} scenes',{count:number(catalog.filteredCount)});
    add('Catalog results',number(catalog.totalCount));
    add('Filtered scenes',number(catalog.filteredCount));
    add('Selected scenes',number(catalog.selectedCount));
    add('Loaded imagery',number(catalog.loadedCount));
    add('Catalog state',t(catalog.complete&&catalog.state==='ready'?'Catalog search complete':'Catalog results are partial'));
  }
  if (summary?.kind==='places') outcome=t('{count} place matches',{count:number(summary.total ?? summary.count)});
  if (summary?.kind==='levels') outcome=t('{count} administrative levels',{count:number(summary.count)});
  if (summary?.kind==='projects') outcome=t('{count} projects read',{count:number(summary.count)});
  if (summary?.kind==='jobs') outcome=t('{count} tasks read',{count:number(summary.count)});
  if (summary?.kind==='job') outcome=t(summary.status==='succeeded'&&summary.settled?'Output ready':'Task is not complete');
  if (summary?.kind==='search') outcome=t('{count} scenes found',{count:number(summary.count)});
  if (summary?.kind==='sources') outcome=t('Supports {count} sources',{count:number(summary.count)});
  if (summary?.kind==='vectors') outcome=t('{count} vector files read',{count:number(summary.count)});
  if (summary?.kind==='vector') outcome=t('{count} features verified',{count:number(summary.count)});
  if (summary?.kind==='file') outcome=t('File checksum read');
  if (summary?.kind==='plan') outcome=t(({pending:'Review awaiting confirmation',submitted:'Review submitted',expired:'Review expired',superseded:'Review replaced',unavailable:'Review unavailable'})[summary.status]??'Review unavailable');
  if(result.candidates?.length){
    add('Matches',names(result.candidates.map(c=>names([c.name,c.countryCode,c.adminLevel===undefined?undefined:`ADM${c.adminLevel}`]))));
    outcome=names([outcome,result.candidates[0].name]);
  }
  const levels=result.availableLevels ?? result.levels;
  if(levels?.length)add('Administrative levels',names(levels.map(level=>names([level.adminLevel===undefined?level.level:`ADM${level.adminLevel}`,level.levelName]))));
  if(result.boundary && ['Polygon','MultiPolygon'].includes(result.geometryType)){
    outcome=names([result.name,t('Source polygon read')]);
    add('Geometry',result.geometryType);
    add('Area',result.name);
  }
  add('Source',names([result.provenance?.provider ?? result.provider,input.provider,input.product].filter((value,index,all)=>all.indexOf(value)===index)));
  add('Source year',result.provenance?.dataYear ?? result.provenance?.year);
  add('License',result.provenance?.license ?? result.license);
  add('WGS 84 extent',bounds(result.bounds ?? result.context?.bounds ?? input.bounds));
  if(entry.name?.startsWith('geod_dataset_')) {
    add('Data product',result.dataset?.title ?? input.datasetId);
    add('Reference year',result.request?.year ?? input.year);
    add('Product layer',result.request?.layer ?? input.layer);
    add('Declared unit',result.dataset?.unit);
    add('License',result.dataset?.license);
    if(entry.name==='geod_dataset_connect' && Number.isSafeInteger(result.count)) {
      outcome=t(result.complete ? 'Product sources checked · {count}' : 'Product sources partly available · {count}',{count:number(result.count)});
    }
  }
  if(result.cached!==undefined)add('Lookup',t(result.cached?'Read from saved source cache':'Queried source online'));
  if(result.scenes?.length){
    const dates=[...new Set(result.scenes.map(scene=>scene.date).filter(Boolean))].sort();
    add('Returned dates',names([dates[0],dates.at(-1)===dates[0]?undefined:dates.at(-1)]));
    const cloud=result.scenes.map(scene=>scene.cloud).filter(value=>typeof value==='number'&&Number.isFinite(value));
    if(cloud.length)add('Scene cloud range',`${number(Math.min(...cloud),{maximumFractionDigits:2})}–${number(Math.max(...cloud),{maximumFractionDigits:2})}%`);
  }
  const coverage=result.coverage;
  if(coverage){
    const labels={complete:'Catalog coverage complete',partial:'Catalog coverage incomplete',unknown:'Catalog coverage unknown'};
    outcome=t(labels[coverage.status] ?? 'Catalog coverage unknown');
    if(coverage.status!=='unknown'&&typeof coverage.coveredFraction==='number')add('Catalog footprint coverage',number(coverage.coveredFraction,{style:'percent',maximumFractionDigits:2}));
    add('Selected scenes',coverage.sceneCount);
    add('Coverage basis',t('Catalog footprints; valid pixels and local clouds need separate checks.'));
  }
  const plan=(entry.references ?? []).filter(ref=>ref.kind==='plan').map(ref=>plans?.find(plan=>plan.planId===ref.id)).find(Boolean);
  if(plan){
    const labels={pending:'Review awaiting confirmation',submitted:'Review submitted',expired:'Review expired',superseded:'Review replaced',unavailable:'Review unavailable'};
    outcome=t(labels[plan.status] ?? 'Review unavailable');
    add('Source',plan.source);
    add('Selected files',plan.files?.length);
    add('Geometry',plan.polygon?t('Source polygon'):t('Rectangle'));
  }
  const raster=result.raster ?? result;
  if(Number.isSafeInteger(raster.width)&&Number.isSafeInteger(raster.height))add('Raster dimensions',`${number(raster.width)} × ${number(raster.height)} px`);
  add('CRS',raster.crs ?? (raster.epsg?`EPSG:${raster.epsg}`:undefined));
  if(result.status!==undefined && typeof result.settled==='boolean')add('Task status',t(result.status==='succeeded'&&result.settled?'Output ready':'Task is not complete'));
  if(result.context===null&&entry.name==='geod_workspace_context')outcome=t('No map area selected');
  if(!outcome && record && Object.keys(result).length)outcome=t('Query result saved');
  return {outcome,observations,record};
}
