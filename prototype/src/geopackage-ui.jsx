import React from 'react';
import {Disclosure} from './ui/index.jsx';
import {useI18n} from './i18n.jsx';
export function GeoPackageDetails({source}) {
  const {t,number}=useI18n();if(!source)return null;
  return <><dt>{t('Original format')}</dt><dd>GeoPackage {Math.floor(source.userVersion/10000)}.{Math.floor(source.userVersion/100)%100}.{source.userVersion%100}</dd>
    <dt>{t('Map and GeoJSON coordinates')}</dt><dd>EPSG:4326 · {t('Source Z and M values are retained; heights are not transformed.')}</dd>
    <dt className="geopackage-section-title">{t('GeoPackage layers')}</dt><dd className="geopackage-section-content">{source.layers.map(l=><Disclosure key={l.table} summary={`${l.identifier||l.table} · ${t('{count} features',{count:number(l.featureCount)})}`}><dl className="vector-properties geopackage-layer-properties">
      <dt>{t('Source table')}</dt><dd>{l.table}</dd><dt>{t('Source coordinate system')}</dt><dd>{l.organization}:{l.organizationCoordsysId} · SRS {l.srsId}</dd><dt>{t('Horizontal datum operation')}</dt><dd>{l.coordinateOperation}</dd>
      <dt>{t('Source geometry')}</dt><dd>{l.geometryType} · Z {l.z} · M {l.m}</dd>{l.description&&<><dt>{t('Description')}</dt><dd>{l.description}</dd></>}
      <dt>{t('Coordinate operation reference')}</dt><dd>{l.coordinateOperationId?`EPSG:${l.coordinateOperationId}`:l.coordinateOperation} · {l.coordinateAccuracyMeters===null?t('Accuracy not specified'):t('Declared accuracy: {value} m',{value:number(l.coordinateAccuracyMeters)})}</dd>
      {l.coordinateOperationArea&&<><dt>{t('Coordinate operation area')}</dt><dd>{l.coordinateOperationArea.join(', ')}</dd></>}
      <dt>{t('Field types and export encoding')}</dt><dd><dl className="vector-properties geopackage-field-properties">{l.fields.map(f=><React.Fragment key={f.name}><dt>{f.name}</dt><dd>{f.fieldType} · {f.jsonEncoding}</dd></React.Fragment>)}</dl></dd>
      <dt>{t('Original coordinate definition')}</dt><dd className="mono">{l.definition12063||l.definition}</dd>
      {l.coordinateDefinition!==(l.definition12063||l.definition)&&<><dt>{t('Coordinate definition used for conversion')}</dt><dd className="mono">{l.coordinateDefinition}</dd></>}
    </dl></Disclosure>)}</dd>{source.otherContents.length>0&&<><dt>{t('Other original contents')}</dt><dd>{source.otherContents.map(c=>`${c.table} (${c.dataType})`).join(', ')} · {t('Retained in the original file; not rendered as vector layers.')}</dd></>}
  </>;
}
export function GeoPackageAccuracyNotice({source}) {
  const {t,number}=useI18n();const count=source?.layers.reduce((n,l)=>n+l.coordinatesOutsideOperationArea,0)||0;
  return count>0?<p className="vector-source-note" role="note">{t('{count} coordinates are outside the conversion method’s declared area. Its stated accuracy does not apply there; the original coordinates remain in the GeoPackage.',{count:number(count)})}</p>:null;
}
