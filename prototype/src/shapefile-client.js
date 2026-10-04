const plain=v=>v!==null&&typeof v==='object'&&!Array.isArray(v);
const hash=/^[a-f0-9]{64}$/;
const integer=(n,max)=>Number.isSafeInteger(n)&&n>=0&&n<=max;
const name=n=>typeof n==='string'&&n.length>0&&new TextEncoder().encode(n).length<=240&&!/[\\:\u0000-\u001f\u007f-\u009f]/.test(n)&&n.split('/').every(s=>s&&s!=='.'&&s!=='..'&&!/[. ]$/.test(s));
const encodings={C:'string',N:'decimal-string',F:'decimal-string',L:'boolean-or-null',D:'iso-date-or-null'};
export function validateShapefile(p,count) {
  const fail=()=>{throw new Error('Invalid Shapefile source metadata.');};
  if(!plain(p)||p.conversion!==1||!['zip','sidecars'].includes(p.container)||p.horizontalCrs!=='EPSG:4326'
    ||p.verticalConversion!=='source-z-retained; no-vertical-transformation'||p.measureEncoding!=='geodMeasures; source-values; below-minus-1e38-as-null'
    ||p.numericEncoding!=='DBF N/F as exact trimmed decimal strings'||p.deletedRecords!=='retained-with-geodDeleted; excluded-from-map'
    ||p.polygonRings!=='source-XY orientation-and-containment; no-topology-repair'
    ||!Array.isArray(p.files)||!p.files.length||p.files.length>256||!Array.isArray(p.layers)||!p.layers.length||p.layers.length>32)fail();
  const files=new Set();let bytes=0;
  for(const f of p.files){if(!plain(f)||!name(f.name)||files.has(f.name.toLowerCase())||!integer(f.bytes,20*1024*1024)||!hash.test(f.sha256))fail();files.add(f.name.toLowerCase());bytes+=f.bytes;}
  if(bytes>20*1024*1024)fail();
  const layers=new Set();let records=0;
  for(const l of p.layers){
    if(!plain(l)||!name(l.table+'.shp')||layers.has(l.table.toLowerCase())||![1,3,5,8,11,13,15,18,21,23,25,28].includes(l.shapeType)
      ||!integer(l.featureCount,50000)||!integer(l.deletedCount,l.featureCount)||!integer(l.nullGeometryCount,l.featureCount)
      ||!Array.isArray(l.fields)||l.fields.length>128||!integer(l.languageDriverId,255)
      ||!['cpg','ldid','ascii-only'].includes(l.encodingSource)||typeof l.encoding!=='string'||!l.encoding||l.encoding.length>80
      ||l.cpg!==null&&(typeof l.cpg!=='string'||!l.cpg||l.cpg.length>80)
      ||['definition','coordinateDefinition'].some(k=>typeof l[k]!=='string'||!l[k]||new TextEncoder().encode(l[k]).length>65536)
      ||typeof l.coordinateOperation!=='string'||!l.coordinateOperation||l.coordinateOperation.length>512
      ||l.coordinateOperationId!==null&&!integer(l.coordinateOperationId,4294967295)
      ||!['forward','reverse'].includes(l.coordinateOperationDirection)
      ||l.coordinateAccuracyMeters!==null&&(!Number.isFinite(l.coordinateAccuracyMeters)||l.coordinateAccuracyMeters<0)
      ||l.coordinateOperationArea!==null&&(!Array.isArray(l.coordinateOperationArea)||l.coordinateOperationArea.length!==4||!l.coordinateOperationArea.every(Number.isFinite)||l.coordinateOperationArea[0]<-180||l.coordinateOperationArea[2]>180||l.coordinateOperationArea[1]<-90||l.coordinateOperationArea[3]>90||l.coordinateOperationArea[1]>l.coordinateOperationArea[3])
      ||!integer(l.coordinatesOutsideOperationArea,500000)||!integer(l.coordinatesClampedToBounds,500000))fail();
    const fields=new Set();for(const f of l.fields){if(!plain(f)||typeof f.name!=='string'||!f.name||new TextEncoder().encode(f.name).length>64||/[\u0000-\u001f\u007f-\u009f]/.test(f.name)||fields.has(f.name.toLowerCase())||!Object.hasOwn(encodings,f.fieldType)||encodings[f.fieldType]!==f.jsonEncoding||!integer(f.width,255)||!f.width||!integer(f.decimals,255))fail();fields.add(f.name.toLowerCase());}
    for(const suffix of ['shp','shx','dbf','prj'])if(!files.has(`${l.table}.${suffix}`.toLowerCase()))fail();
    layers.add(l.table.toLowerCase());records+=l.featureCount;
  }
  if(records!==count||records>50000)fail();return p;
}
