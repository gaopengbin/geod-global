# 全球一级行政区边界：当前可执行范围

选区弹窗现在捆绑 Natural Earth 1:1000 万 Admin-1 v5.1.1 的 4596 个一级行政区，涉及 251 个国家或地区代码。英文、中文及数据提供的本地名称进入搜索索引；选中结果后按国家读取 GeoJSON 几何，因此打开弹窗不会一次加载约 52 MB 的全部边界。国家轮廓仍来自独立的 1:5000 万 Admin-0 图层。少数小国在此 Admin-1 数据集中没有下级区划。

数据来自 [Natural Earth 官方 Admin-1 下载](https://www.naturalearthdata.com/downloads/10m-cultural-vectors/10m-admin-1-states-provinces/)，其[使用条款](https://www.naturalearthdata.com/about/terms-of-use/)将这些矢量数据列为公有领域。应用保留原始下载地址、版本、ZIP SHA-256、每个派生文件 SHA-256 与要素数，见[`manifest.json`](../prototype/public/basemaps/admin1-10m/manifest.json)。边界用于制图和产品交互，不作为法定国界或行政界线判断。

开发者可从[官方 ZIP](https://naturalearth.s3.amazonaws.com/10m_cultural/ne_10m_admin_1_states_provinces.zip)下载固定版本，运行 `python -m pip install -r requirements-dev.txt`，再运行 `python scripts/build-admin1-boundaries.py <下载的 ZIP 路径>`。构建器校验 ZIP SHA-256 和版本，生成索引、251 个按国家拆分的 GeoJSON 与清单。`npm run verify`核对清单与每个文件的哈希、数量和索引对应关系。

选择行政区多边形后，Earth Search 仍按其外接矩形检索；本地裁剪可把原始 GeoJSON 多边形交给 `geod-raster-recipe/v2`，在已下载的 Sentinel-2 SCL 栅格上进行遮罩。完整州界通常大于单次 800 万输出像元上限，需在配方中指定与边界相交的较小窗口。真实 1:1000 万加州边界与已下载 SCL 的旧金山湾窗口已执行，并用 Rasterio、PyProj、Shapely 对全部 912900 个像元独立核验，见[记录](../prototype/qa/global-admin1-clip-verification.json)。

4596 个边界均可索引，但其中 8 个不符合当前本地裁剪器的静态几何约束：2 个极区、5 个跨日期变更线或经度跨度过大、1 个超过 30000 个位置。界面在用户选择时提示并禁用相应多边形裁剪入口。其余边界仍须通过源文件、范围相交、NoData、像元数量和运行时预检；“索引可搜索”不代表有该区的源影像或可一次处理整个行政区。当前不支持 Admin-2、通用 RGB 或任意栅格裁剪。历史 1:5000 万 Admin-1 文件保留作旧版验收参照，不再作为选区的全球一级行政区来源。
