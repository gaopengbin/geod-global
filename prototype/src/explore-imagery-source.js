import GeoTIFF from 'ol/source/GeoTIFF.js';
import { assetReadURL } from './providers.js';
import { imageryHrefs, landsatColorStyle, validateLandsatImages } from './explore-imagery.js';
import { validateNaipImages } from './aerial.js';

class NaipSource extends GeoTIFF {
  constructor(scene, sources) {
    super({ sources, normalize: true, interpolate: false });
    this.scene = scene;
    this.on('change', () => {
      if (this.getState() === 'error') this.viewRejector(new Error('NAIP COG metadata does not match the selected RGB + NIR source.'));
    });
  }
  async configure_(sources) {
    try { validateNaipImages(sources, this.scene); await super.configure_(sources); }
    catch {
      this.error_ = new Error('NAIP COG metadata does not match the selected RGB + NIR source.');
      this.setState('error'); this.viewRejector(this.error_); this.dispatchEvent('error');
    }
  }
}

// OpenLayers 10.10.0 does not adjust PixelIsPoint tiepoints. This adapter keeps
// its tile samples untouched and applies the half-pixel translation to geometry.
// configure_ is a version-pinned internal hook: tests cover its rejection and
// actual COG geometry before upgrading OpenLayers or geotiff.js.
class LandsatSource extends GeoTIFF {
  constructor(scene, sources) {
    super({ sources, normalize: false, interpolate: false });
    this.scene = scene;
    // GeoTIFF's initial transport catch changes state but does not reject its
    // view promise. Reject here so metadata failures can be retried promptly.
    this.on('change', () => {
      if (this.getState() === 'error') {
        this.error_ = new Error('Landsat COG metadata does not match the selected original bands.');
        this.viewRejector(this.error_);
      }
    });
  }
  async configure_(sources) {
    try {
      this.pointTransform = validateLandsatImages(sources, this.scene);
      await super.configure_(sources);
    } catch {
      const error = new Error('Landsat COG metadata does not match the selected original bands.');
      this.error_ = error;
      this.setState('error');
      this.viewRejector(error);
      this.dispatchEvent('error');
    }
  }
  determineTransformMatrix(sources) {
    super.determineTransformMatrix(sources);
    this.transformMatrix = this.pointTransform;
  }
}

export function createImagerySource(scene) {
  const hrefs = imageryHrefs(scene);
  if (!hrefs.length) throw new Error('This scene has no supported georeferenced true-color grid.');
  if (scene.provider === 'planetary-naip') return { source: new NaipSource(scene, hrefs.map(href => ({ url: assetReadURL(href), bands: [1, 2, 3], min: 0, max: 255 }))) };
  const sources = hrefs.map(href => ({ url: assetReadURL(href), ...(scene.provider === 'planetary-landsat' ? { bands: [1], nodata: 0 } : {}) }));
  return scene.provider === 'planetary-landsat' ? { source: new LandsatSource(scene, sources), style: landsatColorStyle() }
    : { source: new GeoTIFF({ sources }) };
}
