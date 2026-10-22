import {diagnosticError} from './usage-diagnostics.js';
import XYZ from 'ol/source/XYZ.js';
import TileState from 'ol/TileState.js';
import { transformExtent } from 'ol/proj.js';
import { retryPreviewRequest } from './preview-network.js';

const EMPTY_TILE = 'data:image/png;base64,iVBORw0KGgoAAAANSUhEUgAAAAEAAAABCAYAAAAfFcSJAAAAC0lEQVR42mNgAAIAAAUAAXpeqz8AAAAASUVORK5CYII=';

export async function itemTileBlob(url, signal, fetchImpl = fetch) {
  let attempts=0;try{return await retryPreviewRequest(async () => {
      attempts++;
      const response = await fetchImpl(url, {signal,credentials:'omit',redirect:'error'});
      if (response.status === 404) {
        const body = await response.json().catch(() => null);
        if (/^Tile\(x=\d+, y=\d+, z=\d+\) is outside bounds$/.test(body?.detail || '')) return null;
      }
      if (!response.ok || !response.headers.get('content-type')?.startsWith('image/png')) {
        const error = new Error('The map preview tiles could not load. Check your connection or retry.');
        error.retryable = [408,425,429].includes(response.status) || response.status >= 500;
        throw diagnosticError(error,{stage:'preview_tile',httpStatus:String(response.status),reason:response.ok?'invalid_response':undefined,diagnosticOrigin:'explicit'});
      }
      return await response.blob();
  },signal);}catch(error){throw diagnosticError(error,{stage:'preview_tile',retries:Math.max(0,attempts-1),diagnosticOrigin:'explicit'});}
}

export function createItemPreviewSource(preview) {
  const extent = transformExtent(preview.bounds, 'EPSG:4326', 'EPSG:3857', 16);
  const requests = new Set(), images = new Set();
  let disposed = false;
  const source = new XYZ({
    url:preview.url,projection:'EPSG:3857',crossOrigin:'anonymous',wrapX:false,
    interpolate:false,transition:180,maxZoom:12,
    tileLoadFunction(tile,url) {
      const controller = new AbortController();
      requests.add(controller);
      itemTileBlob(url,controller.signal).then(blob => {
        if (disposed) return;
        const image = tile.getImage();
        if (!blob) { image.src = EMPTY_TILE; return; }
        const objectURL = URL.createObjectURL(blob);
        images.add(objectURL);
        const release = () => { URL.revokeObjectURL(objectURL); images.delete(objectURL); };
        image.addEventListener('load',release,{once:true});
        image.addEventListener('error',()=>{tile.previewFailure=diagnosticError(new Error('Invalid preview image.'),{stage:'preview_decode',reason:'invalid_response',diagnosticOrigin:'explicit'});release();},{once:true});
        image.src = objectURL;
      }).catch(error => { if (!disposed) {tile.previewFailure=error;tile.setState(TileState.ERROR);} })
        .finally(() => requests.delete(controller));
    },
  });
  return {source,extent,dispose() {
    disposed = true;
    requests.forEach(controller => controller.abort()); requests.clear();
    images.forEach(url => URL.revokeObjectURL(url)); images.clear();
    source.dispose();
  }};
}
