import test from 'node:test';
import assert from 'node:assert/strict';
import {observeDownloadProgress,downloadQueueCounts,downloadStatusLabel} from './download-observation.js';
const job={id:'a',kind:'download',status:'running',bytesDownloaded:0,totalBytes:1000000};
test('queue observations never become a zero-speed transfer or inherit an earlier attempt rate',()=>{
 let observations=observeDownloadProgress(new Map(),[{...job,status:'queued'}],1000);
 observations=observeDownloadProgress(observations,[{...job,status:'queued'}],61000);
 assert.equal(observations.get('a').rate,null);assert.equal(observations.get('a').idleMs,0);
 observations=observeDownloadProgress(observations,[job],62000);
 assert.equal(observations.get('a').idleMs,0);assert.equal(observations.get('a').rate,null);
 observations=observeDownloadProgress(observations,[{...job,bytesDownloaded:20000}],64000);
 assert.equal(observations.get('a').rate,10000);
 observations=observeDownloadProgress(observations,[{...job,bytesDownloaded:20000}],77000);
 assert.equal(observations.get('a').rate,null);assert.equal(observations.get('a').idleMs,13000);
 observations=observeDownloadProgress(observations,[job],79000);
 assert.equal(observations.get('a').rate,null);assert.equal(observations.get('a').idleMs,0);
 assert.equal(observeDownloadProgress(observations,[{...job,status:'cancelled'}],80000).size,0);
});
test('download states distinguish queue admission, response, first bytes and final validation',()=>{
 assert.equal(downloadStatusLabel({...job,status:'queued'}),'Waiting for download slot');
 assert.equal(downloadStatusLabel({...job,totalBytes:null}),'Connecting to source');
 assert.equal(downloadStatusLabel(job),'Waiting for source data');
 assert.equal(downloadStatusLabel({...job,bytesDownloaded:1}),'Downloading');
 assert.equal(downloadStatusLabel({...job,bytesDownloaded:1000000}),'Verifying download');
 assert.equal(downloadStatusLabel({...job,kind:'raster_mosaic'}),null);
 assert.deepEqual(downloadQueueCounts([job,job,{...job,id:'b',status:'queued'},{...job,id:'c',kind:'raster_mosaic'}]),{running:1,queued:1});
});
