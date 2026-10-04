"""Read one actual Int16 index sample using GDAL, independent of the app."""
import argparse,json
import rasterio
from rasterio.windows import Window

parser=argparse.ArgumentParser(description=__doc__);parser.add_argument('file');parser.add_argument('x',type=float);parser.add_argument('y',type=float);args=parser.parse_args()
with rasterio.open(args.file) as ds:
    assert ds.dtypes==('int16',) and ds.nodata==-3000 and ds.scales==(.0001,) and ds.offsets==(0.,)
    assert ds.tags()['AREA_OR_POINT']=='Area'
    row,col=ds.index(args.x,args.y);assert 0<=col<ds.width and 0<=row<ds.height
    value=int(ds.read(1,window=Window(col,row,1,1))[0,0]);x,y=ds.xy(row,col)
    result={'value':value,'pixel':[col,row],'center':[x,y],'isNoData':value==-3000}
    if value!=-3000:result['indexValue']=value*.0001
    print(json.dumps(result))
