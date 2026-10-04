"""Bind actual downloads, independent scientific checks, UI and development build.
This hash/receipt gate does not stand in for real files or a native window test.
"""
import argparse,copy,hashlib,json,shutil
from pathlib import Path
from datetime import datetime,timezone
p=argparse.ArgumentParser();p.add_argument('source',type=Path);p.add_argument('native',type=Path);p.add_argument('cache',type=Path);p.add_argument('ui',type=Path);p.add_argument('explore',type=Path);p.add_argument('compatibility',type=Path);p.add_argument('root',type=Path);args=p.parse_args()
workspace=Path.cwd().resolve();root=args.root.resolve();assert root.parent==workspace/'.verification' and root.name.startswith('modis-science-')
def sha(path):
 h=hashlib.sha256()
 with Path(path).open('rb') as f:
  for block in iter(lambda:f.read(1024*1024),b''):h.update(block)
 return h.hexdigest()
def require(value,message):
 if not value:raise ValueError(message)
def local(path):return Path(path.removeprefix('\\\\?\\')).resolve()
def manifest(directory):return sorted([{'path':p.relative_to(directory).as_posix(),'sha256':sha(p)} for p in directory.rglob('*') if p.is_file()],key=lambda e:e['path'])
definitions={'vi_quality':('UInt16',1,65535),'vi_reliability':('Int8',1,-1),'vi_doy':('Int16',1,-1),'vi_red':('Int16',.0001,-1000),'vi_nir':('Int16',.0001,-1000),'vi_blue':('Int16',.0001,-1000),'vi_mir':('Int16',.0001,-1000),'vi_view_zenith':('Int16',.01,-10000),'vi_sun_zenith':('Int16',.01,-10000),'vi_relative_azimuth':('Int16',.01,-4000)}
paths={'source':args.source/'source-verification.json','native':args.native/'verification.json','mcp':args.native/'mcp-verification.json','cache':args.cache/'cache-verification.json','ui':args.ui/'ui/verification.json','explore':args.explore/'verification.json','compatibility':args.compatibility/'verification.json'}
bundle={key:json.loads(path.read_text(encoding='utf-8')) for key,path in paths.items()}
def validate(b):
 require(all(r['status']=='passed' for r in b.values()),'Every stage must pass')
 s,n,m,c,u,e,v=[b[k] for k in ['source','native','mcp','cache','ui','explore','compatibility']];current=n['nativeBinarySha256']
 require(all(b[k]['nativeBinarySha256']==current for k in b if k!='source'),'Current native identity differs')
 require(len(s['cases'])==len(n['originals'])==30 and len(n['outputs'])==40,'Exactly thirty original downloads and forty verified outputs required')
 originals={r['job']['id']:r['job'] for r in n['originals']}
 require(originals.keys()=={r['job']['id'] for r in s['cases']},'Original job IDs differ')
 for r in s['cases']:
  j=r['job'];require(j['status']=='succeeded' and j['settled'],'Original transfer not settled')
  require(all(j[k]==originals[j['id']][k] for k in ['assetKey','itemId','href','sha256','bytesDownloaded']),'Original identity or hash differs')
 require({r['name'] for r in n['cases']}=={'single','mosaic','polygon','temporal'},'All geometry and temporal cases required')
 for r in n['originals']+n['outputs']:
  key=r['job']['assetKey'];info=r['metadata'];dtype,scale,fill=definitions[key]
  require(info['dataType']==dtype and info['nodata']==fill and info['science']['band']==key and info['science']['scale']==scale and info['science']['countsFullResolution'] is False,'Scientific type, calibration or sampled statistics differ')
  require('vegetation' not in info and 'reflectance' not in info,'Scientific metadata families mixed')
  if key=='vi_doy':require(info['science']['calendarYear']==2025,'Actual observation year differs')
  require(r['pngPixelsCompared']==info['previewWidth']*info['previewHeight']+r['thumbnail']['width']*r['thumbnail']['height'],'Incomplete image comparison')
  if r in n['outputs']:require(r['dnPixelsCompared']==info['width']*info['height'],'Incomplete output comparison')
 require(n['dnPixelsRead']==691200000 and n['outputDnPixelsCompared']==552960,'Independent comparison count differs')
 require(n['offlineRestart']['persistentThumbnailsReused']==70 and n['offlineRestart']['cacheFilesNotRegenerated'],'Original restart missing')
 temporal={r['job']['assetKey']:r for r in n['outputs'] if r['case']=='temporal'}
 for key,count in [('vi_reliability',71),('vi_doy',104),('vi_red',104)]:require(next(v for k,v in temporal[key]['winnerCounts'].items() if k.startswith('MYD13Q1'))==count,'Real older valid-source fallback missing')
 require(len(n['negativeControls'])==33,'Source binding refusal controls missing')
 require(c['originalsAbsentDuringColdAndRestartReads']==30 and c['derivedFilesRestored']==c['persistentEntriesNotRegenerated']==40,'Parent-free restart or persistence missing')
 require(len(c['corruptedCacheRebuiltWithoutParents'])==len(c['changedDerivedRejectsCachedPreview'])==5,'Cache corruption controls missing')
 require(m['pixelsCompared']==sum(len(r['pixels']) for r in n['originals']+n['outputs'])*2 and len(m['cases'])==140,'MCP read count differs')
 require(m['readOnlyWriteDeniedInBothModes'] and m['disconnectedAdapterRejected'] and m['protocolOnlyStdoutAndCleanExit'],'MCP transport controls missing')
 require(len(u['cases'])==len(u['createdByUi'])==10 and {r['key'] for r in u['cases']}==set(definitions),'Ten actual UI layer workflows required')
 for r in u['createdByUi']:
  reference=next(v for v in n['outputs'] if v['case']==r['case'] and v['job']['assetKey']==r['key'])
  require(r['sha256']==reference['job']['sha256'] and all(r[k] for k in ['sourcePinsIdentical','outputBytesIdentical','planIdentical','displayIdentical']),'UI output does not match independently checked output')
 require({r['width'] for r in u['cases']}=={900,1024,1440} and {r['locale'] for r in u['cases']}=={'en','zh-CN'} and {r['theme'] for r in u['cases']}=={'light','dark'},'UI layout coverage missing')
 require(all(r['paintedRaster'] and r['thumbnailsFillUniformCards'] and r['sampledScienceMetadataVisible'] for r in u['cases']) and not u['errors'] and not u['remoteRequests'],'UI raster/layout errors')
 require(e['allScienceDownloadChoicesVisible'] and len(e['restoredScienceLayers'])==10 and e['returnToSameProject'] and e['createdOriginalDownloads']==0 and len(e['liveCatalogRequests'])==1 and e['providerPreviews'],'Actual read-only discovery required')
 require(all(b[k]['nativeWindowTested'] is False for k in ['native','cache','ui','explore']),'Native-window evidence boundary differs')
 require(v['dnPixelsRead']==138240000 and v['outputDnPixelsCompared']==99812,'Old index regression incomplete')
validate(bundle);hashes={k:sha(p) for k,p in paths.items()};s,n,m,c,u,e,v=[bundle[k] for k in ['source','native','mcp','cache','ui','explore','compatibility']]
require(n['sourceReceiptSha256']==hashes['source'],'Source receipt binding differs')
require(m['nativeReceiptSha256']==c['sourceReceiptSha256']==u['sourceReceiptSha256']==hashes['native'],'Local receipt binding differs')
require(e['rendererReceipt']['sha256']==hashes['ui'],'Discovery renderer differs')
require(sha(workspace/'target/debug/geod-runtime.exe')=='3b0e845ebea363f154644db9e75f0fff1f852b69201eb2bcbae85174523ea03b','User runtime changed')
native=args.native/f"runtime-{n['nativeBinarySha256'][:16]}.exe";require(sha(native)==n['nativeBinarySha256'],'Frozen runtime differs')
for r in n['originals']+n['outputs']+v['originals']+v['outputs']:
 j=r['job'];require(sha(local(j['outputPath']))==j['sha256'],'Original or output file changed')
 if j.get('manifestPath'):require(local(j['manifestPath']).is_file(),'Provenance missing')
previous=json.loads((workspace/'prototype/qa/modis-vegetation-verification.json').read_text(encoding='utf-8'))
old={(r['case'],r['assetKey']):r['sha256'] for r in previous['native']['outputs']}
require(all(old[(r['case'],r['job']['assetKey'])]==r['job']['sha256'] for r in v['outputs']),'Old NDVI/EVI output bytes changed')
for r in e['liveCatalogRequests']+e['providerPreviews']+e['previewFailures']:require(sha(local(r['path']))==r['sha256'],'Live remote response capture changed')
accepted_files=sorted(u['rendererFiles'],key=lambda r:r['path']);require(manifest(args.ui/'renderer')==manifest(workspace/'prototype/dist')==accepted_files,'Accepted renderer changed')
require(u['csp']==json.loads((workspace/'src-tauri/tauri.conf.json').read_text(encoding='utf-8'))['app']['security']['csp'],'Desktop CSP changed')
desktop=workspace/'.verification/naip-native-target/debug/geod-global-desktop.exe';desktop_sha=sha(desktop)
snapshot=workspace/'.verification/renderer-modis-science-accepted-20261004';snapshot.mkdir(exist_ok=False);shutil.copytree(args.ui/'renderer',snapshot/'renderer');frozen=snapshot/f'desktop-{desktop_sha[:16]}.exe';shutil.copy2(desktop,frozen)
require(sha(frozen)==desktop_sha and manifest(snapshot/'renderer')==accepted_files,'Frozen development artifacts differ')
root.mkdir(exist_ok=False);(root/'evidence').mkdir();evidence={}
for k,p in paths.items():
 saved=root/'evidence'/f'{k}-{hashes[k]}.json';shutil.copy2(p,saved);require(sha(saved)==hashes[k],'Frozen receipt differs');evidence[k]={'snapshot':str(saved),'sha256':hashes[k]}
controls=[]
for name,change in [
 ('failed-stage',lambda b:b['source'].update(status='failed')),
 ('changed-original',lambda b:b['native']['originals'][0]['job'].update(sha256='0'*64)),
 ('wrong-signed-type',lambda b:b['native']['originals'][3]['metadata'].update(dataType='UInt8')),
 ('wrong-angle-scale',lambda b:next(r for r in b['native']['outputs'] if r['job']['assetKey']=='vi_view_zenith')['metadata']['science'].update(scale=.1)),
 ('false-full-resolution-counts',lambda b:b['native']['outputs'][0]['metadata']['science'].update(countsFullResolution=True)),
 ('wrong-observation-year',lambda b:next(r for r in b['native']['outputs'] if r['job']['assetKey']=='vi_doy')['metadata']['science'].update(calendarYear=2024)),
 ('partial-output',lambda b:b['native']['outputs'][0].update(dnPixelsCompared=0)),
 ('false-native-window',lambda b:b['ui'].update(nativeWindowTested=True)),
 ('unmatched-ui-output',lambda b:b['ui']['createdByUi'][0].update(sha256='0'*64)),
 ('parents-present',lambda b:b['cache'].update(originalsAbsentDuringColdAndRestartReads=0)),
 ('inflated-mcp',lambda b:b['mcp'].update(pixelsCompared=745)),
 ('discovery-as-download',lambda b:b['explore'].update(createdOriginalDownloads=30)),
]:
 amended=copy.deepcopy(bundle);change(amended)
 try:validate(amended)
 except (ValueError,KeyError,TypeError):controls.append(name)
 else:raise AssertionError('Invalid receipt accepted: '+name)
gate={'status':'passed','refusedControls':controls,'canonicalRuntimeUnchanged':True,'legacyNdviEviBytesUnchanged':True,'scope':'Hash and receipt binding only; no new download or native window acceptance'}
(root/'gate.json').write_text(json.dumps(gate,indent=2)+'\n',encoding='utf-8');evidence['gate']={'snapshot':str(root/'gate.json'),'sha256':sha(root/'gate.json')}
summary={'schema':'geod-modis-science-verification/v1','status':'passed','checkedAt':datetime.now(timezone.utc).isoformat(),'scope':'Ten ancillary MOD13Q1/MYD13Q1 v061 Planetary Computer science COGs; previous NDVI/EVI acceptance retained','definition':'https://lpdaac.usgs.gov/documents/621/MOD13_User_Guide_V61.pdf','evidence':evidence,
 'nativeBinary':{'path':str(native.resolve()),'sha256':n['nativeBinarySha256']},'sources':{'actualOriginalDownloads':30,'totalBytes':sum(r['job']['bytesDownloaded'] for r in n['originals']),'initialDownloadBinarySha256':s['nativeBinarySha256'],'reinspectedWithCurrentBinary':True,'originals':[{k:r['job'][k] for k in ['id','itemId','assetKey','href','sha256','bytesDownloaded']} for r in n['originals']]},
 'native':{'originalDnRead':n['dnPixelsRead'],'outputDnCompared':n['outputDnPixelsCompared'],'previewAndThumbnailRgbaPixelsCompared':n['pngPixelsCompared'],'independentRawPointsCompared':sum(len(r['pixels']) for r in n['originals']+n['outputs']),'cases':['single','mosaic','polygon-with-hole','temporal-fallback'],'outputs':[{'id':r['job']['id'],'case':r['case'],'assetKey':r['job']['assetKey'],'sha256':r['job']['sha256'],'width':r['metadata']['width'],'height':r['metadata']['height'],'allDnCompared':r['dnPixelsCompared'],'winnerCounts':r['winnerCounts'],'maskedPixels':r['maskedPixels']} for r in n['outputs']],'refusedSourceBindingControls':n['negativeControls'],'offlineRestart':n['offlineRestart']},
 'mcp':{'modes':['direct','loopback adapter'],'rawPointsCompared':m['pixelsCompared'],'writeDenied':True,'disconnectedAdapterRejected':True},'cache':c,
 'ui':{'cases':[{k:r[k] for k in ['case','key','width','locale','theme','paintedRaster','pixelInteraction','sampledScienceMetadataVisible','thumbnailsFillUniformCards','cardCount','cardHeight']} for r in u['cases']],'actualNativeProcessingButtons':10,'liveCatalogItems':e['liveCatalogRequests'][0]['features'],'providerPreviews':len(e['providerPreviews']),'allTwelveChoicesAndSameProjectReturn':True,'nativeWindowTested':False,'usedUserDesktop':False},
 'rendererSnapshot':{'root':str(snapshot),'files':accepted_files,'desktop':{'path':str(frozen),'bytes':frozen.stat().st_size,'sha256':desktop_sha}},'summaryControls':gate,
 'boundaries':['Converted science COGs do not include full NASA HDF.','Each layer mosaics independently by latest valid DN, not a coupled quality-screened observation. No quality mask is applied.','Source HDF divisor calibration is converted to multiplicative values; original DN and sample types remain unchanged.','Observation days use the source calendar year; cross-year day mosaics are refused.','Counts describe sampled preview pixels, not full-resolution quality statistics.','No reprojection or resampling. Nominal 250 m differs from actual sinusoidal PixelIsArea spacing ~231.656358264 m.','Production renderer and native API checks do not accept an installed or native WebView window.','Protected NASA/Copernicus original downloads still await account authorization. No installer or publication.']}
destination=workspace/'prototype/qa/modis-science-verification.json';require(not destination.exists(),'Preserve existing public receipt');destination.write_text(json.dumps(summary,indent=2,ensure_ascii=False)+'\n',encoding='utf-8');(root/'summary.json').write_text(json.dumps(summary,indent=2,ensure_ascii=False)+'\n',encoding='utf-8')
print(json.dumps({'status':'passed','sourceBytes':summary['sources']['totalBytes'],'desktopBytes':frozen.stat().st_size,'rendererFiles':len(accepted_files),'refusedControls':len(controls),'publicReceipt':str(destination)}))
