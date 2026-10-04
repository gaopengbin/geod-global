import assert from 'node:assert/strict';
import { readFile } from 'node:fs/promises';
import path from 'node:path';

// GeoTIFF metadata and tiles use fetch, not img-src. Its decoder pool creates
// local blob workers. Keep these separate from permission to run remote scripts.
export function verifyDesktopCspConfig(config) {
  const imagery = 'https://sentinel-cogs.s3.us-west-2.amazonaws.com';
  const catalog = 'https://earth-search.aws.element84.com';
  for (const [name, development] of [['csp', false], ['devCsp', true]]) {
    const entries = config.app.security[name].split(';').map(value => value.trim().split(/\s+/)).filter(value => value[0]);
    assert.equal(new Set(entries.map(([key]) => key)).size, entries.length, 'Duplicate CSP directive');
    const directives = new Map(entries.map(([key, ...values]) => [key, values]));
    const expected = ["'self'", 'blob:', 'ipc:', 'http://ipc.localhost', 'geod-elevation:', 'http://geod-elevation.localhost', catalog, imagery,
      'https://planetarycomputer.microsoft.com', 'https://stac.dataspace.copernicus.eu', 'https://sentinel2l2a01.blob.core.windows.net', 'https://landsateuwest.blob.core.windows.net', 'https://sentinel1euwestrtc.blob.core.windows.net', 'https://naipeuwest.blob.core.windows.net', 'https://cmr.earthdata.nasa.gov',
      'https://copernicus-dem-30m.s3.eu-central-1.amazonaws.com', 'https://copernicus-dem-90m.s3.eu-central-1.amazonaws.com',
      ...(development ? ['http://127.0.0.1:4317', 'ws://127.0.0.1:4317'] : [])];
    assert.deepEqual([...(directives.get('connect-src') || [])].sort(), expected.sort(), `${name}: catalog and COG requests need the exact reviewed origins`);
    assert.ok(directives.get('img-src')?.includes('https://data.lpdaac.earthdatacloud.nasa.gov'), `${name}: NASA public browse imagery needs its reviewed origin`);
    assert.ok(directives.get('img-src')?.includes('https://d1nklfio7vscoe.cloudfront.net'), `${name}: NASA public browse redirects need their exact reviewed CDN origin`);
    assert.deepEqual(directives.get('script-src'), ["'self'", "'wasm-unsafe-eval'"], `${name}: local WebAssembly compilation is allowed; remote/inline/JavaScript eval remain prohibited`);
    assert.deepEqual(directives.get('worker-src'), ["'self'", 'blob:'], `${name}: only bundled/local GeoTIFF decoder workers`);
    for (const directive of ['object-src', 'frame-src', 'form-action']) {
      assert.deepEqual(directives.get(directive), ["'none'"]);
    }
  }
  return { desktopCogPolicy: 'passed', remoteScriptOrigins: false, localDecoderWorkers: true, localWasmCompilation: true };
}

export async function verifyDesktopCsp(root) {
  return verifyDesktopCspConfig(JSON.parse(await readFile(path.join(root, 'src-tauri/tauri.conf.json'), 'utf8')));
}
