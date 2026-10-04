# 国际公开影像数据源

2026-10-04，桌面版探索页支持十五个数据源入口，共用区域、目录分页、选择与工程流程。单景卫星光学影像提供 UTC 日期和整景云量筛选；雷达按日期、升降轨与极化检索，MODIS / VIIRS 按区域和合成时段检索，NAIP 航空影像按区域和日期检索，高程瓦片按区域检索；这些产品不提供光学整景云量筛选。可用能力按具体产品区分；一个平台的目录接入不代表其所有产品均可下载或处理。

[完整规划与剩余接入项](provider-integration-status.md)区分已有产品、待账号验收的能力和尚未实现的公开数据适配。

[探索页公开来源预览](explore-public-previews.md)说明九个公开入口的具体预览方式、固定显示参数、临时错误重试，以及 NAIP 历史目录的修复范围；地图预览与完整原文件验收分别记录。

本地文件另有“矢量文件”入口：支持 WGS84 GeoJSON 与完整几何 Overpass JSON 的引用 / 明确复制、地图属性和导出。真实 247 个 OSM 要素已独立验收，详见[本地矢量与边界](vector-local-data.md)。同页另提供用户配置服务的 [OSM Overpass 小范围提取](osm-overpass.md)；不预设公共实例，不代表 PMTiles 或大区域批量工作流完成。

| 数据源 | 目录与缩略图 | 地理配准地图 | 原文件与工程处理 |
|---|---|---|---|
| MODIS NDVI / EVI · Planetary Computer | 实时 MOD13Q1/MYD13Q1 v061 目录、Terra / Aqua、完整 16 天合成时段和供应商浏览图；无整景云量筛选 | 探索页自动在线预览 NDVI / EVI，可切换指数；下载后显示 Int16 正弦投影指数地图，保留原始 DN 与 0.0001 换算，NoData 透明，固定色带只影响显示 | NDVI/EVI 完整 COG 下载、工程恢复、持久化缩略图、同网格裁剪 / 拼接已接入；六份实际原件、八份成果全部 DN、真实较旧景补齐、界面、MCP 和无父级文件的离线缓存通过，见[植被指数验收](modis-vegetation-indices.md)。只含十二个科学图层中的两层；完整 HDF、其余图层、质量筛选与重投影待接入 |
| Sentinel-1 IW RTC · Planetary Computer | 实时 IW 目录、升降轨、VV / VH / HH / HV 极化及分页筛选；不提供云量筛选 | 探索页自动在线预览当前景的极化，可切换 VV / VH / HH / HV，按 −30 至 0 dB 显示；下载后按原始 UTM 网格显示 Float32 gamma0，并返回原始线性值 | 工程原文件下载、持久化缩略图、来源与像元检查、工程裁剪和对齐网格拼接已接入；既有两份 VV 原件及三份成果验收保留。新增 VH / HH / HV 六份完整原件、九份成果、210 次 MCP 原值读取、九个实际处理按钮及十五份文件 / 缓存离线恢复通过，见[极化矩阵](radar-polarizations.md)、[雷达读取](radar-inspection.md)与[处理验收](radar-processing.md)。跨网格处理、散斑滤波、其他模式及原始 GRD / SLC 待接入 |
| NASA/NOAA VIIRS · NOAA-21 / NOAA-20 / Suomi-NPP | 三个 09A1 v002 实时 CMR 目录、完整 8 天时段、公开浏览图；无整景云量筛选 | 探索页显示瓦片范围与列表预览；本地准备 M5/M4/M3 Int16 GeoTIFF、原始像元、RGB 和持久化科学缩略图已接入 | 共用 Earthdata 授权入口，完整 HDF5 下载适配、工程恢复及按波段区域处理已接入；合成文件独立验证全值及单景裁剪，生产原件、专项多景拼接与其他科学 / QA 层仍待验收 / 实现，见[处理与边界](viirs-processing.md) |
| MODIS · Planetary Computer | 实时 MOD/MYD09A1 v061 公开目录、合成时段与瓦片范围 | 探索页自动在线 RGB，固定波段 1 / 4 / 3 与反射率显示范围；下载后支持正弦投影反射率及 QC / State 本地显示、原值 / 位解码、工程处理与质量选择 | 三波段、科学 RGB 和原始质量验收保留；全部五类实际文件、单景 / 多景 / 带孔裁剪共 15 份成果、缓存复用及 MCP 通过，见[五类文件处理](modis-quality-processing.md)和[质量说明](modis-quality.md)。新增[同景 RGB 筛选](modis-rgb-quality-mask.md)，十二份真实成果的全部 DN、预览、离线缓存和交付通过。新增[多景质量选择](modis-coupled-rgb.md)，十二份真实成果、云区回退、界面、MCP 和离线缓存通过。PC 转换 COG 不冒充 NASA 原始 HDF；其他产品质量规则及重投影待接入 |
| NASA Earthdata · SRTMGL1 v003 | 实时 CMR-STAC 目录和公开高程浏览预览；无需账号搜索；无云量与逐景日期筛选 | 探索页显示瓦片范围；下载后的本地 Int16 / EGM96 灰度地图与原值读取已验证合成 HGT | 共用 Earthdata 桌面授权入口，原生 HGT ZIP 下载适配、工程恢复与持久化缩略图已接入；未进行实际授权原文件下载；工程裁剪 / 拼接保留 Int16、EGM96 和共享边缘 Point 网格，合成输入已在实际运行时和独立 GDAL 验证，见[SRTM 处理](srtm-processing.md) |
| NAIP · Planetary Computer | 已接入美国航空影像目录与 RGB 预览；无云量筛选 | 选景并加载后显示原始四通道 COG 的在线 RGB，兼容实际 2016 年 h / .6 历史条目；下载后本地自然色 / 红外假彩色 / 近红外显示与原始 RGB / NIR 取值 | 0.3 / 0.6 / 1 米真实四通道 COG、NAD83 UTM 网格、完整哈希、原始 RGB / NIR 与持久化缩略图已核对；原件、工程结果、离线重启、MCP 和实际处理按钮通过，见[NAIP 验收](naip-inspection.md)与[分辨率变体](naip-resolution-variants.md)。三种分辨率的新增显示共 36 张预览、十二组实际地图切换、36 个独立原值点、48 次界面原值读取和十二张自然色缓存恢复通过，见[显示方式](naip-display-views.md)；多光谱分析及通用重投影仍待接入 |
| Copernicus DEM GLO-30 Public | 已接入高程瓦片目录；下载后生成持久化缩略图 | 探索页自动读取公开原始 COG 显示高程，校正 Point 半像元；下载后显示本地 Float32 灰度高程 | 原始公开瓦片下载、工程恢复、原始高程取值、同网格工程裁剪 / 拼接与缩略图缓存已用真实文件验收；不同间距重采样、重投影与地形分析未接入 |
| Copernicus DEM GLO-90 | 实时高程瓦片目录；下载后生成持久化缩略图 | 探索页自动读取公开原始 COG 显示高程，校正 Point 半像元；下载后保留实际度制网格与 Float32 高程 | 真实原文件、同网格工程处理、独立像元与离线缓存已验收，见[GLO-90 验收](glo90-inspection.md)；不同网格重采样、重投影与地形分析待接入 |
| Earth Search | 已接入 | 选景并加载后读取原始真彩色 COG | 原有真彩色、SCL 下载及同网格处理保持可用 |
| Microsoft Planetary Computer | 已接入 | 选景并加载后读取临时签名的原始真彩色 COG | 真彩色、SCL 原文件下载，复用工程、任务、缩略图缓存、像元检查和同网格裁剪/拼接 |
| Copernicus Data Space | 已接入公开 STAC 目录、缩略图和影像范围 | 探索页显示范围；原包准备后可显示本地真彩色与 SCL，已验证合成文件 | 原生授权与完整 SAFE ZIP 下载代码已接入工程和任务；官方 OData 产品解析已实测，真实账号下载尚未验收；SAFE TCI / SCL 本地解码、显示与工程处理已接入，完整生产文件待验收 |
| Landsat 8/9 · Planetary Computer | 已接入 Collection 2 Level-2 实时目录、平台真彩色预览和影像范围 | 探索页使用原始 B4/B3/B2 COG 合成真彩色，支持多景与同网格对比；下载后支持原始单波段灰度地图、同景本地 RGB 及 QA_PIXEL / QA_RADSAT 质量图层 | 30 米 RGB 原始 UInt16 波段、工程裁剪 / 同网格拼接、本地 RGB、逐通道 DN / 反射率与重启恢复已用真实文件验收，见[本地 RGB](local-rgb.md)。两种 QA 原件下载、位解读、完整统计、缓存和只读 MCP 见[原始质量文件](landsat-quality.md)；质量裁剪、三景拼接及带孔多边形处理，六份实际 Landsat 8/9 原件、八份成果 / 三组界面和离线恢复通过，见[质量工程处理](landsat-quality-processing.md)；[同景科学 RGB 筛选](landsat-rgb-quality-mask.md)的十二份真实 Landsat 9 成果通过；进一步完成[Landsat 8 整景与多景质量选景](landsat-coupled-rgb.md)，十六份实际成果的完整 DN、预览、界面、MCP、缓存和科学 RGB ZIP 交付通过。其他质量层与通用跨网格处理仍待接入 |
| NASA Earthdata · HLS L30 v2.0 | 已接入 LPCLOUD 的 HLSL30_2.0 实时目录、公开缩略图和影像范围 | 下载后支持 Int16 单波段灰度地图和本地同景 RGB 组合，已验证夹具 | Earthdata 原生授权与 B04/B03/B02 原始波段下载代码已接入；真实账号下载尚未验收，原始波段工程裁剪与同网格拼接已接入；本地原始 RGB 组合与逐通道 DN / 反射率已用有符号合成 GeoTIFF 验证，生产 HLS 文件仍待验收，见[本地 RGB](local-rgb.md) |

[高程读取与验收边界](elevation-inspection.md)说明 EPSG:4326 度制网格、EGM2008 米制高程、Float32 原值和公开版覆盖范围。

[SRTM 独立读取与验收边界](srtm-inspection.md)说明 HGT 大端 Int16、EGM96 米制高程、共享边缘采样、Earthdata 授权和合成验证的范围；不复用 GLO-30 的科学参数。

## 临时访问与来源记录

[数据源账号入口与验收边界](provider-accounts.md)：桌面设置页和探索页提供对应入口，敏感凭据不进入浏览器缓存、工程、任务或诊断。当前没有真实账号成功授权或 NASA / CDSE 原始产品下载证据。

Planetary Computer 的目录保存原始无签名链接。地图访问凭证仅保留在内存，加载或重试时检查有效期；原生下载端也按 Sentinel-2 / Landsat / NAIP / MODIS / Sentinel-1 RTC COG 容器共享只读 SAS，合并并发申请，并在有效期不足 60 秒时更新。同一景的官方目录核对在内存保存最多 5 分钟，每个容器最多 128 个条目；每份资产仍须匹配官方条目的完整无签名路径与产品、波段。关闭运行时清空这两个缓存，工程、任务、来源记录不保存签名。

访问只开放验证过的 Sentinel-2 存储容器及真彩色/SCL 路径、`landsateuwest.blob.core.windows.net` 上 Landsat OLI/TIRS Level-2 的 B4/B3/B2 表面反射率及 QA_PIXEL / QA_RADSAT 原始质量路径，以及 `naipeuwest.blob.core.windows.net/naip/v002/` 下绑定州、年份、网格、象限、UTM 区号、分辨率和日期的四通道文件。QA 仅接受绑定 Landsat 8/9、采集日期、处理版本、WRS 网格及官方条目的对应资产；NAIP 的官方条目还须声明 red / green / blue / nir 顺序。共享 SAS 不开放任意 Azure 存储桶或跳过官方目录核对。若目录或签名接口返回 429，排队 worker 共用有界等待期（遵循 Retry-After，1–300 秒，未提供时 30 秒），期间直接显示限流原因，之后可手动重试；不循环重试。下载、目录核对与签名请求继续使用用户配置的运行时代理；探索页目录与地图网络访问仍由浏览器/WebView 的网络环境处理。

[原生批量访问验收](../prototype/qa/planetary-batch-verification.json)：新的隔离运行时实际下载 Landsat B4/B3/B2 共 281,060,423 字节，只查询一次官方条目和申请一次容器 SAS；三个 SHA-256 与前次原文件一致，另由 Python 重新计算一致。重启恢复三个完成任务，存储链接没有签名。Sentinel 真彩色 / SCL 的实际 Range 请求各返回 206、4096 字节，共用一次目录核对和一次 SAS。这轮未重新下载 Sentinel 全景原文件。

NASA CMR-STAC 的当前 GET 接口实测忽略 `eo:cloud_cover` 查询。GeoD 按每页返回的云量执行上下限筛选，并将原区间保留到下一页；即使当前页全部被过滤，也继续遵循 cursor。范围和日期仍传给官方目录，不用旧样本冒充查询结果。较大的 NASA 搜索可能因此需要获取更多目录页。

Landsat 表面反射率波段保留原始 UInt16 DN，反射率换算为 `DN × 0.0000275 - 0.2`，无数据值为 0。这不是 Sentinel-2 的 8 位真彩色文件，也不是已拉伸的预览图。参数作为受校验的波段元数据随工程持久化，并显示在文件详情；16 位分支处理原始波段，保留 DN、NoData 与转换参数，不当作 UInt8 真彩色。

[Landsat 原始质量文件](landsat-quality.md)使用独立的 UInt16 位字段读取，不套用上述反射率参数。QA_PIXEL 按第 0 位判断填充，QA_RADSAT 的 0 仍有效且无法独立判断影像覆盖；图例按显示优先级分组，独立完整统计另行展示。工程补入 QA 必须与已保存的 RGB 原始处理目录一致。

[质量工程处理](landsat-quality-processing.md)保留整个 UInt16 样本，按最新非填充景选取并用 TIFF 内部掩膜记录覆盖；饱和层必须有每景配对的 QA_PIXEL。该处理不合并质量位，也不自动筛选 RGB。

[Landsat 探索页真彩色地图](landsat-map.md)补齐原始三波段的在线显示、共享容器签名、半像元校正和真实网络验收。

[原始反射率波段检查](reflectance-inspection.md)说明本轮已接入的灰度显示、原始 DN 与反射率读取，以及 PixelIsPoint 半像元换算。显示统计不替代科学值或 QA 掩膜；Landsat / HLS 已由独立的 16 位采样分支接入[工程处理](reflectance-processing.md)，保留原 DN、NoData 和版本固定的转换参数。

Sentinel-1 RTC 仅开放 `sentinel1euwestrtc.blob.core.windows.net/sentinel1-grd-rtc/GRD/` 中绑定采集日期、IW 模式、产品与极化的 COG。原生 worker 要求当前官方条目声明 IW、匹配极化、Float32、10 米及 -32768 NoData，不能用共享签名访问其他文件。官方集合说明写明账户要求，但本轮官方只读 SAS 接口实际返回可用令牌并完成原文件下载；这次成功不构成持续匿名访问保证。若供应商拒绝访问，软件报告原始失败原因，不尝试其他凭据或镜像绕过。

原文件仍是整景下载，普通 GeoTIFF / VIIRS HDF5 单文件上限 512 MiB；经过审核的 Sentinel-1 IW RTC 单极化 COG、NAIP RGB / NIR COG 和 Copernicus SAFE ZIP 单产品上限 4 GiB；SRTMGL1 HGT ZIP 上限 64 MiB。工程裁剪和拼接要求来源支持的相同像元网格，不执行重投影；SRTM 原包使用共享边缘 Point 网格处理。RTC 工程处理保留 Float32 线性 gamma0、极化和 -32768 NoData，按较新景的有效值处理重叠，不平均、不额外定标。资产访问可能受到提供方的限流和服务状态影响。

## 授权原文件下载适配

探索页下载弹窗先读取脱敏授权状态。未授权时提供对应设置入口，不创建空工程或让用户误以为已开始下载。保存过的授权由原生 worker 在请求时验证或刷新；令牌不经过浏览器下载请求，不写进工程、任务、来源记录或错误。重试复用同一任务，仍从零开始下载，重新读取授权。

HLS L30 的红、绿、蓝原文件为 B04/B03/B02，保留 Int16、30 米、`DN × 0.0001`、nodata=-9999。项目恢复和文件详情使用这些参数，不套用 USGS Landsat Collection 2 的转换。worker 核对当前官方 CMR 条目与原始波段地址；登录页跳转判定为访问未通过。只接受固定 LP DAAC CDN 上同一对象的有效签名跳转，CDN 请求不带 bearer；其链接仅存于本次请求。

SRTM 仅接入 `SRTMGL1_003` 集合中严格绑定瓦片 ID 的官方 `hgt` ZIP。它共用 Earthdata 令牌及上述官方 CMR / CDN 校验流程，授权入口不增加另一份账号。写入完成前核对 HGT 文件名、3601 × 3601 × 2 字节和 ZIP CRC；本地读取按大端 Int16 解码，始终检查受管路径与原包 SHA-256。没有账号时，任务明确失败且没有输出；这些检查不代表已完成生产原包和授权正向验收。

Copernicus STAC 缩略图不作为原产品。下载弹窗按完整产品名（包括处理基线）向官方 OData 查询唯一 UUID、在线状态和产品大小，再保存稳定的 `Products(UUID)/$value` 地址。原生 worker 重新核对 UUID 对应名称、当前在线状态，使用本地授权下载完整 SAFE ZIP；未审核的跨域跳转会停止。目录中的产品大小仅作预计值，不冒充 ZIP 的实际 HTTP 长度。

ZIP 以流式方式写入，检查传输长度（若提供）、文件签名、SHA-256、完整 ZIP 中央目录、选定 `.SAFE` 根目录、元数据及 TCI/SCL JP2 入口，拒绝路径越界、加密和符号链接条目。下载成功仅表示上述传输和目录检查。后续由独立的本地准备任务解码 JP2 并检查像元，见 [SAFE 本地处理](safe-processing.md)；供应商校验和仍未实现。下载前检查磁盘空间，失败时清理临时文件。

[授权下载适配验收](../prototype/qa/protected-download-verification.json)分开记录协议夹具、实际官方元数据、负向授权、隔离运行时和界面证据。2026-10-01 实际 OData 将测试场景解析为 UUID `0d695b42-4b24-4954-ba09-f2a44303fdd8`，产品预计 1,128,466,167 字节。故意无效的令牌使 NASA GET 返回 302 至 Earthdata Login、CDSE GET 返回 401；软件内两个未连接账号的任务失败且没有生成文件。这些不等于成功授权下载，仍需真实账号完成正向原文件验收。

## 真实验收

[机器验收记录](../prototype/qa/provider-integration-verification.json)保留实际场景 ID、稳定链接、字节数、SHA-256、元数据与独立像元核对结果。测试使用独立数据目录，不向用户工程混入验收工程。

- 三个目录均实测范围 `[-122.55, 37.68, -122.32, 37.84]`、UTC 2025-06、0–5% 整景云量、两页检索；验证筛选与分页未跨源。
- Planetary Computer 场景 `S2C_MSIL2A_20250627T184941_R113_T10SEG_20250627T234511` 实际下载真彩色 274,167,302 字节、10980 × 10980、三波段，以及 SCL 2,057,253 字节、5490 × 5490、单波段；均为 EPSG:32610。
- Rasterio 独立核对原文件哈希、空间元数据及各三个原始像元，与运行时结果一致。
- 同一工程区域得到真彩色 2038 × 1789、SCL 1020 × 895 的 GeoTIFF；独立比较全部输出像元与原文件对应窗口，一致。
- 工程恢复的浏览器检查使用捕获的真实目录场景，地图资产访问为实时签名后的 HTTPS Range 请求，返回 206 并显示真实影像。目录重放用于固定验收场景，不冒充当时实时检索。
- 独立运行时重启后，工程、两份原文件、两份结果和校验值保留；再次下载复用原任务，没有重复传输。存储的链接没有访问签名。
- Copernicus 真实缩略图下载后由独立解码器确认为 JPEG。未执行 Copernicus 账号登录或原产品下载。
- 界面切换在明暗主题 1440 × 900、紧凑 1024 × 900 下共九例通过。界面 QA 用隔离目录响应夹具，真实请求证据单独记录。

前端、Rust runtime、桌面来源边界、CSP、契约、依赖隔离和构建检查通过。没有打安装包或发布远程版本。

### Landsat 与 NASA 本轮实测

[Landsat / NASA 验收记录](../prototype/qa/landsat-nasa-integration-verification.json)保存稳定源链接、实际字节数、SHA-256、Rasterio 独立核对、运行时重启恢复、实时浏览器请求和界面检查结果。原文件保存在隔离的测试数据目录。

- 两个来源均实测同一小区域、UTC 2025-06、两页目录；另测 10%–40% 云量范围，NASA 的客户端过滤覆盖空页继续翻页。
- Landsat 9 场景 `LC09_L2SP_044034_20250628_02_T1` 的 B4/B3/B2 三个原文件实际下载共 281,060,423 字节，均为 7671 × 7791、UInt16、单波段、EPSG:32610、30 米。独立核对文件哈希、像元类型、空间元数据及原始 DN，工程保留各波段的 scale/offset/nodata。
- 独立运行时重启后，原工程和三个完成任务恢复；再次请求下载复用原任务，未重复传输。工程和任务只保存无签名来源链接。
- 浏览器直接检索两个官方目录成功。Landsat 平台预览实际显示 1009 × 1024，NASA HLS 公开 JPEG 实际显示 1000 × 1000；NASA 图片经官方入口跳转到明确核验的 CDN，桌面 CSP 仅为图片开放该指定来源。
- 新入口、下载内容和工程恢复在明暗主题 1440 × 900、紧凑 1024 × 900 共九例通过，原有三个来源的九例切换回归通过。界面重放使用捕获目录与实际测试文件记录，和上述实时网络验收分别记录。
- 首次原文件下载验收未进行 NASA 账号登录或原文件下载，未验收 Landsat 的软件内渲染或处理。后续的本地单波段显示与像元核对见[反射率检查验收](reflectance-inspection.md)；真实 Landsat 三波段工程裁剪已通过逐 DN 比较；HLS 处理及多景重叠使用夹具验证，见[处理验收](reflectance-processing.md)。桌面开发环境继续使用热更新，没有生成新安装包或公开发布。

## 官方参考与测试夹具

- [Planetary Computer STAC](https://planetarycomputer.microsoft.com/docs/reference/stac/)与[签名访问文档](https://planetarycomputer.microsoft.com/docs/concepts/sas/)。
- [Copernicus STAC](https://documentation.dataspace.copernicus.eu/APIs/STAC.html)与[产品下载及认证](https://documentation.dataspace.copernicus.eu/APIs/OData.html)。
- [USGS Collection 2 表面反射率与转换参数](https://www.usgs.gov/landsat-missions/landsat-collection-2-surface-reflectance)、[USGS 原始质量字段定义](https://www.usgs.gov/landsat-missions/landsat-collection-2-quality-assessment-bands)、[Planetary Computer Landsat 数据集](https://planetarycomputer.microsoft.com/dataset/landsat-c2-l2)。
- [NASA CMR-STAC 官方使用文档](https://github.com/nasa/cmr-stac/blob/master/docs/usage/usage.md)、[Earthdata Login](https://urs.earthdata.nasa.gov/documentation/for_users/welcome)。
- [Landsat 捕获目录](../prototype/public/samples/landsat-response.json)和[NASA HLS 捕获目录](../prototype/public/samples/nasa-response.json)：仅用于可重复测试，不作为应用运行时数据源。
- [Planetary Computer 目录夹具](../prototype/public/samples/planetary-computer-response.json)和[Copernicus 目录夹具](../prototype/public/samples/copernicus-response.json)：2026-10-01 从上述官方检索端点捕获，固定 UTC 2025-06 小区域查询，保留一条场景、必要资产、空间字段和分页链接。应用运行时不读取这些夹具；它们仅用于可重复测试。
