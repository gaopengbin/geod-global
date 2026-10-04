# Copernicus DEM GLO-90

2026-10-03。探索页新增独立 GLO-90 入口，复用区域检索、目录分页、工程、原文件任务、本地地图、原始高程读取及持久化缩略图。无需账号的 AWS 公开分发与需要账号的 Copernicus Data Space 原产品入口分开处理。[AWS 数据说明](https://registry.opendata.aws/copernicus-dem/)

## 产品与网格

这是包含建筑物和植被的数字表面模型，采用 AWS 2021 公开版本。标称 90 米，纬向间距为 3 角秒；文件名的 `COG_30` 表示 3 角秒，不是 30 米。公开 COG 每瓦片 1,200 行，随纬度使用 1,200 / 800 / 600 / 400 / 240 / 120 列。经度间距从文件读取，不能按固定的 90 米或正方形网格渲染。[分发格式](https://copernicus-dem-90m.s3.amazonaws.com/readme.html)

公开目录使用 Earth Search `cop-dem-glo-90`。只接受与所选产品身份一致的 `s3://copernicus-dem-90m/<cell>/<cell>.tif`，转换到已审核的 HTTPS 桶；GLO-30 与 GLO-90 的产品编号、桶、行数及角度间距分别验证。工程保存固定原文件身份，返回探索时恢复同一数据源和区域。

原文件与输出均保留单波段 Float32、经纬度 EPSG:4326、PixelIsPoint 网格和原始高程值。米制高程参考 EGM2008（EPSG:3855），来自固定产品规格；原文件如提供垂直参考或单位标签，必须与该产品一致。缺失垂直标签不冒充文件声明。灰度百分位拉伸仅用于显示，点击地图读取原始分辨率像元。[产品规格](https://dataspace.copernicus.eu/explore-data/data-collections/copernicus-contributing-missions/collections-description/COP-DEM)

原目录元数据保留。当前集合的 `item_assets.data.raster:bands.spatial_resolution` 声明为 30，与 GLO-90 的产品级 90 米标称分辨率不一致；本次实际检索的两个条目均声明 90，实际 GeoTIFF 网格也已独立检查。软件按固定产品显示标称分辨率，科学取值按实际文件网格处理，不覆写原目录声明。[原始集合](https://earth-search.aws.element84.com/v1/collections/cop-dem-glo-90)

## 处理与验收

已支持单瓦片工程区域裁剪、同产品同网格瓦片的拼接及多边形掩膜。不重投影、不重采样，不混合 GLO-30 与 GLO-90。有效的零、负数及小数保留原始 Float32 位；空隙及多边形外像元写为 NaN NoData。输出写入 Point、水平 EPSG:4326、垂直 EPSG:3855 和米制 GeoKey，并附原文件校验与来源记录。

[实际验收记录](../prototype/qa/glo90-verification.json)记录三份匿名原文件：N51W001 / N51E000 为 800 × 1,200，N37W123 为 1,200 × 1,200，合计 9,949,780 字节。原生完整下载、SHA-256、15 个原值探针与独立 Rasterio/GDAL 一致。三种实际处理结果共 39,887 个输出像元由 Rasterio、NumPy 和 tifffile 独立核对，包括 13,482 个多边形外 NaN 像元；所有有效 Float32 位与原文件一致。

无头浏览器覆盖 1440 中文浅色和 1024 英文深色的工程返回探索、筛选、继续加入工程并复用下载、真实缩略图及原文件 / 两种处理结果的地图取值。6 次界面地图取值独立核对一致，无页面错误或水平溢出。重启后 6 份源文件及成果的校验、元数据、缩略图内容和 15 个原始像元保持一致。目录界面采用实际公开响应回放，本地请求来自隔离的真实 Rust 运行时；没有操作用户桌面，安装版 WebView 尚未人工验收。

仍待接入：不同网格重采样、重投影、坡度 / 山体阴影等地形分析及其他 DEM 产品。公共陆地覆盖不代表海洋瓦片，空结果不补造高程；免费公开分发不等于公有领域，使用和导出保留产品来源及[许可链接](https://registry.opendata.aws/copernicus-dem/)。
