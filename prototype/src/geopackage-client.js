const plain=v=>v!==null&&typeof v==='object'&&!Array.isArray(v);
const text=(s,max)=>typeof s==='string'&&new TextEncoder().encode(s).length<=max;
const name=s=>text(s,960)&&s.length>0&&!/[\u0000-\u001f\u007f-\u009f]/.test(s)&&Array.from(s).length<=240;
const integer=(n,min,max)=>Number.isSafeInteger(n)&&n>=min&&n<=max;
const kinds=new Set(['POINT','LINESTRING','POLYGON','MULTIPOINT','MULTILINESTRING','MULTIPOLYGON','GEOMETRYCOLLECTION','GEOMETRY']);
const encodings=new Set(['boolean','number-or-decimal-string','number','string','base64','geometry']);
export function validateGeoPackage(source,count) {
  const fail=()=>{throw new Error('Invalid GeoPackage source or layer metadata.');};
  if(!plain(source)||source.conversion!==1||!integer(source.userVersion,10200,10499)||source.horizontalCrs!=='EPSG:4326'||source.verticalConversion!=='source-z-retained; no-vertical-transformation'||source.measureEncoding!=='geodMeasures; source-values; NaN-as-null'||!Array.isArray(source.layers)||!source.layers.length||source.layers.length>32||!Array.isArray(source.otherContents)||source.otherContents.length>32)fail();
  const layers=new Set();let total=0;
  for(const l of source.layers) {
    if(!plain(l)||!name(l.table)||layers.has(l.table)||!(l.identifier===null||text(l.identifier,4096))||!text(l.description,65536)||!name(l.geometryColumn)||!kinds.has(l.geometryType)||!integer(l.srsId,-2147483648,2147483647)||!name(l.organization)||Array.from(l.organization).length>80||!integer(l.organizationCoordsysId,-2147483648,2147483647)||!text(l.definition,65536)||!l.definition||!(l.definition12063===null||text(l.definition12063,65536))||!integer(l.z,0,2)||!integer(l.m,0,2)||!integer(l.featureCount,0,50000)||!text(l.coordinateOperation,4096)||!l.coordinateOperation||!Array.isArray(l.fields)||!l.fields.length||l.fields.length>128)fail();
    if(!text(l.coordinateDefinition,65792)||!l.coordinateDefinition)fail();
    if(!(l.coordinateOperationId===null||integer(l.coordinateOperationId,1,4294967295))||!['forward','reverse'].includes(l.coordinateOperationDirection)||!(l.coordinateAccuracyMeters===null||Number.isFinite(l.coordinateAccuracyMeters)&&l.coordinateAccuracyMeters>=0)||!integer(l.coordinatesOutsideOperationArea,0,500000)||!(l.coordinateOperationArea===null||Array.isArray(l.coordinateOperationArea)&&l.coordinateOperationArea.length===4&&l.coordinateOperationArea.every(Number.isFinite)&&Math.abs(l.coordinateOperationArea[0])<=180&&Math.abs(l.coordinateOperationArea[2])<=180&&l.coordinateOperationArea[1]>=-90&&l.coordinateOperationArea[3]<=90&&l.coordinateOperationArea[1]<=l.coordinateOperationArea[3]))fail();
    layers.add(l.table);total+=l.featureCount;const fields=new Set();let keys=0,geometry=0;
    for(const f of l.fields) {
      if(!plain(f)||!name(f.name)||fields.has(f.name)||!text(f.fieldType,128)||typeof f.nullable!=='boolean'||typeof f.primaryKey!=='boolean'||!encodings.has(f.jsonEncoding))fail();
      fields.add(f.name);if(f.primaryKey){keys++;if(f.jsonEncoding!=='number-or-decimal-string')fail();}if(f.name===l.geometryColumn){geometry++;if(f.jsonEncoding!=='geometry')fail();}
    }
    if(keys!==1||geometry!==1)fail();
  }
  for(const c of source.otherContents){if(!plain(c)||!name(c.table)||layers.has(c.table)||!name(c.dataType)||Array.from(c.dataType).length>80)fail();layers.add(c.table);}
  if(total!==count)fail();return source;
}
