# 年度夜光原件：当前原生内核真实复验

实际运行时间：第一阶段为 2026-10-13 14:19:54–14:24:55 UTC，第二阶段为同日 14:30:14–14:34:31 UTC；文件名中的 20261014 是批次标识，不代替实际运行日期。本记录只涉及本地私有验收，没有新增账号、修改代理或网络设置，也没有部署。

**完整原件、通用原生下载路径和随后注册的专用产品 resolve/connect/download 闭环、原始像元、PNG 和私有重开均已通过。在线地图尚未由本任务验收；实际断点恢复也未发生，不能宣称续传已实测成功。** 本任务未修改软件目录 Ready 状态。

## 实际使用的发布文件与软件路径

同一作者的正式 [Zenodo v0.4 记录](https://zenodo.org/records/17294744) 提供 [2024 年完整数值原件](https://zenodo.org/records/17294744/files/nightlights.average_viirs.v21_m_500m_s_20240101_20241231_go_epsg4326_v20250904.tif)。不使用旧 OpenLandMap 2021 校验冲突原件、API `/content` 的 MIME 不兼容路线、镜像、临时导出或凭据。

当天重新读取正式 `https://zenodo.org/api/records/17294744/files`：HTTP 200、60,216 B，完整未改写响应 SHA256 为 `17d36cb8d9f4424ae2053f0dae2718822287d3169384c41da6ca01a88afd8c6b`。精确文件条目仍声明 **63,713,625 B**、`md5:0b976892d40c2d0d99177c727289eaf2`，与本次软件完成文件一致。发布者 API 条目保留其真实 `links.content`，未将兼容网页文件 URL 改写进发布方元数据。

冻结使用的 runtime SHA256：

`942e58f5933fff6dc7828848c40b25e5b4fd109f29f0c45a8d28f02267febf36`

候选实际文件是 `crates/geod-runtime/src/datasets/nightlights.rs`。第一阶段冻结程序仍由 `#[cfg(test)]` 将其排除在正式 `datasets::list()` 之外，真实 `/datasets/resolve` 对 `openlandmap-nightlights` 返回 400，回执已保留；这一阶段只证明通用内核，不冒称专用产品连接成功。随后主任务正常编译并注册配方，另用新程序完成下述第二阶段验证。

之后只使用当前软件已经提供的通用公开数值栅格入口：

1. `/stac/connections` 实际连接上述精确正式 URL，`kind=raster`。
2. 读取软件生成的不可变 snapshot，选择其 `raster` 资产；没有构造发布方 STAC 或客户端 `reviewedDataset` 证明。
3. `/stac/project` 保存真实 AOI `[116.3,39.8,116.5,40]` 与源 pin。
4. `/stac/downloads` 创建唯一的新原件任务，由软件自行请求源站并落盘。
5. `/stac/jobs/{id}/inspect`、`pixel` 和重启后的相同 API 验证原件与项目。

存储使用新 UUID 私有目录；独立参考 TIFF 没有注入存储，旧数据库和旧下载任务没有复用。原件路径、大小、哈希及 source pin 均由当前软件产生。

## 实际完整下载结果

| 项目 | 实际回执 |
| --- | --- |
| 新任务 | `ccf684d7-821d-47c9-b7a2-6a2b42fdb323` |
| 新项目 | `58b524f3-a622-42b4-b1e3-d54d3be9e3bf` |
| 新 snapshot | `d8b7afce6830bb184f027002b129e97bfe2e498557854fd5d401a177660bade8` |
| 完整字节 | 63,713,625 B，与实时发布文件条目一致 |
| 软件与本地 SHA256 | `e6edde68952052f8c53050e2ce7082bab08ddd0e15a696f999215e2d57194c53` |
| 本地 MD5 | `0b976892d40c2d0d99177c727289eaf2`，与实时发布方 MD5 一致 |
| 软件传输 | 第一次新完整任务成功，`transfer.mode=fresh`、`resumedBytes=0` |
| 验收观察耗时 | 284.026 s，从实际下载任务排队后计至成功，含状态轮询；不含此前目录/连接检查，也不是稳定速率保证 |

本次成功不抹去 [上次原生任务于 17,648,832 B 中断的记录](../next2-nightlights-connection-diagnostic-20261012.md)。单独的本次 HEAD 辅助探查也发生 25 秒读超时，没有取得响应头；其失败单独保存，没有当作当前正在传输的完整 GET 失败或源站文件错误。

## 原始网格、像元和本地预览

实际原件仍为 **86,400 × 35,849、单波段 Int16、512 × 512 tiled、EPSG:4326、PixelIsArea**。原始网格间距为 `0.00416666666666667°`，15 角秒；“500 m”是名义间距，地面距离随纬度变化。

保留实际原始 bounds `[-180,-62.000833333333446,180.00000000000028,87.37]`、原始 transform 及字节。仅未来产品查询 footprint 可与合法经度 180 相交；不能将显示范围处理写成原件已重采样或修改网格。

| 原始列、行 | 软件原值 | 软件 NoData |
| --- | ---: | --- |
| 71135, 11392（北京） | 150 | false |
| 72359, 13480（上海） | 64 | false |
| 25439, 11200（纽约） | 61 | false |
| 35999, 20968（大西洋） | 0 | false |
| 28799, 21688（亚马孙） | 56 | false |
| 43199, 328（北部） | −32768 | true |
| 43199, 35608（南部边缘） | 0 | false |

7 个原像元与此前完整独立解码记录全部一致；软件全文件 SHA256 也与独立完整原件一致。像元 API 返回的是原始基准网格，不是概览像元或按比例改写后的值。零值维持有效存储值，不能将其解释成无观测或确认没有灯光。

软件实际生成 **768 × 319 PNG**，本机已目视检查，可见数值灰度中的灯光点，存在大范围暗值；它不是整幅统一灰色占位图。其采样使用原件的嵌入 overview、自适应原值灰度范围 `[-6,92]`，不应用物理辐射校准，不验证正式 COG 规范符合性，也不能代替浏览器/桌面在线 GeoTIFF 地图验收。

私有 runtime 停止并重新启动后，snapshot、项目 AOI/源 pin、原件 SHA、检查结果、PNG 和全部 7 个像元均保持一致；任务仍为 1 个，没有启动新传输。验收 helper 已停止，私有随机端口已核对关闭。用户 4318 服务和桌面进程未操作。

## 断点恢复：能力与实际结果分开

当前 `lib.rs` / `transfer.rs` / STAC 路径包含强 ETag、源 pin/请求/元数据绑定、精确 `Range` / `If-Range` 和最终完整校验。受控恢复测试的存在不表示每个上游源都提供可接受的 validator。

本次下载全过程没有观察到有效 `.part.resume.json`，所以私有脚本没有执行计划中的“已有真实 checkpoint 后中断并重启”，没有伪造 ETag、partial 或 checkpoint，也没有换源。实际完整任务没有失败或重试，最终明确为 `fresh`，**本次不能宣称验证了该夜光文件的 native resume**。

辅助 HEAD 超时，没有取得本次完整 GET 的原始响应头，不能据此认定 Zenodo 永远不提供 ETag、续传不可能或当前缺少 checkpoint 的具体原因。以后只有取得真实 validator 并实际经历中断、重启、恢复与最终一致性验证，才可为该产品单独给出续传成功声明。

## 第二阶段：专用产品配方独立闭环

主任务注册后的独立程序 SHA256 为 `79931776c1194e2a1266aa88f97d8c4b5cb08a33ef55458ad12c161b362c4527`。再次创建全新 UUID 存储，未复制或注入第一阶段的完整原件，不复用其数据库、任务或 snapshot。

真实 `/datasets/resolve` 与 `/datasets/connect` 均成功。直接文件配方使用既有真实契约：`snapshot.properties.datasetPreset` 与 `asset.metadata.datasetPreset` 完全相同，dataset/request/layer/sourceId/sourceBounds/schemaVersion 与实际 plan 一一核对；不将其冒充发布方 Item 的 `provenance.reviewedDataset`。原始时间戳仍为空；2024 年度参考区间只在客户端配方中声明。

| 项目 | 第二阶段实际回执 |
| --- | --- |
| 私有存储批次 | `run-e5de65ed-5374-421b-9600-860f68c74d12` |
| 新下载任务 | `1bb69eb9-fed3-445f-bd66-b8c39d0861b6` |
| 新项目 | `d5c3f945-cb01-4dc7-8b1f-7f3595f059a9` |
| 新 snapshot | `bd7d7fb6e930a7b6196f66418da5d22739118923f4a50741c55a466df2726440` |
| 完整软件下载 | 首次新完整传输 251.864 s，63,713,625 B；`mode=fresh`、`resumedBytes=0` |
| 完整校验 | 与第一阶段及独立原件同一 SHA256，且与当天实时发布方 MD5 一致 |
| 原生检查 | 同一 Int16/EPSG:4326/原始网格/NoData −32768、上表全部 7 个原值和 768 × 319 PNG |
| 私有重开 | 同一 snapshot、项目 AOI/源 pin、原件、PNG、全部像元；任务仍为 1 个，没有新传输 |

4 个实际产品 API 负例均返回 400：2005 建模年、未支持月份、`radiance` 图层、实际文件范围外的南部极区；不替换为其它年份、图层或区域。第二阶段也没有中断、重试或实际 resume。

第一次专用程序检查只完成 resolve/connect、没有创建任务，因私有脚本误按发布方 Item 位置读取 direct-raster 证明而结束。失败记录保留于 `run-bf1d818f-a2ac-435d-a9eb-86c98e46efd0/`；核对真实代码契约后只修私有脚本并另开新存储。它不是下载、源站或产品模块失败，也未修改共享代码或降低校验。

## 注册建议与剩余门槛

- 产品 ID `openlandmap-nightlights`，图层 `annual-dn`，年份 **2024 only**，固定正式作者 v0.4 原件；不添加未验证年份、月份、场景、地区或质量层，不制造观测时间戳。
- 保留作者处理 DN 身份：作者说明从 0–200 缩放到 0–2000，但实际 TIFF scale 为 **10**、offset 0、无 unit。不得猜成 0.1、物理辐射 `nW/cm²/sr` 或旧集合的 proportion；灰度拉伸与原值单位分开。
- NoData 仅为 **−32768**；负非缺失值、32767 和零保持原值，不补造质量掩膜。没有次数/质量层时不能承诺感测精度或完整有效覆盖。
- 保留正式作者 CC BY 4.0、版本、来源和 EOG 署名；本次当前发布 MD5 比较是独立验收收据，不冒充 direct-raster 契约本来不存在的发布方 STAC checksum 字段。
- 专用配方的真实 `/datasets/resolve`、`/datasets/connect`、完整下载及 provenance 验收已在第二阶段通过。仍需主任务检查浏览器/桌面的原始文件在线预览、多选与切源，目录不由本任务晋升。

## 本机证据

私有根目录：`.verification/nightlights-native-recheck-20261014/`。完整未改写实时发布 JSON、发布文件条目及收据在该根目录；第一阶段原件、API trace、PNG、像元与重开回执在 `run-b5539937-23b9-43d9-83db-65310a30ae66/`，第二阶段在 `run-e5de65ed-5374-421b-9600-860f68c74d12/`。

关键文件：`publisher-files-current.raw.json`、`publisher-files-current-receipt.json`、`publisher-entry-current.json`、`publisher-original-current-head-failure.json`、`latest-acceptance.json`，以及私有 run 中的 `resolve-response.json`、`connect.json`、`snapshot.json`、`project.json`、`job.json`、`integrity.json`、`final-current-publisher-check.json`、`inspection.json`、`pixels.json`、`preview.png`、`reopen.json` 和 `api-trace.json`；第二阶段还有 `negative-resolve-rejections.json`。模块的 23 条精确可见字符串中文映射准备于私有 `nightlights-zh-fragment.json`，由主任务并入共享语言文件。

第一阶段机器可读状态为 `genericCoreAccepted=true`、`presetAccepted=false`；第二阶段为 `genericCoreAccepted=true`、`presetAccepted=true`。两阶段均为 `nativeResumeActuallyObserved=false`。下载原件和私有存储不提交，不以一个成功新原件扩张为 OpenLandMap 全平台或全部年度成功。
