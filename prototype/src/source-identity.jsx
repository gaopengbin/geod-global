import React, { useState } from 'react';
import { Combine, Database, FileImage, FolderOpen, Globe2, History, LandPlot, Layers, Map, Mountain, Orbit, Plane, Satellite, ScanLine, Shapes, SunMoon } from 'lucide-react';
import { useI18n } from './i18n.jsx';

// Product identity and catalogue platform are separate from download entitlement.
export const sourceIdentity = {
  'earth-search': { name:'Sentinel-2 L2A', detail:'10 m true color · 20 m SCL', platform:'Earth Search', mark:'sentinel-2' },
  'planetary-computer': { name:'Sentinel-2 L2A', detail:'10 m true color · 20 m SCL', platform:'Planetary Computer', mark:'sentinel-2' },
  copernicus: { name:'Sentinel-2 SAFE', detail:'Sentinel-2 L2A original product', platform:'Copernicus Data Space', mark:'sentinel-2' },
  'planetary-radar': { name:'Sentinel-1 RTC', detail:'10 m radar · VV / VH / HH / HV', platform:'Planetary Computer', mark:'sentinel-1' },
  'planetary-landsat': { name:'Landsat 8 / 9', detail:'30 m reflectance + quality layers', platform:'Planetary Computer', mark:'landsat' },
  'planetary-modis': { name:'MODIS · 8-day reflectance', detail:'500 m · MOD09A1 / MYD09A1', platform:'Planetary Computer', mark:'modis' },
  'planetary-vegetation': { name:'MODIS NDVI / EVI', detail:'250 m · 16-day indices + science layers', platform:'Planetary Computer', mark:'modis' },
  'planetary-naip': { name:'NAIP aerial imagery', detail:'US aerial imagery · RGB + NIR', platform:'Planetary Computer', mark:'naip' },
  'copernicus-dem': { name:'Copernicus DEM · GLO-30', detail:'30 m surface elevation · Public AWS tiles', platform:'Earth Search', mark:'copernicus-dem' },
  'copernicus-dem-90': { name:'Copernicus DEM · GLO-90', detail:'90 m surface elevation · Public AWS tiles', platform:'Earth Search', mark:'copernicus-dem' },
  'nasa-srtm': { name:'SRTM · SRTMGL1', detail:'30 m surface elevation', platform:'NASA Earthdata', mark:'srtm' },
  'nasa-earthdata': { name:'HLS · Landsat L30', detail:'30 m reflectance', platform:'NASA Earthdata', mark:'hls' },
  'nasa-viirs-noaa21': { name:'VIIRS · NOAA-21', detail:'1 km · 8-day reflectance', platform:'NASA Earthdata', mark:'viirs' },
  'nasa-viirs-noaa20': { name:'VIIRS · NOAA-20', detail:'1 km · 8-day reflectance', platform:'NASA Earthdata', mark:'viirs' },
  'nasa-viirs-npp': { name:'VIIRS · Suomi-NPP', detail:'1 km · 8-day reflectance', platform:'NASA Earthdata', mark:'viirs' },
};

// Official mission artwork is kept intact and served locally. Other identities
// use GeoD's own Lucide/text treatment, never a fabricated official logo.
const marks = {
  'sentinel-1': { label:'Sentinel-1', Icon:Satellite, src:'/source-marks/sentinel-1.jpg', credit:'Sentinel-1 mission mark © ESA' },
  'sentinel-2': { label:'Sentinel-2', Icon:Satellite, src:'/source-marks/sentinel-2.jpg', credit:'Sentinel-2 mission mark © ESA' },
  landsat: { label:'Landsat', Icon:Orbit },
  modis: { label:'MODIS', Icon:ScanLine, src:'/source-marks/modis.png', credit:'MODIS instrument mark: NASA/GSFC-SBRS' },
  viirs: { label:'VIIRS', Icon:SunMoon },
  'copernicus-dem': { label:'DEM', Icon:Mountain },
  srtm: { label:'SRTM', Icon:LandPlot, src:'/source-marks/srtm.jpg', credit:'SRTM mission mark: NASA, via DLR' },
  naip: { label:'NAIP', Icon:Plane },
  hls: { label:'HLS', Icon:Combine },
  nasa: { label:'NASA', Icon:Globe2 },
  nasadem: { label:'NASADEM', Icon:Mountain },
  stac: { label:'STAC', Icon:Database },
  catalog: { label:'Catalog', Icon:Database },
  raster: { label:'Raster', Icon:FileImage },
  wcs: { label:'WCS', Icon:Layers },
  gibs: { label:'GIBS', Icon:Globe2 },
  wms: { label:'WMS', Icon:Map },
  wmts: { label:'WMTS', Icon:Map },
  xyz: { label:'XYZ', Icon:Map },
  tms: { label:'TMS', Icon:Map },
  arcgis: { label:'ArcGIS', Icon:Layers },
  wayback: { label:'Wayback', Icon:History },
  commercial: { label:'Imagery', Icon:Satellite },
  osm: { label:'OSM', Icon:Map },
  vector: { label:'Vector', Icon:Shapes },
  wfs: { label:'WFS', Icon:Shapes },
  pmtiles: { label:'PMTiles', Icon:Map },
  mbtiles: { label:'MBTiles', Icon:Map },
  gpkg: { label:'GPKG', Icon:FolderOpen },
  shp: { label:'SHP', Icon:FolderOpen },
};

export function SourceSeriesMark({ mark }) {
  const { t } = useI18n();
  const [failedSource, setFailedSource] = useState(null);
  const identity = marks[mark] || { label:mark, Icon:Satellite };
  const { Icon, label, src, credit } = identity;
  const official = src && failedSource !== src;
  return <span className={`source-series-mark source-series-mark-${mark}${official ? ' source-series-mark-official' : ''}`} title={official ? credit : t('{series} series identifier', {series:label})}>
    {official ? <img src={src} width="105" height="34" alt={t('{series} mission mark', {series:label})} loading="lazy" decoding="async" onError={() => setFailedSource(src)}/> : <span aria-hidden="true"><Icon size={19}/><span>{t(label)}</span></span>}
  </span>;
}
