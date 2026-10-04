# XYZ / TMS 地图影像

2026-10-04。**我的数据 → 地图影像 → 获取地图影像** 现支持配置公开 XYZ 和 TMS 瓦片地址，按当前探索范围保存渲染影像，在工作空间查看并导出坐标包。桌面、CLI 与调试服务共用原生执行核心和原有地图影像命令。

选择 XYZ 或 TMS，填写连接名称和路径中各含一次 `{z}`、`{x}`、`{y}` 的 HTTPS 模板，再设置瓦片尺寸、级别范围、地址级别偏移、PNG / JPEG 格式及署名说明。保存来源只是保存配置，取图时才验证服务响应；不会声称已经完成 Capabilities 发现。NASA GIBS 示例填入一个固定可视化日期的 MODIS Terra 地址，使用 256 像素、0–9 级、JPEG 和顶部行号。这是按该服务公开网格填写的示例，不是从任意地址自动推断网格。[NASA GIBS 访问说明](https://nasa-gibs.github.io/gibs-api-docs/access-basics/)

官方服务路径可包含正常编码的图层名称，例如 DLR 的 `eoc%3Abasemap@EPSG%3A3857@png`。前端和原生核心现一致接受这种名称，继续拒绝编码的路径分隔符、相对路径、模板结构符、控制字符、无效编码和双重编码。选择 TMS 后点击“填入 DLR 底图”，即可填入已实测的模板、256 像素 PNG、0–16 级及署名；保持底部行号协议，不自动连接或下载。DLR 真实地址的界面填写与原生取图均已通过。

## 网格与保存结果

本次支持固定的全球 **EPSG:3857 / WebMercator** 方形网格。瓦片尺寸可为 256 或 512 像素，逻辑级别为 0–24；每级的行列数为 `2^z`，全球范围为 `±π × 6378137` 米。512 像素意味着同级更细的像素间距，不会隐式改动 URL 级别。地址级别偏移可为 -2 至 +2，且所有配置级别的实际请求级别必须非负。

XYZ 的行号从顶部开始；TMS 请求从底部开始，URL 中的行号为 `2^z - 1 - logicalRow`。地址中的 `{z}` 使用逻辑级别加偏移；行列数仍按逻辑级别计算。不同服务的 TMS 级别起点可能不同，需按供应商网格配置；当前不自动读取 TileMap XML、不支持区域原点或其他投影。[TMS 说明](https://wiki.osgeo.org/wiki/Tile_Map_Service_Specification)、[GeoWebCache 行号说明](https://geowebcache.osgeo.org/docs/current/services/tms.html)

区域按 WGS84 记录，再扩展到所选网格的完整像素。保存结果从原始瓦片复制像素，不重采样；PNG 展开为 RGBA，JPEG 解码为 RGB 并补充不透明 alpha。输出保留原生米制坐标。地图与导出读取同一份保存影像、原生范围和坐标模型。多边形只记录几何，当前输出为外接矩形，不执行多边形遮罩。

来源记录显式保存协议、原模板、尺寸 / 格式 / 级别范围 / 偏移、CRS、矩阵集、逻辑级别、矩阵原点 / 比例尺 / 行列数、请求范围、实际原生范围和像素窗口。逻辑矩阵统一以左上角为原点；TMS 的底部请求方向单独由协议和逐瓦片实际 URL 记录，归档文件名仍使用逻辑顶部行号。配置与矩阵互相核对，防止改动记录后错位读取。

每块原瓦片保留实际 URL、逻辑行列号、字节数和 SHA-256。署名与使用说明按用户配置保存；配置 SHA-256 在共享记录中复用 `capabilitiesSha256` 字段，界面明确标作“配置 SHA-256”，不把它称作能力响应校验值。模板里固定的日期不被推断为单景拍摄时间，也不会新增未经服务声明的时间维度。

## 离线与导出

受管 PNG、来源记录与原始瓦片 ZIP 持久保存在独立应用数据目录。打开时检查本地 PNG；导出时额外核对原瓦片归档的逐文件校验值。ZIP 包含 `map.png`、原生坐标的像素中心文件 `map.pgw`、GDAL 可读取的 `map.png.aux.xml`、`source.json`、`source-tiles.zip`、说明与逐文件 `checksums.sha256`。

显示颜色是可视化渲染结果，不能当作温度、反射率或其他科学原值。访问公开服务也不等于取得全部再利用权。本入口拒绝 OpenStreetMap 标准瓦片主机的离线保存；其标准瓦片政策明确不允许离线下载，第三方或自建来源应按各自政策使用。[OSMF 瓦片政策](https://operations.osmfoundation.org/policies/tiles/)

## 实际验收

### NASA GIBS XYZ

隔离原生运行时实际取得 NASA GIBS 的两种公开 XYZ 渲染来源，日期 `2025-06-27`、区域 `[-125,35,-120,40]`：

| 图层 | 格式 / 级别 | 输出尺寸 | 原瓦片数 | 全部输出通道的独立核对 |
|---|---|---|---|---|
| MODIS Terra 地表温度白天可视化 | PNG / 6 | 228 × 288 | 4 | RGBA 完全一致，颜色不表示原始温度数值 |
| MODIS Terra 真彩色可视化 | JPEG / 7 | 456 × 575 | 9 | alpha 一致；两个 JPEG 解码器最大差 4 个色阶，原 JPEG 字节完全一致 |

界面另实际保存了当前探索区域的一张 85 × 74 真彩色影像，4 个原瓦片，独立 JPEG 解码最大差 3 个色阶。三份结果共 **334,154 像素、17 个原瓦片**。全部原瓦片由独立 HTTP 请求逐字节复核；Pillow 独立组装每个输出像素，GDAL / Rasterio 独立核对 CRS、尺寸、范围、像素变换及保存结果的全部通道。导出校验值与原来源记录一致。

真实关闭服务并重启后，三份影像的元数据、PNG 和 ZIP 字节一致。将隔离运行时代理改为拒绝连接的地址后，仍可完全离线检查、导出。错误瓦片尺寸、非公开地址、带查询凭据的模板和标准 OSM 瓦片主机均被拒绝，没有登记失败影像。

无头 Chromium 覆盖 1440 像素中文浅色和 1024 像素英文深色界面：网格配置、实际保存、真实缩略图、地图、来源与窄窗口布局通过，未报告脚本错误。全过程没有操作用户桌面。安装版 WebView 及保存对话框人工验收仍待进行。

复验脚本：[公开 XYZ](../scripts/verify-xyz-public.py)、[界面保存文件](../scripts/verify-xyz-ui-file.py)、[真实重启](../scripts/verify-xyz-restart.py)。具体记录见 [QA](../prototype/qa/xyz-public-verification.json)。前端 180 项逻辑与 173 项界面测试、原生核心 345 项、CLI 3 项、桌面 4 项通过；原生另有 4 项明确忽略。构建、隔离 / 契约 / 配方检查、格式与严格 Clippy 检查通过。

### DLR EOC Basemap TMS

实际读取 [DLR 官方 TileMap XML](https://tiles.geoservice.dlr.de/service/tms/1.0.0/eoc%3Abasemap@EPSG%3A3857@png)，独立确认全球 EPSG:3857、左下原点、256 像素 PNG 和 0–16 级网格；随后通过隔离原生运行时配置该模板并取得德国区域 `[10,47,13,50]` 的两份影像：

| 逻辑级别 | 输出尺寸 | 原瓦片数 | 独立核对 |
|---|---|---|---|
| 6 | 137 × 207 | 4 | 全部 28,359 个 RGBA 像素一致 |
| 7 | 274 × 413 | 6 | 全部 113,162 个 RGBA 像素一致 |

共 **10 块真实原瓦片、141,521 个输出像素**。每块原 PNG 都以独立 HTTP 请求逐字节复核；底部请求行号从官方原点和比例独立推算。Pillow 核对全部组装像素，GDAL / Rasterio 核对 CRS、范围、坐标变换和保存像素，导出 ZIP、原瓦片归档与全部校验值一致。

真实关闭并重启运行时后，将上游代理设为拒绝连接，两份元数据、PNG 和导出 ZIP 仍逐字节一致，没有任何上游请求。生产前端使用桌面 CSP 和实际原生接口，在 1440 像素英文浅色、1024 像素中文深色下验证两张缩略图、四次地图显示、底部行号来源信息和官方编码模板输入，未报告脚本错误、CSP 违规或外部请求。此次界面是离线只读验证；实际远端取图由原生运行时完成，不声称安装版 WebView 人工验收。

署名保存为 `Data © OpenStreetMap contributors and others; rendering © DLR/EOC`，来源及使用说明保留 [DLR 服务说明](https://geoservice.dlr.de/web/about)。这是渲染底图，不能当作卫星原波段。XML 仅为独立验收读取，产品仍需显式配置模板；512 像素和非零级别偏移仍只有合成协议测试。

复验脚本：[公开 TMS](../scripts/verify-tms-public.py)、[生产界面](../scripts/verify-tms-public-ui.mjs)、[汇总](../scripts/summarize-tms-public.py)。有界证据见 [TMS QA](../prototype/qa/tms-public-verification.json)。本轮相关原生地图服务 43 项、MCP 16 项、CLI 4 项、前端逻辑 231 项和地图服务界面 13 项通过，严格 Clippy 通过；这些是当前改动的验证范围，不替代上面的真实来源证据。

## 当前限制与后续

当前仅公开 HTTPS 443 路径模板，不接受查询参数、URL 凭据、主机占位符、分片子域或自动重定向。一次最多 16 瓦片、每边最多 2048 像素、单瓦片 4 MiB、原瓦片归档 64 MiB、结果 PNG 16 MiB；总获取时间 120 秒。仍是有界取图，尚未接入持久任务、取消 / 断点续取、大区域离线包、共享瓦片缓存、认证源或工程内组合处理。

其他投影 / 区域网格、TileMap XML 自动发现及历史商业图层仍按[完整接入状态](provider-integration-status.md)推进。WMTS REST、MVT / PMTiles / MBTiles 的独立接入证据也见该页；本次渲染影像适配不代表科学原文件或所有国际平台接入完成。

## CLI 请求结构

通过现有 `map-services connect` 的 JSON 文件参数传入：

```json
{
  "name": "NASA GIBS · MODIS Terra · 2025-06-27",
  "protocol": "XYZ",
  "url": "https://gibs.earthdata.nasa.gov/wmts/epsg3857/best/MODIS_Terra_CorrectedReflectance_TrueColor/default/2025-06-27/GoogleMapsCompatible_Level9/{z}/{y}/{x}.jpeg",
  "tileConfig": {
    "tileSize": 256, "minZoom": 0, "maxZoom": 9, "zoomOffset": 0,
    "format": "image/jpeg", "attribution": "NASA GIBS / MODIS Terra",
    "accessConstraints": "NASA GIBS visualization; dataset reuse terms apply."
  }
}
```

`map-images get` 使用返回的 `serviceId`、`layerName: "tiles"`、空样式、`time: null`、`tileMatrixSet: "WebMercator"` 和逻辑级别字符串。WGS84 `bounds`、`width` / `height` 必须匹配整像素窗口；界面自动计算，独立复验脚本也包含计算示例。TMS 使用相同请求结构，连接协议改为 `TMS` 并按服务的真实网格填写参数。
