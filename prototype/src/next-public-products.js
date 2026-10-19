// Shared scope checks for the next ten scientific products. These imports are
// the same immutable original declarations used by the native adapters.
import itemRecords from '../../crates/geod-runtime/src/datasets/next-planetary-reviewed-items.json' with { type: 'json' };
import collectionRecords from '../../crates/geod-runtime/src/datasets/next-planetary-reviewed-collections.json' with { type: 'json' };
import additionalItems from '../../crates/geod-runtime/src/datasets/additional-pc-reviewed-items.json' with { type: 'json' };
import additionalCollections from '../../crates/geod-runtime/src/datasets/additional-pc-reviewed-collections.json' with { type: 'json' };
import ghslRecords from '../../crates/geod-runtime/src/datasets/next-ghsl-profiles.json' with { type: 'json' };
import hydroRecords from '../../crates/geod-runtime/src/datasets/next-hydro-geoid-reviewed-index.json' with { type: 'json' };
import soilRecords from '../../crates/geod-runtime/src/datasets/soilgrids-reviewed-index.json' with { type: 'json' };

const object = value => value !== null && typeof value === 'object' && !Array.isArray(value);
const same = (a,b) => a===b || Array.isArray(a)&&Array.isArray(b)&&a.length===b.length&&a.every((v,i)=>same(v,b[i]))
  || object(a)&&object(b)&&Object.keys(a).length===Object.keys(b).length&&Object.keys(a).every(k=>same(a[k],b[k]));
const noSelectors = request => ['country','region','season'].every(key=>request?.[key]==null);
const declaration = 'reviewed-client-preset; publisher file bytes and embedded metadata verified separately';
// Local full-transfer references are separate from unchanged publisher metadata.
export const ADDITIONAL_PC_ORIGINALS=Object.freeze({
  'planetary-more':{sst:{bytes:1096226,sha256:'47b19bb75c95cdd302b1372a4d6a5a910c3e7213ccc013e6fee5f82f48646bfa'}},
  'modis-more':{Burn_Date:{bytes:112419,sha256:'6537d61e333a8e7e478affd9de6fc7752a4ac60cbf513340cde33b043a04d073'},QA:{bytes:811891,sha256:'5f2f9800fcfad9aabc6bbe5beade4ccd5922fd589868212adb012b2bbe6b1820'}},
});
export const NEXT_PC_PRODUCTS = Object.freeze({
  'pc-hrea':{account:'hreadatasa',container:'hrea',year:2019,month:null,keys:['lightscore'],units:{lightscore:'Dimensionless predicted probability'},title:'HREA · Settlement electricity access',version:'HREA v1 · Hong Kong · 2019 reference year'},
  'pc-biodiversity':{account:'pcdata01euw',container:'impact',year:2020,month:null,keys:['data'],units:{data:'Dimensionless model score'},title:'Impact Observatory · Biodiversity intactness',version:'Biodiversity Intactness v1 · 2020 · one reviewed 7.18° regional tile'},
  'noaa-ohc':{account:'noaacdr',container:'cogs',year:2020,month:null,keys:['heat_content'],units:{heat_content:'10^18 joules'},title:'NOAA · Ocean heat content anomaly',version:'NOAA OHC CDR · 2020 Q4 · 0–700 m · published COG derivative'},
  'noaa-nclimgrid':{account:'nclimgridwesteurope',container:'nclimgrid-cogs',year:2022,month:9,keys:['tavg','tmax','tmin','prcp'],units:{tavg:'degree Celsius',tmax:'degree Celsius',tmin:'degree Celsius',prcp:'mm'},title:'NOAA NClimGrid · Monthly climate observations',version:'NClimGrid Monthly · September 2022 · reviewed CONUS published COGs'},
  'planetary-more':{account:'noaacdr',container:'cogs',year:2020,month:9,keys:['sst'],units:{sst:'Celsius after declared scaling; original Int16 DN retained'},title:'NOAA OISST · Daily sea-surface temperature',version:'OISST v2.1 · 2020-09-01 · fixed published SST COG derivative',scales:{sst:0.009999999776482582},nodata:{sst:-999}},
  'modis-more':{account:'modiseuwest',container:'modis-061-cogs',year:2020,month:9,keys:['Burn_Date','QA'],units:{Burn_Date:'day of year / original -2, -1 and 0 codes',QA:'bit field'},title:'MODIS MCD64A1 · Monthly burned area',version:'MCD64A1 V061 · September 2020 · h27v05 · published COG derivatives',scales:{Burn_Date:1,QA:1},nodata:{Burn_Date:-1,QA:null}},
});
export const isNextPlanetaryProduct = id => Object.hasOwn(NEXT_PC_PRODUCTS,id);

export function validNextPlanetarySnapshot(item, asset) {
  const proof=item?.provenance?.reviewedDataset, id=proof?.dataset?.id, profile=NEXT_PC_PRODUCTS[id];
  const additional=Object.hasOwn(ADDITIONAL_PC_ORIGINALS,id??'');
  const record=(additional?additionalItems:itemRecords).find(r=>r.datasetId===id), collection=(additional?additionalCollections:collectionRecords).find(r=>r.datasetId===id);
  const raw=record?.document, request=proof?.request, key=asset?.key, original=raw?.assets?.[key];
  if(!profile || !raw || !original || !collection || proof.schemaVersion!==2 || proof.catalogRaster!=null
    || proof.declarationKind!==declaration || proof.dataset.discovery!=='stac-items'
    || proof.dataset.title!==profile.title || proof.dataset.version!==profile.version
    || !same(proof.dataset.years,[profile.year]) || proof.dataset.defaultYear!==profile.year
    || !same(proof.dataset.months||[],profile.month==null?[]:[profile.month]) || (proof.dataset.defaultMonth??null)!==profile.month
    || request?.datasetId!==id || request.year!==profile.year
    || (request.month??null)!==profile.month || !noSelectors(request) || !profile.keys.includes(key)
    || request.layer!==key || proof.layer?.id!==key || proof.layer.unit!==profile.units[key] || proof.layer.scale!==(profile.scales?.[key]??1) || proof.layer.offset!==0
    || (proof.layer.nodata??null)!==(additional?profile.nodata[key]:id==='pc-hrea'?-3.3999999521443642e38:null)
    || !same(proof.layer,proof.dataset.layers?.find(layer=>layer.id===key))
    || proof.sourceId!==raw.id || proof.sourceTitle!==`${profile.title} · ${raw.id}` || !same(proof.sourceBounds,raw.bbox)
    || item.itemId!==raw.id || item.collectionId!==raw.collection || !same(item.bbox,raw.bbox)
    || !same(item.geometry,raw.geometry) || !same(item.properties,raw.properties)
    || item.provenance.documentUrl!==record.url || !same(item.provenance.collection,collection.document)
    || !same(asset.metadata,original) || !same(asset.roles,original.roles) || asset.mediaType!==original.type
    || asset.href!==original.href || asset.originalHref!==original.href || asset.transportAdapter!=='planetary-anonymous-sas') return false;
  if(additional && (proof.dataset.license!==collection.document.license
    || !same(proof.dataset.layers?.map(layer=>layer.id),profile.keys)
    || !same(item.provenance.assetTransports,[{assetKey:key,originalHref:original.href,href:original.href,adapter:'planetary-anonymous-sas'}]))) return false;
  try {
    const url=new URL(asset.href);
    if(url.origin!==`https://${profile.account}.blob.core.windows.net` || url.search || url.hash
      || url.username || url.password || !url.pathname.startsWith(`/${profile.container}/`)) return false;
  } catch { return false; }
  return item.datetime===(raw.properties.datetime??null)
    && item.startDatetime===(raw.properties.start_datetime??null) && item.endDatetime===(raw.properties.end_datetime??null);
}

export function nextPlanetaryRange(dataset,asset,item) {
  if(!isNextPlanetaryProduct(dataset?.id) || !same(dataset,item?.provenance?.reviewedDataset?.dataset)
    || !validNextPlanetarySnapshot(item,asset)) return undefined;
  // These published profiles are Float32: HREA has its original decimal fill,
  // biodiversity has no scalar NoData, and both NOAA products declare NaN.
  // Neither rendering expressions nor new missing-value declarations are used.
  // These extra products display raw stored DN/codes, with no Celsius conversion
  // or QA masking inserted into publisher TIFF values.
  if(dataset.id==='planetary-more') return [-200,3500];
  if(dataset.id==='modis-more') return asset.key==='Burn_Date'?[-2,366]:[0,255];
  return dataset.id==='noaa-ohc' ? [-70,70]
    : dataset.id==='noaa-nclimgrid' ? asset.key==='prcp' ? [0,500] : [-5,45] : [0,1];
}

// Online TIFF ranges establish a selected source and its physical grid, not a
// complete-file checksum. Only this factory can issue the custom-grid token;
// a dataset ID, projection string or copied JSON object cannot enable it.
const issuedMcdPreviewPolicies = new WeakSet();
export function nextOverviewPolicy(dataset, asset, item) {
  if (dataset?.id !== 'modis-more' || !same(dataset,item?.provenance?.reviewedDataset?.dataset)
    || !validNextPlanetarySnapshot(item,asset) || !/^[a-f0-9]{64}$/.test(item.id || '')
    || asset.eligible !== true || asset.reason !== null || !Array.isArray(item.assets)
    || item.assets.filter(candidate=>candidate.key===asset.key).length !== 1
    || !same(item.assets.find(candidate=>candidate.key===asset.key),asset)) return undefined;
  const policy=Object.freeze({kind:'mcd64a1-v061-202009-h27v05-online-grid',snapshotId:item.id,
    assetKey:asset.key,href:asset.href,bytes:ADDITIONAL_PC_ORIGINALS['modis-more'][asset.key].bytes,
    crs:'MODIS:Sinusoidal'});
  issuedMcdPreviewPolicies.add(policy);
  return policy;
}

export function isMcdOverviewPolicy(policy, binding = {}) {
  return object(policy) && issuedMcdPreviewPolicies.has(policy)
    && (binding.snapshotId === undefined || policy.snapshotId === binding.snapshotId)
    && (binding.assetKey === undefined || policy.assetKey === binding.assetKey);
}

const ghslRanges={'ghsl-built-surface':[0,500000],'ghsl-built-volume':[0,12000000],'ghsl-population':[0,15000]};
const ethHeaders={N51E003:{bytes:171132359,bounds:[3,51,6,54]},N39E114:{bytes:301900420,bounds:[114,39,117,42]},
  N21E111:{bytes:304474031,bounds:[111,21,114,24]},N45E006:{bytes:407025206,bounds:[6,45,9,48]}};
export const NEXT_DIRECT_PRODUCTS = new Set([...Object.keys(ghslRanges),'eth-canopy-height','hydrosheds-accumulation','etopo-geoid','soilgrids']);
const physical = bounds => [Math.max(-180,bounds[0]),Math.max(-90,bounds[1]),Math.min(180,bounds[2]),Math.min(90,bounds[3])];

export function nextDirectRange(dataset,asset) {
  const proof=asset?.metadata?.datasetPreset, request=proof?.request;
  if(!NEXT_DIRECT_PRODUCTS.has(dataset?.id) || asset?.key!=='raster' || asset.metadata.kind!=='direct-raster'
    || asset.metadata.href!==asset.href || proof?.schemaVersion!==2 || !same(proof.dataset,dataset)
    || request?.datasetId!==dataset.id || !noSelectors(request) || request.month!=null
    || proof.declarationKind!==declaration || proof.catalogRaster!=null
    || !same(proof.layer,dataset.layers?.find(layer=>layer.id===request.layer))
    || proof.layer?.scale!==1 || proof.layer.offset!==0) return undefined;
  // The 16-byte connection probe cannot declare TIFF dtype/NoData. Each
  // applicability check below binds an independently reviewed physical profile
  // to its exact canonical original; the map decoder reads original tags and
  // masks separately. These display ranges never supply or replace NoData.
  if(dataset.id==='soilgrids') {
    const record=soilRecords.originals.find(r=>r.id===proof.sourceId&&r.layer===request.layer&&r.url===asset.href);
    if(!record||request.year!=null||dataset.title!=='SoilGrids250m WRB 2017 corrected originals'||dataset.version!==soilRecords.release
      ||proof.layer.nodata!==255||!same(proof.sourceBounds,record.bounds)||record.dataType!=='UInt8'||record.nodata!==255
      ||proof.layer.unit!==(request.layer==='most-probable'?'WRB class code':'raw score'))return undefined;
    return request.layer==='most-probable'?[0,29]:[0,Math.max(...soilRecords.originals.filter(r=>r.layer==='acrisols-score').map(r=>r.maximum))];
  }
  const ghsl=ghslRecords.profiles.find(p=>p.datasetId===dataset.id);
  if(ghsl) return request.year===2020 && request.layer===ghsl.layer && (proof.layer.nodata??null)===null
    && dataset.title===ghsl.title && proof.layer.unit===ghsl.unit && asset.href===ghsl.url
    && proof.sourceId===ghsl.url.split('/').at(-1).replace(/\.tif$/,'') && same(proof.sourceBounds,ghsl.queryBounds)
    && ghsl.nodata===null && ghsl.scale===1 && ghsl.offset===0
    && ['UInt32','Float64'].includes(ghsl.dataType) ? [...ghslRanges[dataset.id]] : undefined;
  if(dataset.id==='eth-canopy-height') {
    const header=ethHeaders[proof.sourceId];
    return request.year===2020 && request.layer==='height' && proof.layer.nodata===255 && proof.layer.unit==='m'
      && dataset.title==='ETH global canopy height · 2020' && header && same(proof.sourceBounds,header.bounds)
      && asset.href===`https://libdrive.ethz.ch/index.php/s/cO8or7iOe5dT2Rt/download?path=%2F3deg_cogs&files=ETH_GlobalCanopyHeight_10m_2020_${proof.sourceId}_Map.tif`
      ? [0,50] : undefined;
  }
  const record=hydroRecords.originals.find(r=>r.datasetId===dataset.id && r.layer===request.layer && r.id===proof.sourceId && r.url===asset.href);
  if(!record || request.year!=null || record.scale!==1 || record.offset!==0 || !same(proof.sourceBounds,physical(record.bounds))
    || (proof.layer.nodata??null)!==record.nodata) return undefined;
  if(dataset.id==='etopo-geoid') return request.layer==='geoid' && record.dtype==='Float32' && record.nodata===null && proof.layer.unit==='m' ? [-120,120] : undefined;
  return record.dtype==='Float64' && record.nodata===1.79e308
    ? request.layer==='acc' && proof.layer.unit==='cells' ? [0,10000] : request.layer==='aca' && proof.layer.unit==='km²' ? [0,100] : undefined : undefined;
}
