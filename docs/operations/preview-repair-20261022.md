# 预览失败排查与修复：2026-10-22

## 历史证据与排查边界

线上 production 事件排除已登记验收标识后有 19 次 `preview_failed`：GLO-90 6 次、GLO-30 2 次、Earth Search 5 次、Planetary Computer Sentinel-2 4 次、Planetary Landsat 2 次。聚合依据为 `.verification/adoption-check-20261022/usage-summary.json`。

匿名埋点不包含精确区域、影像 ID、原始错误或签名 URL。因此无法逐条重放原用户当时场景，也不能将下面发现的问题说成全部 19 次失败的已证实根因。没有修改或删除线上历史失败记录。

## 已确认并修复

- 原来的光学/高程 COG 读取直接交给 GeoTIFF 默认传输，未接入已有的有界重试。新增共享 `CogPreviewSource`，将首次元数据和后续像素 Range 读取都接入同一传输：临时连接、408/425/429/5xx 和响应体读取中断最多三次尝试；每个读取总期限为 30 秒。403、校验失败、取消不做无效重试。
- 原来的普通 Sentinel-2 GeoTIFF 源在初始元数据请求失败时只改变 source 状态，不拒绝 `getView()`，页面会等到约 45 秒的静默超时。共享源现在及时结束失败承诺。DEM/Landsat/NAIP 也使用同一处理，保留真实传输类别，不把传输失败统一伪装成产品网格不匹配。
- 切换/销毁来源时取消该源仍在进行的读取，防止后台旧请求继续占用连接和重试。外部取消信号也保留。
- Range 被忽略而返回 200 时取消响应体，不以整文件下载掩盖预览失败。原来的高程 Float32、PixelIsPoint 半像素、Landsat 原波段与校准验证保持不变。
- 元数据失败埋点使用真实原因做粗分类，仍不上传原始错误、地址或凭据。源公开错误使用固定安全信息，不暴露 SAS 签名。

## 实际验证

使用本仓库 Vite 开发运行版 `http://127.0.0.1:4317/#Explore` 和独立本地原生服务 `127.0.0.1:4318`，数据目录 `.verification/preview-repair-20261022/store`。没有读取用户原工作空间、创建下载任务或调用模型。开发环境埋点与生产统计分开。

区域：西 -122.6、南 37.6、东 -122.3、北 37.85。目录为实际 HTTPS 查询；图像来自原文件字节读取，不以缩略图、测试栅格或离线样例替代地图预览。

| 来源 | 实测条目 | 结果与截图 |
| --- | --- | --- |
| Earth Search Sentinel-2 | `S2B_10SEG_20261015_0_L2A` | mapReady=true，真彩色可见；`earth-search.png` |
| Planetary Computer Sentinel-2 | `S2B_MSIL2A_20261015T185349_R113_T10SEG_20261015T221625` | mapReady=true，真彩色可见；`planetary-computer.png` |
| Copernicus DEM GLO-90 | `Copernicus_DSM_COG_30_N37_00_W123_00_DEM` | mapReady=true，灰度地形可见；`glo90.png` |
| Copernicus DEM GLO-30 | `Copernicus_DSM_COG_10_N37_00_W123_00_DEM` | mapReady=true，灰度地形可见；`glo30.png` |
| Planetary Landsat | `LC09_L2SP_044034_20261005_02_T1` | mapReady=true，三原始波段真彩色可见；`landsat.png` |

已逐张查看以上截图，预览无失败提示或持续加载。截图在 `.verification/preview-repair-20261022/`。

### 真实服务中断与恢复

停止了本次专用 4318 测试服务（先核对进程名与专用 data-dir），重新请求 GLO-90：页面及时出现“地图预览的地理信息读取失败，请检查网络或重试”和“重试地图”，不持续卡在读取元数据。保存 `service-offline.png/.txt`。

重新启动同一测试服务，实际点击“重试地图”：预览恢复到 mapReady=true，错误与进度浮层消失；继续放大地图正常。保存 `service-recovered.png/.json`。最终测试服务 `/jobs` 返回空数组，未创建原件下载作业。

### 有控制的故障回归

新测试通过真实本地 HTTP Range 端点返回一次 503 后返回合法 TIFF 头，验证重试后字节和 Content-Range 保持一致。另验证响应体中断恢复、403 不重试、整文件 200 不读取、销毁取消。它们是有意制造的故障测试，不是供应商当时发生故障的证据。

55 项相关测试通过：28 项 Node 测试（含新传输测试 4 项）、27 项 OpenLayers/React 集成测试。生产构建通过；保留原有大 chunk/动态导入提示，不将这些提示混作失败。

## 当前交付状态

修复在本地源码与运行版中，尚未重新打包、替换或发布正式桌面安装包。原生 HTTP 高程链路实际使用了本仓库已有 runtime；本次没有修改 Rust 或宣称完成新的 Tauri 安装包验收。

当前真实来源复测通过，不保证所有网络、区域和历史影像都不会失败。后续继续观察正式新版的失败分类；不为追溯历史而扩大采集精确区域、影像 ID 或凭据。
