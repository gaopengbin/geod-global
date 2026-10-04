import test from 'node:test';
import assert from 'node:assert/strict';
import { readFileSync } from 'node:fs';
import { normalizeScene } from './catalog.js';
import { localRgbGroups, verifiedCompositeMetadata, verifyCompositePixel } from './local-rgb.js';

export function fixture(signed = false) {
  const scene = normalizeScene(JSON.parse(readFileSync(`prototype/public/samples/${signed ? 'nasa' : 'landsat'}-response.json`, 'utf8')).features[0], signed ? 'nasa-earthdata' : 'planetary-landsat');
  const jobs = ['red', 'green', 'blue'].map((assetKey, index) => ({ id: `band-${index}`, kind: 'download', assetKey, itemId: scene.id, status: 'succeeded', href: scene.assets[assetKey].href, sha256: String(index + 1).repeat(64) }));
  const data = { width: 3, height: 2, bandCount: 3, dataType: signed ? 'Int16' : 'UInt16', crs: 'EPSG:32610', bounds: [500000, 4199940, 500090, 4200000], pixelSize: [30, 30], pixelInterpretation: 'PixelIsArea', nodata: signed ? -9999 : 0,
    previewWidth: 3, previewHeight: 2, previewDataUrl: 'data:image/png;base64,AAAA',
    composite: { product: signed ? 'hls-l30-v2' : 'landsat-c2-l2', sources: jobs.map(job => ({ jobId: job.id, sha256: job.sha256, band: job.assetKey })), scale: signed ? .0001 : .0000275, offset: signed ? 0 : -.2, displayRanges: [[100, 200], [200, 300], [300, 400]], sampleCount: 6, validSampleCount: 5 } };
  return { jobs, data, group: localRgbGroups(jobs)[0] };
}

test('groups only complete managed original triplets from the exact scene and processing directory', () => {
  const { jobs, group } = fixture();
  assert.equal(group.assetKey, 'rgb');
  assert.deepEqual(group.sourceJobs.map(job => job.assetKey), ['red', 'green', 'blue']);
  assert.equal(group.sha256, undefined);
  assert.equal(localRgbGroups(jobs.slice(0, 2)).length, 0);
  assert.equal(localRgbGroups(jobs.map((job, i) => i === 2 ? { ...job, status: 'running' } : job)).length, 0);
  assert.equal(localRgbGroups(jobs.map((job, i) => i === 2 ? { ...job, href: job.href.replace(/_(\d{8})_02_T1/g, '_20000101_02_T1') } : job)).length, 0);
  assert.equal(localRgbGroups(jobs.map((job, i) => i === 2 ? { ...job, kind: 'raster_mosaic' } : job)).length, 0);
  const repeated = localRgbGroups([...jobs, { ...jobs[0], id: 'repeated', updatedAt: '2026-10-02' }]);
  assert.equal(repeated[0].sourceJobs[0].id, 'repeated');
});

test('validates product calibration, exact original pins and grid instead of inventing a composite file hash', () => {
  for (const signed of [false, true]) {
    const { data, group } = fixture(signed);
    assert.equal(verifiedCompositeMetadata(group, data), data);
    for (const update of [{ dataType: 'UInt8' }, { pixelSize: [10, 10] }, { bounds: [500000, 4199940, 500080, 4200000] }, { composite: { ...data.composite, scale: 1 } }, { composite: { ...data.composite, sources: data.composite.sources.toReversed() } }])
      assert.throws(() => verifiedCompositeMetadata(group, { ...data, ...update }));
  }
});

test('RGB pixel validation retains signed, zero and above-one reflectance with per-channel NoData', () => {
  const { data, group } = fixture(true);
  const coordinate = [500015, 4199985];
  const pixel = { sources: data.composite.sources, crs: data.crs, coordinate, pixel: [0, 0], center: coordinate, values: [-100, 0, 32767], reflectances: [-.01, 0, 3.2767], channelNoData: [false, false, false], isNoData: false };
  assert.equal(verifyCompositePixel(pixel, group, data, coordinate), pixel);
  const filled = { ...pixel, values: [-9999, 0, 32767], reflectances: [null, 0, 3.2767], channelNoData: [true, false, false], isNoData: true };
  assert.equal(verifyCompositePixel(filled, group, data, coordinate), filled);
  for (const update of [{ reflectances: [0, 0, 1] }, { center: [500000, 4199985] }, { channelNoData: [true, false, false] }, { sources: pixel.sources.slice(0, 2) }])
    assert.throws(() => verifyCompositePixel({ ...pixel, ...update }, group, data, coordinate));
});

test('saved scientific RGB binds its own checksum and original grid without requiring parent records', () => {
  const {data}=fixture(true);
  const job={id:'saved-rgb',kind:'raster_rgb',status:'succeeded',sha256:'f'.repeat(64),rgbSpec:{schemaVersion:'geod-scientific-rgb/v1',profile:{product:data.composite.product,scale:data.composite.scale,offset:data.composite.offset,nodata:data.nodata,signed:true},sources:data.composite.sources,grid:Object.fromEntries(['width','height','crs','bounds','pixelSize','pixelInterpretation'].map(key=>[key,data[key]]))}};
  const result={...data,artifact:{jobId:job.id,sha256:job.sha256},composite:{...data.composite,derived:true}};
  assert.equal(verifiedCompositeMetadata(job,result),result);
  for(const patch of[{artifact:{jobId:'different',sha256:job.sha256}},{artifact:{jobId:job.id,sha256:'0'.repeat(64)}},{width:4},{composite:{...result.composite,derived:false}}])assert.throws(()=>verifiedCompositeMetadata(job,{...result,...patch}));
  const coordinate=[500015,4199985];const pixel={artifact:result.artifact,sources:data.composite.sources,crs:data.crs,coordinate,pixel:[0,0],center:coordinate,values:[-100,0,32767],reflectances:[-.01,0,3.2767],channelNoData:[false,false,false],isNoData:false};
  assert.equal(verifyCompositePixel(pixel,job,result,coordinate),pixel);assert.throws(()=>verifyCompositePixel({...pixel,artifact:{...pixel.artifact,sha256:'1'.repeat(64)}},job,result,coordinate));
});

test('processed triplets require the same project, grid and original scene selection', () => {
  const {jobs,data}=fixture();
  const outputs=jobs.map((source,i)=>({...source,id:'processed-'+i,kind:'raster_mosaic',itemId:'project:one',mosaic:{projectId:'one',sources:[{jobId:source.id,sha256:source.sha256}]},mosaicOutput:{width:3,height:2,bandCount:1,crs:data.crs,bounds:data.bounds,pixelSize:data.pixelSize,calibration:{product:data.composite.product}}}));
  const groups=localRgbGroups([...jobs,...outputs]);const processed=groups.find(group=>group.derived);assert.ok(processed);assert.equal(processed.sourceJobs.length,3);
  assert.equal(localRgbGroups([...jobs,...outputs.map((job,i)=>i===1?{...job,mosaic:{...job.mosaic,projectId:'other'}}:job)]).filter(g=>g.derived).length,0);
  assert.equal(localRgbGroups([...jobs,...outputs.map((job,i)=>i===2?{...job,mosaicOutput:{...job.mosaicOutput,bounds:[500000,4199940,500120,4200000]}}:job)]).filter(g=>g.derived).length,0);
  assert.equal(localRgbGroups([...jobs,...outputs.map((job,i)=>i===1?{...job,mosaic:{...job.mosaic,sources:[{jobId:jobs[2].id,sha256:jobs[2].sha256}]}}:job)]).filter(g=>g.derived).length,0);
});
