import {afterEach,describe,expect,it,vi} from 'vitest';
import fixture from './overture-native.fixture.json';
import {validateOvertureProvenance,validateOvertureInspection,overtureRequest,OVERTURE_PACKAGE_ID} from './overture-client.js';
const clone=()=>structuredClone(fixture);
afterEach(()=>vi.unstubAllGlobals());
describe('actual retained Overture original and distinct bounded preview',()=>{
  it('accepts actual native geometry/attribute receipt without treating preview bytes as original',()=>{
    const data=clone();expect(validateOvertureInspection(data)).toBe(data);
    expect(data.asset.bytes).toBe(22907);expect(data.asset.geoParquet.originalBytes).toBe(52018294);
    expect(data.asset.featureCount).toBe(11);expect(data.asset.geoParquet.originalRows).toBe(407277);
    expect(data.geojson.features[0].properties.sources[0].dataset).toBe('ETOPO/GLOBathy');
    expect(data.geojson.features[0].properties.sources[0].confidence).toBeNull();
  });
  it.each(['originalBytes','originalSha256','originalRows','originalUrl','packageId','rowGroups','selection','geoMetadata'])('rejects changed original identity field %s',key=>{
    const data=clone();data.asset.geoParquet[key]=key==='geoMetadata'?{}:typeof data.asset.geoParquet[key]==='number'?1:'different';
    expect(()=>validateOvertureProvenance(data.asset.geoParquet,data.asset)).toThrow();
  });
  it('rejects preview count, row identity, bbox and unsupported original-as-GeoJSON claims',()=>{
    let data=clone();data.asset.bytes=52018294;expect(()=>validateOvertureInspection(data)).toThrow();
    data=clone();data.geojson.features[0].properties.bbox.xmin=140;expect(()=>validateOvertureInspection(data)).toThrow();
    data=clone();data.geojson.features[1].geodOriginalRow=data.geojson.features[0].geodOriginalRow;expect(()=>validateOvertureInspection(data)).toThrow();
    data=clone();data.asset.geoParquet.previewFeatureCount=407277;expect(()=>validateOvertureInspection(data)).toThrow();
  });
  it('pins official download and captured AOI before attaching a result',async()=>{
    const data=clone();const fetch=vi.fn().mockResolvedValue({ok:true,json:async()=>data.asset});vi.stubGlobal('fetch',fetch);
    const bounds=data.asset.geoParquet.requestedBounds;
    await expect(overtureRequest('download',{datasetId:'overture-maps',packageId:OVERTURE_PACKAGE_ID,bounds})).resolves.toEqual(data.asset);
    const sent=JSON.parse(fetch.mock.calls[0][1].body);expect(sent.bounds).toEqual(bounds);
    await expect(overtureRequest('download',{datasetId:'overture-maps',packageId:OVERTURE_PACKAGE_ID,bounds:[1,2,3,4]})).rejects.toThrow('different preview area');
    await expect(overtureRequest('download',{datasetId:'overture-maps',packageId:'other',bounds})).rejects.toThrow();
  });
  it('rejects unbounded or unrelated original row pages',async()=>{
    const data=clone(),f=data.geojson.features[0],offset=f.geodOriginalRow;
    const valid={assetId:data.asset.id,originalVerified:true,originalSha256:data.asset.geoParquet.originalSha256,originalRows:407277,offset,
      rows:[{originalRow:offset,properties:f.properties,geometryEncoding:'WKB',geometryBytes:477,geometryWkbSha256:f.geodOriginalWkbSha256,geometryOmitted:true}],complete:false,nextOffset:offset+1};
    const fetch=vi.fn().mockResolvedValue({ok:true,json:async()=>valid});vi.stubGlobal('fetch',fetch);
    await expect(overtureRequest('rows',{id:data.asset.id,offset,limit:1})).resolves.toEqual(valid);
    valid.originalSha256='0'.repeat(64);await expect(overtureRequest('rows',{id:data.asset.id,offset,limit:1})).rejects.toThrow();
    await expect(overtureRequest('rows',{id:data.asset.id,offset:407278,limit:1})).rejects.toThrow();
  });
});
