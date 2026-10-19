# USGS 3DEP Current 原件接入候选核验

2026-10-12。推荐新增 `usgs-3dep` / `elevation`，通过 USGS TNM 的真实 Current 1 arc-second 目录取得公开 AWS GeoTIFF。研究阶段已完整下载并逐块解码；软件原生接线、重开及浏览器验收另行进行，不能用本报告替代。

[机器可读证据](next-channels-20261012-usgs-3dep.json) 保留实际 TNM JSON、响应 SHA、源记录、完整原件收据、独立像元检查与 GeoKeys。原件和未改写的 HTTP/XML 证据在 `.verification/next-channels-20261012-usgs/`，不包含账号凭据。

## 最可行新增渠道

[USGS 官方下载页](https://www.usgs.gov/the-national-map-data-delivery/gis-data-download) 同时链接 TNM Access API 和公开 `StagedProducts`，无需 ERS。实际 `datasets` 目录将该层标为 `addCurrent=true`；查询参数必须是 `National Elevation Dataset (NED) 1 arc-second Current`，而非猜测产品名。实际接口为 `https://tnmaccess.nationalmap.gov/api/v1/products`，公开只读 GET，参数 `datasets`、`bbox`、`max=100`、`offset=0`、`outputFormat=JSON`。参见 [TNM API](https://tnmaccess.nationalmap.gov/api/v1/docs)。这不是 STAC，应保存准确原始目录字节及其记录，不能伪造发布者 Item。

小范围 `[-73.994,40.71,-73.99,40.714]` 的真实 API 返回 2 条 Current 记录；其中 n41w075 的声明范围不与请求相交，按原始 bbox 本地复核后仅选择 n41w074。元数据几何匹配不能代替真实像元覆盖。响应超过 100、页不完整、匹配超过 8 件或未知大小均明确拒绝，不静默截断或猜测。

本次优选记录为 `6aac9d7f1ba49b6a17baded8`，题名 `USGS 1 Arc Second n41w074 20260917`，发布日 2026-09-17。原件：

- [公开完整 GeoTIFF](https://prd-tnm.s3.amazonaws.com/StagedProducts/Elevation/1/TIFF/historical/n41w074/USGS_1_n41w074_20260917.tif)
- [独立发布者 XML](https://thor-f5.er.usgs.gov/ngtoc/metadata/waf/elevation/1_arc-second/geotiff/NJ_NE6County_B23/USGS_1_n41w074_20260917.xml)
- [ScienceBase 记录](https://www.sciencebase.gov/catalog/item/6aac9d7f1ba49b6a17baded8)

该 URL 的目录名含 `historical`，但实际 Current API 精确返回此版本文件；不能单凭路径名改换其他 URL。实际文件和目录大小均为 20,582,281 字节；SHA-256 `4f23a1a0801a74b1d1e9b4e83c09a41ee371010cbdca7bff0a1a92c05f0dc3dd`。

## 原件与科学边界

优选原件为 3612 × 3612、Float32、LZW/COG/512 像素块，EPSG:4269（NAD83）。保留原始边界 `[-74.00166666718229,39.99833333341752,-72.99833333291633,41.00166666678416]` 和六像素重叠，完整下载，不按 AOI 裁切。全网格解码 13,046,544 像元：13,024,920 有效、21,624 嵌入 NoData（−999999），范围 −17.564453125 至 168.40380859375。零和负高程均保留为有效样本。例如 `(column=356,row=0)` 为 −0.016024990007281303，`(8,3606)` 为原始 NoData。

发布者 XML 明确该 CONUS 瓦片的垂直基准为 NAVD88、单位米；源时间范围 1931-01-01 至 2025-04-09，2026 是发布年份。1 arc-second 约 30 米，属于融合的裸地高程，不是 2026 卫星单景。其他地区垂直基准可能不同，应保留实际元数据。[3DEP 产品说明](https://www.usgs.gov/3d-elevation-program/about-3dep-products-services) 指明数据免费；实际 XML/目录声明 public domain。

1 米项目原件也已测试，但初版不纳入 Current 接口：`Digital Elevation Model (DEM) 1 meter Current` 实查为 0 条，官方该产品项未声明 `addCurrent`。独立测试文件 `USGS_1M_18_x58y451_NJ_NE6County_B23.tif` 45,328,865 字节、10012 × 10012、EPSG:26918，不能冒认 WGS84 UTM。100,240,144 像元全部解码，17,215,330 有效、83,024,814 NoData，范围 −4.808831691741943 至 35.89037322998047；SHA、实际网格及 XML 源期在 JSON。原始投影转换后的边界与目录 bbox 相差约 0.00007 度，应同时保留，不能要求二者伪装一致。

历史 2021 版本存在实际大小差异：API 20,496,942 字节，HTTP/完整文件为 20,761,575。严格大小校验首次停止；另行重新完整下载并独立解码才建立研究收据。旧目录大小不得作为 Current 下载的猜测值，也不应悄悄忽略差异。

## NOAA 补充核验

`etopo` 已存在当前产品目录，不计为本次新增唯一产品。[官方 ETOPO 页](https://www.ncei.noaa.gov/products/etopo-global-relief-model) 链接固定原件目录。实际 `ETOPO_2022_v1_15s_N00E030_surface.tif` 为 25,179,967 字节、3600 × 3600、Float32，完整 12,960,000 像元全解码；SHA-256 `975c87680ad0dffb759cfa6c85385323b5dc94ab23e9dffc95483a5f1d944bb7`。边界 `[30,-15,45,0]`，有效范围 −4616 至 5828.8203125，无实际 NoData 像元，不能把负海底高程当缺失。

实际 GeoKeys 是水平 EPSG:4326 与垂直 EPSG:3855；Rasterio 组合识别为 EPSG:9518。原文件采用 EGM2008 高程，显示地图不等于垂直转换。15 角秒版本为固定 15° 瓦片融合模型，发布版本 2022 v1，目录许可 CC0-1.0；保留其引用。[官方元数据](https://data.noaa.gov/metaview/page?header=none&view=getDataView&xml=NOAA/NESDIS/NGDC/MGG/DEM//iso/xml/etopo_2022.xml) 和 [用户指南](https://www.ngdc.noaa.gov/mgg/global/relief/ETOPO2022/docs/1.2%20ETOPO%202022%20User%20Guide.pdf) 是科学语义依据，非在线裁剪导出的替代原件。

本次所有原件均为 HTTP 200 完整响应，没有 Range 请求、缩略图、授权账号、临时导出或重采样作为验收替代。观测到的下载时间是当时该网络的样本，不能承诺所有用户或渠道均达到相同速度。
