"""Independent GDAL read for an actual UI-selected ancillary science pixel."""
import argparse,json
from datetime import date,timedelta
import rasterio
from rasterio.windows import Window
p=argparse.ArgumentParser();p.add_argument('file');p.add_argument('key');p.add_argument('x',type=float);p.add_argument('y',type=float);args=p.parse_args()
definitions={'vi_quality':('uint16',1,65535),'vi_reliability':('int8',1,-1),'vi_doy':('int16',1,-1),'vi_red':('int16',.0001,-1000),'vi_nir':('int16',.0001,-1000),'vi_blue':('int16',.0001,-1000),'vi_mir':('int16',.0001,-1000),'vi_view_zenith':('int16',.01,-10000),'vi_sun_zenith':('int16',.01,-10000),'vi_relative_azimuth':('int16',.01,-4000)}
dtype,scale,fill=definitions[args.key]
with rasterio.open(args.file) as ds:
 assert ds.dtypes==(dtype,) and ds.nodata==fill and ds.scales==(scale,) and ds.tags()['AREA_OR_POINT']=='Area'
 row,col=ds.index(args.x,args.y);assert 0<=col<ds.width and 0<=row<ds.height
 value=int(ds.read(1,window=Window(col,row,1,1))[0,0]);x,y=ds.xy(row,col)
 result={'value':value,'pixel':[col,row],'center':[x,y],'isNoData':value==fill}
 if value!=fill:
  if args.key=='vi_doy':result['date']=(date(int(ds.tags()['CALENDAR_YEAR']),1,1)+timedelta(days=value-1)).isoformat()
  if scale!=1:result['convertedValue']=value*scale
 print(json.dumps(result))
