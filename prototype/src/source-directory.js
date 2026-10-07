import { PROVIDERS } from './providers.js';
import { originalsReleased } from './release-policy.js';
import openDataCandidates from './open-data-candidates.json';

// Current 2D product scope. Planned entries are visible but have no action.
// A supported protocol is a connection path, not blanket provider entitlement.
const productCategory = provider => provider.domain === 'elevation' ? 'elevation' : provider.domain === 'radar' ? 'radar' : 'imagery';
const featured = ['earth-search','planetary-landsat','planetary-radar','planetary-vegetation','copernicus-dem','planetary-naip','planetary-modis','planetary-computer','copernicus-dem-90'];
const order = provider => featured.includes(provider.id) ? featured.indexOf(provider.id) : featured.length;
const entry = (id, name, detail, platform, mark, series, category, group, action) => ({id,name,detail,platform,mark,series,category,group,status:action ? 'ready' : 'planned',action});

export const SOURCE_GROUPS = [
  {id:'optical',label:'Optical and aerial imagery'},
  {id:'radar',label:'SAR radar imagery'},
  {id:'elevation',label:'Elevation and bathymetry'},
  {id:'land-cover',label:'Land-cover products'},
  {id:'vegetation',label:'Vegetation and forest products'},
  {id:'hydrology',label:'Water and hydrology products'},
  {id:'population',label:'Population and settlement products'},
  {id:'soil',label:'Soil products'},
  {id:'climate',label:'Climate and precipitation products'},
  {id:'nightlights',label:'Nighttime light products'},
  {id:'vectors',label:'Vector datasets and services'},
  {id:'maps',label:'Map imagery and history'},
  {id:'tiles',label:'Offline map archives'},
  {id:'catalogs',label:'Raster catalogs and coverages'},
  {id:'local',label:'Local 2D files'},
  {id:'portals',label:'Public data portals'},
];

// Group the board by the data product; platform and access remain independent.
const productGroup = source => {
  if (source.series === 'local') return 'local';
  if (source.group === 'portals' || source.group === 'catalogs') return source.group;
  if (source.providerId === 'planetary-vegetation' || source.id === 'glad-forest') return 'vegetation';
  if (source.series === 'eog') return 'nightlights';
  if (['land-cover','hydrology','population','soil','climate'].includes(source.series)) return source.series;
  if (source.category === 'radar' || source.category === 'elevation') return source.category;
  if (source.category === 'vectors' || source.category === 'tiles' || source.category === 'maps') return source.category;
  return 'optical';
};
export const DIRECTORY_FAMILIES = [
  {id:'earth-search',label:'Earth Search'}, {id:'planetary',label:'Planetary Computer'},
  {id:'copernicus',label:'Copernicus'}, {id:'nasa',label:'NASA Earthdata'},
  {id:'nasadem',label:'NASADEM'}, {id:'gibs',label:'NASA GIBS'},
  {id:'osm',label:'OpenStreetMap'}, {id:'protomaps',label:'Protomaps / PMTiles'},
  {id:'map-services',label:'Map services'}, {id:'vector-services',label:'Vector services'},
  {id:'custom-raster',label:'Custom raster sources'}, {id:'local',label:'Local files'},
  {id:'wayback',label:'Wayback'}, {id:'commercial',label:'Commercial imagery'},
  {id:'cbers',label:'CBERS'}, {id:'maxar',label:'Maxar / Vantor Open Data'},
  {id:'esa-tpm',label:'ESA Third Party Missions'}, {id:'dlr',label:'DLR / EnMAP'},
  {id:'piesat',label:'PIESAT-1'}, {id:'land-cover',label:'Land cover'},
  {id:'hydrology',label:'Water and hydrology'}, {id:'population',label:'Population and settlements'},
  {id:'soil',label:'Soil'}, {id:'terrain',label:'Terrain and bathymetry'},
  {id:'climate',label:'Climate and precipitation'}, {id:'eog',label:'Nighttime lights'},
  {id:'open-vectors',label:'Open vector datasets'}, {id:'jaxa',label:'JAXA Earth'},
  {id:'china-data',label:'China scientific data portals'},
];

export const SOURCE_DIRECTORY = [
  ...[...PROVIDERS].sort((a,b)=>order(a)-order(b)).map(provider=>({id:provider.id,providerId:provider.id,series:provider.series,category:productCategory(provider),group:'products',status:originalsReleased(provider)?'ready':'auth',action:{kind:'provider',providerId:provider.id}})),
  entry('usgs-landsat','Landsat · USGS direct access','Official catalog and original-product downloads','USGS','landsat','landsat','imagery','products'),
  entry('modis-more','MODIS · Additional products','Additional NASA products and original HDF formats','NASA Earthdata','modis','modis','imagery','products'),
  entry('viirs-more','VIIRS · Additional products','Additional science and quality products','NASA Earthdata','viirs','viirs','imagery','products'),
  entry('nasa-more','NASA Earthdata · Additional products','Additional Earthdata product adapters','NASA Earthdata','nasa','nasa','imagery','products'),
  entry('srtm-gl3','SRTMGL3','Three-arc-second elevation product','NASA Earthdata','srtm','srtm','elevation','products'),
  entry('srtm-num','SRTM NUM','Elevation source and quality information','NASA Earthdata','srtm','srtm','elevation','products'),
  entry('nasadem','NASADEM','Reprocessed SRTM elevation products','NASA Earthdata','nasadem','nasadem','elevation','products'),
  entry('custom-stac-api','STAC API','Connect a public raster catalog and search its collections','Custom service','stac','custom-raster','imagery','catalogs',{kind:'stac',sourceType:'api'}),
  entry('static-stac','Static STAC catalog','Browse catalog directories and scan items by area and date','Custom service','stac','custom-raster','imagery','catalogs',{kind:'stac',sourceType:'catalog'}),
  entry('stac-item','STAC item','Read a single item and its declared original assets','Custom service','stac','custom-raster','imagery','catalogs',{kind:'stac',sourceType:'item'}),
  entry('direct-cog','COG / GeoTIFF URL','Connect an explicit public raster file URL','Custom source','raster','custom-raster','imagery','catalogs',{kind:'stac',sourceType:'raster'}),
  entry('wcs-service','WCS 2.0.1','Get a coverage subset on the service native grid','Custom service','wcs','custom-raster','imagery','catalogs',{kind:'wcs'}),
  entry('earth-search-more','Earth Search · Additional collections','Additional collections require product-specific adapters','Earth Search','catalog','earth-search','imagery','catalogs'),
  entry('planetary-more','Planetary Computer · Additional collections','Additional collections require product-specific adapters','Planetary Computer','catalog','planetary','imagery','catalogs'),
  entry('copernicus-more','Copernicus · Additional products','Additional collections, authorization and product formats','Copernicus Data Space','catalog','copernicus','imagery','catalogs'),
  entry('nasa-gibs','NASA GIBS','Date-aware rendered map imagery via WMS / WMTS','NASA GIBS','gibs','gibs','maps','maps',{kind:'map',protocol:'WMS',preset:'gibs'}),
  entry('wms-service','WMS','Save rendered imagery with coordinates and provenance','Custom service','wms','map-services','maps','maps',{kind:'map',protocol:'WMS'}),
  entry('wmts-service','WMTS','KVP / REST tiles and native-grid image assembly','Custom service','wmts','map-services','maps','maps',{kind:'map',protocol:'WMTS'}),
  entry('xyz-service','XYZ','Public PNG / JPEG tile templates in Web Mercator','Custom service','xyz','map-services','maps','maps',{kind:'map',protocol:'XYZ'}),
  entry('tms-service','TMS','Tile templates with rows counted from the bottom','Custom service','tms','map-services','maps','maps',{kind:'map',protocol:'TMS'}),
  entry('arcgis-map','ArcGIS MapServer / ImageServer','Save the public service rendered map for an area','ArcGIS REST','arcgis','map-services','maps','maps',{kind:'map',protocol:'ArcGIS'}),
  entry('wayback','Wayback','Historical imagery versions and authorized export','Esri','wayback','wayback','maps','maps'),
  entry('commercial-imagery','Jilin-1 / commercial imagery','Provider catalogs, orders and licensed 2D delivery','Authorized providers','commercial','commercial','imagery','maps'),
  entry('osm-overpass','OpenStreetMap / Overpass','Extract bounded features from a user-provided endpoint','OpenStreetMap','osm','osm','vectors','vectors',{kind:'vector',protocol:'Overpass'}),
  entry('ogc-features','OGC API Features','Discover collections and save complete bounded features','Custom service','vector','vector-services','vectors','vectors',{kind:'vector',protocol:'OGC'}),
  entry('wfs-service','WFS 2.0','Discover feature types and read GML / GeoJSON','Custom service','wfs','vector-services','vectors','vectors',{kind:'vector',protocol:'WFS2'}),
  entry('arcgis-features','ArcGIS Feature Service','Read public feature layers into managed GeoJSON','ArcGIS REST','arcgis','vector-services','vectors','vectors',{kind:'vector',protocol:'ArcGIS'}),
  entry('protomaps','Protomaps / PMTiles','Extract a bounded vector tile pyramid from a public archive','Public archives','pmtiles','protomaps','tiles','tiles',{kind:'tiles'}),
  entry('local-pmtiles','Local PMTiles','Open a local vector tile archive and retain an offline copy','Local file','pmtiles','local','tiles','tiles',{kind:'library',view:'tiles'}),
  entry('local-mbtiles','Local MBTiles','Open local vector or PNG / JPEG tile archives','Local file','mbtiles','local','tiles','tiles',{kind:'library',view:'tiles'}),
  entry('local-geojson','GeoJSON / Overpass JSON','Inspect local attributes and geometry in a 2D map','Local file','vector','local','vectors','local',{kind:'library',view:'vectors'}),
  entry('local-geopackage','GeoPackage','Read local feature layers and preserve the original database','Local file','gpkg','local','vectors','local',{kind:'library',view:'vectors'}),
  entry('local-shapefile','Shapefile / ZIP','Read the original geometry, DBF attributes and encoding','Local file','shp','local','vectors','local',{kind:'library',view:'vectors'}),
  entry('local-osm','OSM XML / PBF','Read bounded local OSM snapshots and original tags','Local file','osm','local','vectors','local',{kind:'library',view:'vectors'}),
  entry('local-raster','Local GeoTIFF / COG files','Standalone local raster import and registration','Local file','raster','local','imagery','local'),
  // Research candidates have no connector action until product-specific acceptance.
  ...openDataCandidates.map(source=>({...source,status:'planned'})),
].map(source=>({...source,group:productGroup(source)}));
