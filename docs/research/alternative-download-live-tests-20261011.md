# 替代渠道真实下载实测 · 2026-10-11

本轮实际取得 **3 个公开数据产品的样本**：JAXA AW3D30 DSM（附质量掩膜）、OpenTopography Copernicus GLO-90，以及 Esri Landsat Level-1 单红波段区域导出。均完成完整文件下载和 Rasterio 全像元解码；不是只检查目录或请求一个 Range 片段。

**仍未解决北京 Landsat C2L2 SR 原件的等价下载替代。** Esri L1 子集不能冒充 C2L2，国内三家需要继续做账户与同产品验证。此次未修改业务下载源码、未发布、未产生已知付费订单；没有创建账户或读取用户凭据。

## 已成功的真实样本

| 路线 | 样本与区域 | 完整文件下载 | 栅格验收 | 可成立的结论 |
|---|---|---|---|---|
| JAXA Earth / AW3D30 v4.1 | DSM，经度116–117°、纬度39–40°，含北京中心附近；非整个北京市 | 8,418,816 B；1.261秒，约6.365 MiB/s | 3600×3600、Int16、EPSG:4326、1 arc-second；完整12,960,000像元；非NoData99.9761% | 新的匿名DSM分发可走通。它是地表高程产品，非原始ALOS场景；2024-04是版本发布期，不是采集日期 |
| JAXA Earth / 同块MSK | AW3D30质量掩膜 | 43,231 B；0.297秒 | 3600×3600、UInt8、EPSG:4326；完整12,960,000像元解码 | 伴随质量层可同时取得，不能丢掉质量信息 |
| OpenTopography / COP90静态资产 | 北京北部N40/E116，一度瓦片，约116–117°E、40–41°N | 6,545,035 B；6.328秒，约0.986 MiB/s | 1201×1201、Float32、EPSG:4326；完整1,442,401像元；高程3.348–1765.994m | 所测静态原文件匿名可下载；与需Key的裁剪加工API分开 |
| Esri / Landsat8_Views | 北京小区116.35,39.89–116.40,39.93；固定2026-10-07的具体Landsat8场景 | 132,216 B；生成0.735秒＋下载1.218秒 | 143×149、Int16、单红波段、EPSG:32650、30m；21,307像元全解码，有效率100% | 小区域科学数值导出可走通。是C2 Level-1 RT来源和服务处理像元，非C2L2 SR原件 |

速度是当前网络下这些有限样本的观察值，未切换代理节点；不代表独占带宽基准、相同文件的跨平台比较或全国大任务速度。两份DSM不是同网格、同版本、同区域的比较试验。

## Landsat 子集的产品与处理记录

Esri场景固定为 `OBJECTID=5974848`，产品 `LC08_L1TP_123032_20261007_20261007_02_RT`，采集时间2026-10-07 02:53:34 UTC。导出使用 `None`函数、零基 `bandIds=3`，未换成全色锐化或RGB图。

服务声明会将DN转为0–10000的 apparent reflectance。输出采用UTM50N的30米网格和最近邻重采样；所以这是服务处理后的区域数值成果，不能声称保持完整场景原始像元网格、C2L2大气校正或配套QA文件。实测值域290–7692。

真正的C2L2服务 `https://landsat.imagery1.arcgis.com/arcgis/rest/services/LandsatC2L2/ImageServer` 匿名请求虽然HTTP200，但JSON为 **499 / Token Required**；没有绕过。Esri官方[应用说明](https://github.com/Esri/imagery-explorer-apps/blob/master/README.md)也明确这是需ArcGIS登录的订阅内容。

## 未通过与未完成项

| 路线 | 本轮实际状态 | 下一步 |
|---|---|---|
| GSCloud Landsat | 浏览器目录可读，能进入Landsat8实体表；页面显示登录/注册。筛选操作多次超时，最后观察仅路径123、行32被填入，表仍是原始全库记录，日期筛选未完成；没有下载影像 | 不能把自动化超时认作源站下载失败，也不能由默认页日期推断最新数据仅到2022。登录并核对C2L2、同日期、完整波段后再下载 |
| AI Earth | 按当前官方SDK地址匿名访问 `/open/stac` 和 `/open/stac/collections`，均HTTP503、190字节HTML；未取得影像目录或样本 | 503不能当401鉴权错误。需要平台账号/授权后再区分网关、路由和权限；已准备小AOI C2L2 SR_B4脚本，未执行 |
| PIE Engine | 入口/目录HTTP200但只是HTML应用壳；无匿名GeoTIFF；未取得本次SDK/导出授权 | 使用用户平台账号，核对当前数据与额度；准备的脚本为 `LC08/02/SR` / `B4`，不是旧 `LC08/01/T1`；未执行 |
| Esri C2L2 | 匿名返回JSON499 Token Required | 有ArcGIS订阅授权才可继续；不可填成免费匿名镜像 |
| OpenTopography COP30 | 同位置静态文件HEAD200，Content-Length58,311,536 B | 超过这轮30MB样本预算，未下载；不是完整文件验收通过 |

GSCloud官方[帮助](https://www.gscloud.cn/help/uses)描述下载/预订与批量FTP申请，不代表已经存在软件可直接调用的免费稳定API。AI Earth的[数据SDK](https://pypi.org/project/aiearth-data/)支持鉴权数值块读取；安装包下载和脚本语法检查不算影像下载成功。

## 文件与证据

真实文件、STAC链、请求记录、预览和原始JSON保存在项目的 `.verification/alternative-download-test-20261011/`，该目录为本地验收产物，不作为公开仓库影像附件提交。

- [JAXA 下载与解码记录](../../.verification/alternative-download-test-20261011/jaxa/summary.json)
- [JAXA DSM GeoTIFF](../../.verification/alternative-download-test-20261011/jaxa/beijing-aw3d30-v4.1-dsm.tif)
- [JAXA MSK GeoTIFF](../../.verification/alternative-download-test-20261011/jaxa/beijing-aw3d30-v4.1-msk.tif)
- [OpenTopography 下载与解码记录](../../.verification/alternative-download-test-20261011/opentopography/download-report.json)
- [OpenTopography COP90 GeoTIFF](../../.verification/alternative-download-test-20261011/opentopography/Copernicus_DSM_30_N40_00_E116_00_DEM.tif)
- [Esri Landsat 子集下载记录](../../.verification/alternative-download-test-20261011/esri-landsat/report.json)
- [Esri Landsat 单红波段GeoTIFF](../../.verification/alternative-download-test-20261011/esri-landsat/beijing-red-none.tif)
- [Esri C2L2 鉴权失败记录](../../.verification/alternative-download-test-20261011/esri-landsat/c2l2-anonymous-auth-report.json)
- [国内渠道匿名请求记录](../../.verification/chinese-channel-sdk-audit-20261011/anonymous-probe.json)

JAXA DSM SHA256：`c5eafee72fe18dffe03113d7dde4240d12a5c421cfc5ff4bc709476ebd69a189`。
COP90 SHA256：`e47435025369366c6e942e4d76665c830bc16e64f1f55415a254129ac7e99e33`。
Landsat子集SHA256：`30db5eddb70d5d1ddc407612eaef907c96d5b62cfa4e164db08e443b342aa880`。

匿名下载样本验证与GeoD软件完整接入是两层验收。本轮未完成软件内检索、确认、任务管理、断点恢复和成果展示的完整链路，因此不修改“已接入”产品声明。
