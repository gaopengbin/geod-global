import React from 'react';
import {Disclosure} from './ui/index.jsx';
import {useI18n} from './i18n.jsx';
export function vectorErrorText(message,t) {
  const reference=/^OSM (node|way|relation)\/(\d+) has missing (node|way|relation)\/(\d+); import a complete extract$/.exec(message);
  if(reference)return t('OSM {owner} has missing {dependency}; import a complete extract.',{owner:`${reference[1]}/${reference[2]}`,dependency:`${reference[3]}/${reference[4]}`});
  const members=/^OSM relation\/(\d+) exceeds 1,024 members$/.exec(message);if(members)return t('OSM relation/{id} exceeds 1,024 members.',{id:members[1]});
  const required=/^Unsupported required OSM PBF feature: (.+)$/.exec(message);if(required)return t('Unsupported required OSM PBF feature: {feature}',{feature:required[1]});
  const missing=/^Shapefile layer (.+) is missing its \.(shp|shx|dbf|prj) companion$/.exec(message);
  if(missing)return t('Shapefile layer {layer} is missing its .{extension} companion.',{layer:missing[1],extension:missing[2]});
  const record=/^Shapefile layer (.+): Shapefile record (\d+): (.+)$/.exec(message);
  if(record)return t('Shapefile layer {layer}, record {record}: {message}',{layer:record[1],record:record[2],message:t(record[3])});
  const layer=/^Shapefile layer (.+): (.+)$/.exec(message);
  return layer?t('Shapefile layer {layer}: {message}',{layer:layer[1],message:t(layer[2])}):t(message);
}
export function ShapefileDetails({source}) {
  const {t,number}=useI18n();if(!source)return null;
  return <><dt>{t('Original format')}</dt><dd>Shapefile · {t(source.container==='zip'?'Original ZIP retained byte for byte':'Exact companion files in a reproducible ZIP bundle')}</dd>
    <dt>{t('Map and GeoJSON coordinates')}</dt><dd>EPSG:4326 · {t('Source Z and M values are retained; heights are not transformed.')}</dd>
    <dt>{t('Numeric attributes')}</dt><dd>{t('DBF numeric fields export as exact decimal strings.')}</dd>
    <dt className="geopackage-section-title">{t('Shapefile layers')}</dt><dd className="geopackage-section-content">{source.layers.map(l=><Disclosure key={l.table} summary={`${l.table} · ${t('{count} features',{count:number(l.featureCount)})}`}><dl className="vector-properties geopackage-layer-properties">
      <dt>{t('Source layer')}</dt><dd>{l.table}</dd><dt>{t('Text encoding')}</dt><dd>{l.encoding} · {l.encodingSource==='cpg'?`CPG: ${l.cpg}`:l.encodingSource==='ldid'?`LDID: 0x${l.languageDriverId.toString(16).padStart(2,'0')}`:t('ASCII text only; no encoding assumed')}</dd>
      <dt>{t('Record handling')}</dt><dd>{t('{deleted} deleted records retained; {empty} null geometries',{deleted:number(l.deletedCount),empty:number(l.nullGeometryCount)})}</dd>
      <dt>{t('Horizontal datum operation')}</dt><dd>{l.coordinateOperation}</dd>
      <dt>{t('Coordinate operation reference')}</dt><dd>{l.coordinateOperationId?`EPSG:${l.coordinateOperationId}`:l.coordinateOperation} · {l.coordinateAccuracyMeters===null?t('Accuracy not specified'):t('Declared accuracy: {value} m',{value:number(l.coordinateAccuracyMeters)})}</dd>
      {l.coordinateOperationArea&&<><dt>{t('Coordinate operation area')}</dt><dd>{l.coordinateOperationArea.join(', ')}</dd></>}
      <dt>{t('Field types and export encoding')}</dt><dd><dl className="vector-properties geopackage-field-properties">{l.fields.map(f=><React.Fragment key={f.name}><dt>{f.name}</dt><dd>{f.fieldType} ({f.width}, {f.decimals}) · {f.jsonEncoding}</dd></React.Fragment>)}</dl></dd>
      <dt>{t('Original coordinate definition')}</dt><dd className="mono">{l.definition}</dd>{l.coordinateDefinition!==l.definition&&<><dt>{t('Coordinate definition used for conversion')}</dt><dd className="mono">{l.coordinateDefinition}</dd></>}
    </dl></Disclosure>)}</dd>
    <dt className="geopackage-section-title">{t('Companion file checksums')}</dt><dd className="geopackage-section-content"><Disclosure summary={t('{count} original files',{count:number(source.files.length)})}><dl className="vector-properties geopackage-layer-properties">{source.files.map(f=><React.Fragment key={f.name}><dt>{f.name}</dt><dd className="mono">{f.sha256} · {number(f.bytes)} B</dd></React.Fragment>)}</dl></Disclosure></dd>
  </>;
}
export function ShapefileNotice({source}) {
  const {t,number}=useI18n();if(!source)return null;
  const deleted=source.layers.reduce((n,l)=>n+l.deletedCount,0),outside=source.layers.reduce((n,l)=>n+l.coordinatesOutsideOperationArea,0);
  return <>{deleted>0&&<p className="vector-source-note" role="note">{t('{count} deleted DBF records are retained in the export and omitted from the map.',{count:number(deleted)})}</p>}{outside>0&&<p className="vector-source-note" role="note">{t('{count} coordinates are outside the conversion method’s declared area. Its stated accuracy does not apply there; the original coordinates remain in the Shapefile.',{count:number(outside)})}</p>}</>;
}
