# USGS 3DEP NJ 1 m DEM：完整原件验证 — 2026-10-13

本次通过匿名 USGS National Map API 重新取得原始目录记录，完整下载了一份 **项目式 1 m 裸地 DEM** 并独立解码全部 100,240,144 个像元。新模块为 `usgs-3dep-1m`，只支持这一份明确核对的 NJ 瓦片。随后使用新构建程序完成真实原生目录连接、全件下载、原像元/NoData、PNG、落盘和关闭重开验收；浏览器地图界面由单独流程验证。

## 真实来源与范围

- [USGS 产品说明](https://www.usgs.gov/3d-elevation-program/about-3dep-products-services) 区分项目式 1 m、seamless 1 m 与 Current 1 arc-second；这些不是可互换的产品。
- 实际查询为 `https://tnmaccess.nationalmap.gov/api/v1/products`，参数 `datasets=Digital Elevation Model (DEM) 1 meter`（**没有 Current 后缀**）、请求原始 bbox、`max=100`、`offset=0`、`outputFormat=JSON`。
- 验证区域 `[-73.994,40.71,-73.99,40.714]` 的真实查询返回 3 个项目记录。新连接器只选择已核对的 `sourceId=6aac9d8b1ba49b6a17badefa`，其他项目不自动获得支持。用户区域必须实际与该原始记录 bbox 相交；不内置演示 AOI 或回退到其他区域。
- 记录标题：`USGS 1 Meter 18 x58y451 NJ_NE6County_B23`；原件为 [完整官方 GeoTIFF](https://prd-tnm.s3.amazonaws.com/StagedProducts/Elevation/1m/Projects/NJ_NE6County_B23/TIFF/USGS_1M_18_x58y451_NJ_NE6County_B23.tif)。
- [原始发布者 XML](https://thor-f5.er.usgs.gov/ngtoc/metadata/waf/elevation/1_meter/geotiff/NJ_NE6County_B23/USGS_1M_18_x58y451_NJ_NE6County_B23.xml) 的 source period 为 **2023-04-13 至 2025-04-09**；**2026-09-17 是发布日**，不合成一个单景观测 datetime。
- 原件与实际 TNM 记录声明大小均为 **45,328,865 B**；新一次 HTTP 200 全件下载用时约 74.375 秒，为本机当时网络观测值。
- SHA-256：`2d37a51d9de6e72a052bf128ea793671eeac9f4a417f70908c263b6940da39e8`。

## 原始数值与基准

| 特征 | 真实原件 |
| --- | --- |
| 栅格 | 10012 × 10012，单波段 Float32，1 × 1 m，512 像素块 |
| 横向参考 | EPSG:26918，NAD83 / UTM 18N；不能改标 WGS84 UTM 32618 |
| 高程参考 | NAVD88、米；依据实际 TNM record.body 与原始 XML.abstract，原 TIFF 没有显式 vertical EPSG key |
| 原始投影边界 | `[579993.9999588705,4499994.000000671,590005.9999588705,4510006.000000671]` |
| 嵌入 NoData | −999999；零和负高程均有效 |
| 完整独立解码 | 100,240,144 像元：17,215,330 有效、83,024,814 NoData；有效像元均有限数 |
| 有效原始范围 | −4.808831691741943 至 35.89037322998047 m |
| 保留方式 | 完整 10 km 瓦片与六像素边缘，不是 1 km 瓦片；不裁切、重采样或变换高程基准 |

典型原始像元：`(0,0)=32.44863510131836`，`(468,287)=−0.10568944364786148`（有效负高程），`(3877,0)=−999999`（NoData）。完整文件有大量 NoData，不能用 bbox 相交推定区域内每一点有裸地高程。

真实 TNM 地理 bbox 为 `[-74.05380579399997,40.645950318000075,-73.93411540899996,40.73705480600006]`。将原投影网格转换到地理参考后与目录 bbox 略有差异；同时保存两者，不修改原件或假装两份声明精确相等。

## 严格停止的另一条实际记录

同区域旧 `NJ_SdL5_2014` 记录声明 34,686,894 B，实际 HTTP Content-Length 为 34,682,947 B。严格大小检查停止，未将其作为完整软件成功原件，也未改写发布者记录或猜测替代大小。随后单独选择发布较新的已核对 NJ_NE6County_B23 原件，并重新完整下载建立了本报告证据。

## 新模块边界与验收产物

`crates/geod-runtime/src/datasets/usgs_3dep_1m.rs` 复用 `catalog-raster` 与 schema 2 `SourceProvenance`：归档完整真实 TNM JSON/hash、精确选择记录、原始文件 URL、请求 AOI、原 bbox 与产品声明；不是伪造发布者 STAC Item。只有原始目录完整且其中的已核对记录实际与用户区域相交才能生成源选择。文件、source identity、大小、出版时间、body 或其他原始声明被修改时会拒绝。

未改写的真实记录和查询分别保存在 `usgs-3dep-1m-reviewed-record.json` 与 `usgs-3dep-1m-reviewed-query.json`。本机完整原件、HTTP/XML 收据、独立解码与旧记录大小冲突位于 `.verification/usgs-1m-probe-20261013/`，不提交原始下载文件或凭据。

验收脚本明确绑定新构建 binary 的 SHA-256，流程为 fresh 私有数据目录、真实目录连接、全件下载/哈希、原投影网格、原像元/NoData、PNG、工程落盘、关闭与重开，重开时确认不新增下载任务。

## 实际原生验收

- 新程序 SHA-256：`60033499aba12269c0d1eb6e824c54bdabd14a0f08a2d4984bb64fb2bf3ae775`。
- 工程：`72ecf77b-ae81-4e03-9590-fb80b6e38d3a`；原始下载任务：`aa3a478c-d734-4649-8e86-e94a488b21c7`。
- Fresh 下载成功，45,328,865 B 与独立全件 SHA 匹配；下载轮询观测约 103.76 秒，未复用研究原件。
- 原生检查保留 EPSG:26918、10012² 原投影网格、Float32 与 −999999 NoData。6 个像元包含有效负值和 NoData，均与独立解码精确匹配。
- 实际生成并读取 768 × 768 PNG。该瓦片约 82.8% 是 NoData，灰度预览主要可见西北部有效裸地；这不是完整纽约城市 DEM 覆盖声明。
- 关闭私有内核再启动，snapshot、工程选择、检查结果和所有像元完全不变，下载任务总数仍为 1；没有重开后再传输。
- 第一次连接曾因本机系统 DNS 解析失败在创建下载任务前停止。只读官方域名 A 记录查询后系统解析恢复；未修改代理、DNS 配置或 hosts。第一失败报告独立保留，第二次 fresh 数据目录重新完成全部验收。

准确回执为本机 `.verification/usgs-1m-probe-20261013/native-acceptance.json`、`native-pixels.json`、`native-inspection.json` 与 `native-reopen.json`。验收 helper 已停止，没有操作用户已有工程或活动任务。
