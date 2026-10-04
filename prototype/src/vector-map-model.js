import Feature from 'ol/Feature.js';
import GeoJSON from 'ol/format/GeoJSON.js';

/** Keep original identities and arbitrary properties separate from OL's keys.
 * Duplicate original IDs are legal; each drawable feature needs a unique key. */
export function vectorFeatures(geojson,{localOsm=false}={}) {
  const format = new GeoJSON();
  return geojson.features.map((raw,index) => {
    const feature = new Feature();
    feature.setId(index);
    feature.set('geodOriginalId',raw.id);
    feature.set('geodOriginalProperties',raw.properties);
    feature.set('geodSourceLayer',localOsm?raw.properties?.osm_type:raw.geodLayer);
    feature.set('geodOriginalMeasures',raw.geodMeasures);
    const geometry=raw.geodDeleted===true?null:drawableGeometry(raw.geometry);
    if (geometry) feature.setGeometry(format.readGeometry(geometry,{
      dataProjection:'EPSG:4326',featureProjection:'EPSG:4326',
    }));
    return feature;
  });
}
// Empty geometries stay in the export and source. They have no map pixels.
function drawableGeometry(g) {
  if(!g)return null;
  if(g.type==='GeometryCollection') {const geometries=g.geometries.map(drawableGeometry).filter(Boolean);return geometries.length?{...g,geometries}:null;}
  return g.coordinates?.length?g:null;
}
