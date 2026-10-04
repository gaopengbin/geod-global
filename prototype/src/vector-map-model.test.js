import {test} from 'node:test';
import assert from 'node:assert/strict';
import VectorSource from 'ol/source/Vector.js';
import {vectorFeatures} from './vector-map-model.js';
test('OSM object filtering uses validated asset context rather than arbitrary GeoJSON foreign members',()=>{
  const raw={type:'FeatureCollection',geodLocalOsm:{untrusted:true},features:[{type:'Feature',properties:null,geometry:{type:'Point',coordinates:[12,48]}}]};
  assert.equal(vectorFeatures(raw)[0].get('geodSourceLayer'),undefined);
  const osm={type:'FeatureCollection',features:[{type:'Feature',id:'way/1',properties:{osm_type:'way',tags:{name:'test'}},geometry:{type:'LineString',coordinates:[[12,48],[13,49]]}}]};
  assert.equal(vectorFeatures(osm,{localOsm:true})[0].get('geodSourceLayer'),'way');assert.equal(vectorFeatures(osm)[0].get('geodSourceLayer'),undefined);
});
test('deleted Shapefile records retain attributes and measures but contribute no map geometry',()=>{
  const raw={type:'FeatureCollection',features:[{type:'Feature',id:1,geodLayer:'places',geodDeleted:true,geodMeasures:7.5,properties:{large:'9007199254740993'},geometry:{type:'Point',coordinates:[12,48,18]}},{type:'Feature',id:2,geodLayer:'places',properties:{},geometry:{type:'Point',coordinates:[13,49]}}]};
  const before=JSON.stringify(raw),features=vectorFeatures(raw);
  assert.equal(features[0].getGeometry(),undefined);assert.deepEqual(features[0].get('geodOriginalProperties'),raw.features[0].properties);assert.equal(features[0].get('geodOriginalMeasures'),7.5);assert.deepEqual(features[1].getGeometry().getCoordinates(),[13,49]);assert.equal(JSON.stringify(raw),before);
});

test('map keeps duplicate original IDs, elevations and untrusted property keys without replacing geometry',()=>{
  const properties=JSON.parse('{"geometry":"original attribute","__proto__":{"polluted":true},"name":"<script>"}');
  const geojson={type:'FeatureCollection',features:[
    {type:'Feature',id:'duplicate',properties,geometry:{type:'Point',coordinates:[13,52,100]}},
    {type:'Feature',id:'duplicate',properties:null,geometry:{type:'LineString',coordinates:[[13,52,1],[14,53,2]]}},
    {type:'Feature',properties:{},geometry:null},
  ]};
  const features=vectorFeatures(geojson),source=new VectorSource({features,wrapX:false});
  assert.equal(source.getFeatures().length,3);
  assert.deepEqual(features[0].getGeometry().getCoordinates(),[13,52,100]);
  assert.deepEqual(features[1].getGeometry().getCoordinates(),[[13,52,1],[14,53,2]]);
  assert.equal(features[0].get('geodOriginalId'),features[1].get('geodOriginalId'));
  assert.notEqual(features[0].getId(),features[1].getId());
  assert.deepEqual(features[0].get('geodOriginalProperties'),properties);
  assert.equal({}.polluted,undefined);
  assert.equal(features[2].getGeometry(),undefined);
});
test('GeoPackage layer identities, M values and empty members remain separate from drawing geometry',()=>{
  const raw={type:'FeatureCollection',features:[
    {type:'Feature',id:1,geodLayer:'all',geodMeasures:[null,7.5],properties:{geodSourceLayer:'untrusted value'},geometry:{type:'GeometryCollection',geodOriginalGeometryType:'MultiPoint',geometries:[{type:'Point',coordinates:[]},{type:'Point',coordinates:[12,48,18.25]}]}},
    {type:'Feature',id:1,geodLayer:'second',properties:{},geometry:{type:'LineString',coordinates:[]}},
  ]};
  const sourceCopy=JSON.stringify(raw),features=vectorFeatures(raw);
  assert.equal(features[0].get('geodSourceLayer'),'all');
  assert.equal(features[0].get('geodOriginalProperties').geodSourceLayer,'untrusted value');
  assert.deepEqual(features[0].get('geodOriginalMeasures'),[null,7.5]);
  assert.deepEqual(features[0].getGeometry().getGeometries()[0].getCoordinates(),[12,48,18.25]);
  assert.equal(features[1].getGeometry(),undefined);
  assert.notEqual(features[0].getId(),features[1].getId());assert.equal(JSON.stringify(raw),sourceCopy);
});
