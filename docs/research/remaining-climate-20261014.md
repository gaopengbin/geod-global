# 剩余原渠道：气候 / NUM 新鲜可行性核对 — 2026-10-14

本轮只研究原 `dynamic-world`、`era5-land`、`srtm-num`。新鲜匿名 HTTP 与官方资料没有取得可满足本轮原件验收的完整数据文件；**原 ID 核销 0，额外新增 0，状态和账本未改**。没有读取、创建或输出用户凭据，没有运行 GEE/CDS 计算、付费或大批任务。完整匿名回执与现有 GEE 契约复查位于私有 `.verification/remaining-backlog-20261014/climate/`；[机器可读摘要](remaining-climate-20261014.json)保存 HTTP 状态、完整证据文件 SHA 与后续门槛。

## 历史事实与本次实时结果

| 原 ID | 旧记录 | 2026-10-14 重新核对 | 当前结论 |
| --- | --- | --- | --- |
| `dynamic-world` | 旧研究只确认官方 Earth Engine collection 与数据许可，没有匿名原 TIFF；本轮禁止用 GEE 导出替代原件核销。 | [官方目录](https://developers.google.com/earth-engine/datasets/catalog/GOOGLE_DYNAMICWORLD_V1?hl=en)实际 HTTP 200；仍是 `GOOGLE/DYNAMICWORLD/V1`，每景 Sentinel-2 L1C 派生的九类概率及 label。未发现该页面提供可验证完整原 TIFF href；没请求或运行任何 EE 计算。 | 官方身份可确定，匿名完整原件仍未取得。生成脚本或成功下载用户导出不能抵扣此原 ID。 |
| `era5-land` | 旧待办是 CDS/ECMWF 的小时/月度陆面再分析，格式与授权未完成；不能用别的 NOAA 气候产品替代。 | [CDS 当前 ARCO 页面](https://cds.climate.copernicus.eu/datasets/reanalysis-era5-land?tab=analysis_ready_data)与[官方示例 notebook](https://github.com/ecmwf-training/dss-notebooks/blob/main/datasets/reanalysis-era5-land/arco-access.ipynb)均实际 200。从 notebook 原样提取的 2m-temperature geo/time 两个 `zarr.json` 元数据对象匿名均 **401**，没有绕过认证。 | ARCO 是官方重打包数据，仍需 CDS API key；也不是现已支持的 TIFF 格式。没有匿名完整时间对象和已支持格式可立即接入，不承诺转换即完成。 |
| `srtm-num` | 旧 CMR 记录为正式 `SRTMGL1N.003`，N36W122 小 ZIP 的旧匿名请求 401；OpenTopography 公共 SRTM_GL1 桶没有 NUM。 | 当前 CMR collection 与 exact granule 元数据均 **200**，collection `C2763266368-LPCLOUD` / granule `G2822570219-LPCLOUD`。同正式 ZIP 的匿名 0–15 byte 请求现在 **302** 到 `urs.earthdata.nasa.gov/oauth/authorize`；未跟随重定向、未保留 query/token。 | 当前是登录重定向，不把旧 401当成本次状态。未取得完整 ZIP，也未核 CRC / 原值 / SHA；现有 HGT decoder 不支持此一字节 NUM。 |

NUM 的正式原 URL 仍为 `https://data.lpdaac.earthdatacloud.nasa.gov/lp-prod-protected/SRTMGL1N.003/N36W122.SRTMGL1N.num/N36W122.SRTMGL1N.num.zip`。CMR 的 `granule_size:0.225549` 是目录 MB 声明，不是本地完整字节校核；不能由它造完整原件长度或 SHA。记录的真实观测区间为 2000-02-11 至 2000-02-21。[正式集合查询](https://cmr.earthdata.nasa.gov/search/collections.json?short_name=SRTMGL1N&version=003)、[官方用户指南](https://lpdaac.usgs.gov/documents/179/SRTM_User_Guide_V3.pdf)。没有重复探测父任务已核对的 OpenTopography 桶。

## 2026 许可、授权与科学含义

Dynamic World 数据是 CC BY 4.0，需 Google / National Geographic Society / WRI 及 modified Copernicus Sentinel 署名；这不等于 Earth Engine 平台权限。标签 0–8 是最大类概率的索引，0 为 water，不可作为 NoData。云/阴影掩膜与模型概率保留，分类并非实测地类证明。[官方目录的条款与波段](https://developers.google.com/earth-engine/datasets/catalog/GOOGLE_DYNAMICWORLD_V1?hl=en)。

CDS 此次实际 HTML/嵌入 catalog 明确 `license:CC-BY-4.0`、许可 revision 1，并要求明确 Copernicus / 数据提供方署名。[当前 CDS 条款入口](https://cds.climate.copernicus.eu/datasets/reanalysis-era5-land?tab=download)。API 仍需用户 personal access token 与接受数据集条款，未探测任何用户 token。ERA5-Land 是再分析；当前 CDS 0.1°交付格网不叫原生 9 km 网格的逐字节镜像，GRIB/NetCDF 与 ARCO 重打包分别记录。[CDS API 官方流程](https://cds.climate.copernicus.eu/how-to-api)。

GEE 的 ERA5-Land 路径仍是 `ECMWF/ERA5_LAND/HOURLY`，2m temperature 的单位为 K；累积变量与平台新增的小时差分层必须区分，蒸散三个变量错位是官方已知问题。若另获明确允许的 generated 流程，最小范围应先选单个 UTC 小时 `temperature_2m` 二维切片，保留 K 与 original valid time，不自动换 °C、月均或累积降水。[GEE 官方产品定义](https://developers.google.com/earth-engine/datasets/catalog/ECMWF_ERA5_LAND_HOURLY?hl=en)。当前 GEE 页引用 C3S/CAMS 署名约定，不能把该平台说明替代 CDS 当前取件版本的许可核对。

Earth Engine 自 **2026-04-27** 起为已验证非商业项目逐步应用按月 EECU 配额；Community 为 150 EECU-hours / 540000 EECU-seconds。配额是项目级，其他 Google Cloud 服务及商业模式另有费用，不称无限免费或零计算成本；本轮没有消耗该项目计算配额。[2026 官方非商业层级](https://developers.google.com/earth-engine/guides/noncommercial_tiers)。

SRTMGL1N 的一字节原 NUM 为 3601² point grid 来源/填充代码，**不是高程或通用质量置信度**。官方举例：1/2 为水体处理来源，101–200 编码 ASTER 场景数量，201–224 编码 SRTM 非空 swath 数量；来源类别和计数不能统一当连续值。未读原件时不自行指定 0/255 为 NoData，也不能套用早期雷达亮度 mosaic 的 `.num` 计数定义或 NASADEM NUM。[SRTM V3 指南](https://lpdaac.usgs.gov/documents/179/SRTM_User_Guide_V3.pdf)。

## 最可操作的后续原件范围

1. **SRTM NUM：正式 N36W122.SRTMGL1N.003 单完整 ZIP。**体量和一字节 3601² 原成员有可控实现空间，但须先通过用户已有的 NASA Earthdata 授权读取正式完整文件；不新建凭据。随后保留 original ZIP / raw NUM 两份 SHA、CRC、Point 注册和来源代码；实现独立 `providers/srtm_num.rs` / `raster/srtm_num.rs`，保持当前 SRTMGL1 signed big-endian 16-bit HGT 路径不变。Provider/registry/service 的共享注册需父任务另行分工，取得真实原件与独立原值参考后才开始适配及最终验收。当前未实现。
2. **ERA5-Land：有授权时的单 UTC 小时 2m temperature 正式 CDS 文件。**先取得用户已接受条款后的真实完整 GRIB（或明确转换 provenance 的 CDS NetCDF），保留原 request、原响应、完整 bytes/SHA、时间/网格/单位/缺测；不能用样例图、辅助 land-sea mask、GEE 导出或另一个 ERA5 dataset 清账。软件目前没有此类原件 codec/协议链；原请求取得与受限二维解码需独立分工，不假造 TIFF href。ARCO 两个当前端点已401，暂不将其当落地捷径。

这两项是明确后续契约，不是“已可匿名完成”承诺。三项当前没有新增原件或 Ready 数据门槛的实测通过，继续保留原 Planned 状态。

## 现有用户 GEE 导出可复用处与限制

`earth_engine.rs` 的 receiver 是 `{url,name}`；它仅接受严格 GEE `getPixels` URL，将完整导出 TIFF 保存并核对 bytes/SHA/projectId。32 MiB / 180 s 下载、10000 轴长、1–16 波段、128 MiB 全基准块样本预算及30 s解码均保留。现有 HTTP/Tauri 能完成下载、检查、离线原件保存和重开；不存临时链接或凭据，`eecuSeconds:null` / `usageStatus:not_measured` 不伪造计费数据。

现 script generator 仅接受 Landsat LC08/LC09、六个特定波段和30 m；没有 Dynamic World / ERA5-Land 的 verified dataset identity、时间、科学波段/单位和原投影 receipt。Opaque GEE URL 与用户名称不能证明 publisher 产品身份。即使以后添加专用 recipe，产物也应叫 **用户项目生成的 subset/reduction**，保留用户声明与独立验证字段之别；本轮依然不能拿它代替发布方原件或清原 ID。因此没有实现无真实回执的模板或改共享协议。

若未来单独授权 generated 路径，Dynamic World 最小建议是一个确切观测的 label + 九概率（不同原数据类型分开导出、不静默升型），完整绑定 collection/image ID、算法与 QA 版本、UTC、bands/order、AOI/grid、mask/NoData、所有转换和输出 SHA；ERA5-Land 单小时温度契约同样保留原 K、UTC 与用户项目归属。表达式/recipe SHA 只能证明声明没有改变，不能独自证明远程执行身份。这是后续设计要求，未执行或宣称验收。
