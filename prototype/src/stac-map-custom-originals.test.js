import test from 'node:test';
import assert from 'node:assert/strict';
import proj4 from 'proj4';
import {genericCoordinateToPixel,genericMapMetadata,genericProjectionDefinition} from './stac-map-model.js';

const transform=[463.3127165283333,0,10007554.677014,0,-463.3127165279167,4447802.078664];
const base={crs:'MODIS:Sinusoidal',horizontalCrs:'MODIS:Sinusoidal',verticalCrs:null,storageLayout:'tile',pixelInterpretation:'area',
  width:2400,height:2400,transform,bounds:[transform[2],transform[5]+2400*transform[4],transform[2]+2400*transform[0],transform[5]]};
const burn={...base,sha256:'6537d61e333a8e7e478affd9de6fc7752a4ac60cbf513340cde33b043a04d073',bands:[{index:1,dataType:'Int16',nodata:-1}]};
const qa={...base,sha256:'5f2f9800fcfad9aabc6bbe5beade4ccd5922fd589868212adb012b2bbe6b1820',bands:[{index:1,dataType:'UInt8',nodata:null}]};
test('real MCD64A1 sphere locates paired BurnDate252 and QA3 in the original tile',()=>{
  const definition=genericProjectionDefinition(base.crs),point=[114.13054327659972,34.93124999683757];
  for(const data of [burn,qa]) {
    assert.equal(genericMapMetadata(data),data);
    const native=proj4('EPSG:4326',definition,point);
    assert.ok(Math.abs(native[0]-10404382.018720519)<0.001);
    assert.ok(Math.abs(native[1]-3884182.1590077896)<0.001);
    assert.deepEqual(genericCoordinateToPixel(native,data),{column:856,row:1216});
  }
});
test('custom MODIS identity cannot substitute a different sphere, byte identity, mask or grid',()=>{
  for(const patch of [{sha256:'0'.repeat(64)},{width:2401},{pixelInterpretation:'point'},
    {horizontalCrs:'EPSG:4326'},{bands:[{index:1,dataType:'Int16',nodata:-2}]},
    {transform:[...transform.slice(0,2),transform[2]+1,...transform.slice(3)]}]) assert.throws(()=>genericMapMetadata({...burn,...patch}));
  assert.throws(()=>genericMapMetadata({...qa,bands:[{index:1,dataType:'UInt8',nodata:0}]}));
});
