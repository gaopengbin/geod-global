import test from 'node:test';
import assert from 'node:assert/strict';
import proj4 from 'proj4';
import { genericProjectionDefinition, genericMapDisplayBounds, genericMapMetadata, genericCoordinateToPixel } from './stac-map-model.js';
import { deafricaCases } from './public-dataset-deafrica-fixtures.js';

test('actual DE Africa CEA originals locate geographic clicks in the unchanged 30 m native grid', () => {
  const definition=genericProjectionDefinition('EPSG:6933');
  assert.equal(definition,'+proj=cea +lat_ts=30 +lon_0=0 +x_0=0 +y_0=0 +datum=WGS84 +units=m +no_defs');
  for(const layer of ['qa','count_valid','frequency']) {
    const c=deafricaCases.find(c=>c.layer===layer),data=c.inspection;
    assert.equal(genericMapMetadata(data),data);
    assert.deepEqual(data.transform,[30,0,3552000,0,-30,-96000]);
    const location=[36.85,-1.29],coordinate=proj4('EPSG:4326',definition,location);
    assert.deepEqual(genericCoordinateToPixel(coordinate,data),{column:117,row:2285});
    const roundTrip=proj4(definition,'EPSG:4326',coordinate);
    location.forEach((value,index)=>assert.ok(Math.abs(value-roundTrip[index])<1e-9));
    for(const original of c.pixels) {
      const center=[data.transform[2]+(original.column+0.5)*30,data.transform[5]-(original.row+0.5)*30];
      const geographic=proj4(definition,'EPSG:4326',center);
      assert.deepEqual(genericCoordinateToPixel(proj4('EPSG:4326',definition,geographic),data),{column:original.column,row:original.row});
    }
  }
  const actual=deafricaCases.find(c=>c.layer==='qa').pixels.find(p=>p.column===117&&p.row===2285);
  assert.deepEqual(actual.values,[2]);assert.deepEqual(actual.noData,[false]);
});

test('geographic edge cells fit within the physical globe without rewriting the source grid', () => {
  const data = {crs:'EPSG:4326', pixelInterpretation:'area', width:43200, height:20880,
    transform:[0.0083333333,0,-180.00013888885002,0,-0.0083333333,83.99986041515001]};
  const [dx,,x,,,y] = data.transform;
  data.bounds=[x,y + data.height * data.transform[4],x + data.width * dx,y];
  const before=JSON.stringify(data),display=genericMapDisplayBounds(data,data.bounds);
  assert.equal(display[0],-180);assert.equal(display[1],-90);
  assert.deepEqual(display.slice(2),data.bounds.slice(2));
  assert.equal(JSON.stringify(data),before);
  const original=genericCoordinateToPixel([-179.999,-89.999],data);
  assert.deepEqual(original,{column:0,row:20879});
  assert.throws(()=>genericMapDisplayBounds({...data,crs:'EPSG:3857'},data.bounds));
  assert.throws(()=>genericMapDisplayBounds({...data,pixelInterpretation:undefined},data.bounds));
  assert.throws(()=>genericMapDisplayBounds(data,[-180.01,...data.bounds.slice(1)]));
  assert.throws(()=>genericMapDisplayBounds(data,[data.bounds[0],-90.01,...data.bounds.slice(2)]));
  assert.throws(()=>genericMapDisplayBounds(data,[1,2,0,3]));
});

test('NAD83 remains NAD83, while unsupported datum revisions cannot be treated as WGS84', () => {
  assert.equal(genericProjectionDefinition('EPSG:4269'),'+proj=longlat +datum=NAD83 +no_defs');
  assert.equal(genericProjectionDefinition('EPSG:26918'),'+proj=utm +zone=18 +datum=NAD83 +units=m +no_defs');
  assert.throws(()=>genericProjectionDefinition('EPSG:6347'));
  const native=proj4('EPSG:4326',genericProjectionDefinition('EPSG:26918'),[-74,40.7]);
  assert.ok(Math.abs(native[0]-584482.3522822957)<0.001);
  assert.ok(Math.abs(native[1]-4505935.869333813)<0.001);
});
