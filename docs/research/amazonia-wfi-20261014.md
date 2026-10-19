# AMAZÔNIA-1 WFI L4 DN 接入核查

文件名按本轮接入批次 20261014；本次现场查询实际始于 **2026-10-13 14:11 UTC / 22:11 Asia/Shanghai**。所有观测查询使用 UTC，不把处理/发布时间当作观测时间。范围是 `AMZ1-WFI-L4-DN-1` 的 64 米多光谱原始数字量，不接 L2 或 L4-SR，不自动校准、裁剪、重采样或全色锐化。

## 原始产品、许可和元数据冲突

[INPE 官方产品页](https://data.inpe.br/dados/amazonia-1/)将 WFI 描述为四个 64 米可见光/近红外波段、10 bit 采集。L4 DN 由 L2 DN 正射校正得到，仍是数字量；L4 表面反射率 COG 是另一个产品。本轮固定使用 [`AMZ1-WFI-L4-DN-1`](https://data.inpe.br/bdc/stac/v1/collections/AMZ1-WFI-L4-DN-1)，保留蓝 BAND1、绿 BAND2、红 BAND3、近红外 BAND4 的各自完整原件。

实时 collection 声明 `Creative-Commons-Attribution-4.0-International`，链接 CC BY 4.0。保留 INPE / Brazil Data Cube 与 AMAZÔNIA-1 来源、collection、scene、原始资产 URL、处理级别、观测时间和访问时间。[实时 collection JSON](../../.verification/amazonia-wfi-20261014/collection.json)、[原始 scene JSON](../../.verification/amazonia-wfi-20261014/item.json)、[带实际时间/HTTP/hash 的收据](../../.verification/amazonia-wfi-20261014/network-receipts.json)均归档。

发布方 collection 和资产 `eo:bands` 同时声明 Int16、NoData -9999、scale 0.0001、最大值 10000。这与 DN 产品身份，以及此前独立读头的内嵌 NoData 0 / scale 1 不一致。保留全部原始声明，不静默修正 publisher JSON，也不直接把 raw DN × 0.0001 称为表面反射率。其它 scene 必须读取自身原件标签，不能由一份 BAND1 推出全部科学属性。

完整 BAND1 XML 也已直接获取，165,217 字节及 SHA256 与 publisher multihash 匹配。其投影字段为 UTM / WGS84，水平与垂直像元均 64 米；URL 目录却写 `LCC`，目录名不能代替真实 CRS。XML 的 `scaleFactor=0.9996` 是投影参数，不是 DN 辐射校准。见 [完整 XML](../../.verification/amazonia-wfi-20261014/band1.xml)、[XML 传输与 publisher 校验](../../.verification/amazonia-wfi-20261014/band1-xml-receipt.json)、[XML 科学字段](../../.verification/amazonia-wfi-20261014/xml-science-fields.json)。

## UTC 月份、地区和真实检索

实时 collection temporal extent 为 `2021-03-17T00:00:00.000000Z` 至 `2026-10-12T15:03:48.000000Z`；空间包络为 `[-82.67666122230501,-45.71144766691233,40.2992295692555,40.143081776204916]`。该包络不代表全球或连续覆盖。候选年限定 2021–2026，月份 1–12，2021 年 3 月以前拒绝；尚未发布的候选月份允许正常返回空列表。

公开研究 AOI 是 `[-74,-19,-73.8,-18.8]`，明确是本次核查区域，不会把它代入用户请求。实际 UTC 查询 `2026-10-01T00:00:00Z/2026-10-31T23:59:59.999999Z`、固定 collection、limit 20，返回三景：

- `AMAZONIA_1_WFI_20261012_040_018_L4`
- `AMAZONIA_1_WFI_20261007_040_018_L4`
- `AMAZONIA_1_WFI_20261002_040_018_L4`

北京 `[116,39,117,40]` 按完全相同的 collection / UTC 月份返回 0 景，没有换成其它地区、日期或传感器。原始响应见 [研究区域](../../.verification/amazonia-wfi-20261014/scene-area-october.json)、[北京空结果](../../.verification/amazonia-wfi-20261014/beijing-october.json)。软件 API 首页同样最多 20 景，更多页必须保持原始查询条件。

选定核查 scene 的 acquisition 是 `2026-10-12T15:03:48.000000Z`，created 是 `2026-10-13T00:03:38.137750Z`；path 040、row 018。`eo:cloud_cover=null` 不表示 0% 云量。Scene bbox `[-78.2795569252278,-21.909814575758997,-68.29449123089073,-13.58211198194212]` 只是足迹包络，不证明 AOI 有效像元或无云完整覆盖。

## 真实资产大小与初次失败

| 原件 | publisher `bdc:size` / 实际 HEAD 200 字节 |
| --- | ---: |
| BAND1 蓝 | 163,671,005 |
| BAND2 绿 | 166,037,389 |
| BAND3 红 | 165,196,705 |
| BAND4 近红外 | 185,621,213 |

四份均在 512 MiB 单原件限制内；这只是大小/HEAD 核对，不是四个波段已完整下载的声明。原始 collection/scene 共保留 17 个资产，XML 与 thumbnail 不能替代原始光学栅格验收。

初次普通完整 GET 返回 HTTP 200、Content-Length 163,671,005，实际在 **286.812 秒后仅收到 5,701,395 字节即 EOF**。长度校验失败，部分文件没有被当作完整原件。此尝试自然退出；定位 helper 的操作未找到仍运行的匹配进程，没有终止用户进程或代理。见 [真实失败收据](../../.verification/amazonia-wfi-20261014/initial-transfer-failure.json)。

之后保持同一 URL / 强 ETag `"6acd30b1-9c16bdd"`，按精确 HTTP 206 和 Content-Range 续传。38 个不超过 4 MiB 的缺失区间中，37 个完成；总共 5 次读取超时，只有 `[93781779,97976082]` 这个区间在两次尝试后仍未完成。再仅将它拆成 16 个 256 KiB 区间，其中一次超时后第二次成功，补齐阶段耗 62.359 秒。失败与补齐都保留，未换源、换产品、换 AOI 或修改代理/DNS。最终完整文件长度和 publisher SHA256 完全匹配：

`62ca359e7d7537f44b4e245d7eb00d086ced81187b83f6f38cbed37211734c11`

证据：[完整传输终态](../../.verification/amazonia-wfi-20261014/transfer.json)、[4 MiB 区间收据](../../.verification/amazonia-wfi-20261014/resume-range-receipts.json)、[该阶段真实失败](../../.verification/amazonia-wfi-20261014/resume-failure.json)、[256 KiB 补齐收据](../../.verification/amazonia-wfi-20261014/repair-range-receipts.json)。这是一次带实际失败和续传恢复的完整原件核查，不能描述为一次普通整件 GET 成功。

## 独立全像元解码

Rasterio 1.3.9 / GDAL 3.6.4 按条带遍历 **14,253 行 / 230,128,938 像元**，完整解码耗时 12.938 秒，不分配全图 NumPy 数组。

| 项目 | 完整实际 BAND1 |
| --- | --- |
| 网格 / dtype | 16,146 × 14,253，单波段 Int16 |
| CRS / 像元 | EPSG:32718，64 米 × 64 米 |
| 仿射变换 | `[64,0,161056,0,-64,8489344]` |
| 原始投影范围 | `[161056,7577152,1194400,8489344]` |
| 编码 | Deflate 条带，RowsPerStrip=1，无 overviews，不是 COG |
| 内嵌 NoData / scale / offset | 0 / 1 / 0 |
| raw / valid 范围 | 0–1023 / 50–1023 |
| NoData 像元数 | 74,278,897 |

六个原始样本：`(0,0)=0`、`(8073,7126)=628`、`(16145,14252)=0`、`(4036,3563)=1023`、`(12109,10689)=314`、`(10000,10000)=785`。两处角点 0 对应真实内嵌 NoData，其余样本为未校准 DN。完整直方图和原始 STAC 波段声明同时保留在 [独立解码收据](../../.verification/amazonia-wfi-20261014/independent-decode.json)。完整原件在同一验证目录，不纳入代码或安装包。

全图 Int16 数组约 438.94 MiB，加上压缩原件约 595.02 MiB；不能把两者同时常驻假定为安全。每个实际行条带仅 32,292 原始解码字节。按 native 每样本 8 字节的保守计费，采样 768 行约 94.61 MiB，在现有限额内；不放宽 stripped 单边、像元总量、分块或内存限制。精确预算和网络失败计数见 [核查汇总](../../.verification/amazonia-wfi-20261014/verification-summary.json)。

## 实施和验收边界

新增专属 [amazonia_wfi.rs](../../crates/geod-runtime/src/datasets/amazonia_wfi.rs)提供 `definition()` / `resolve()`，dataset ID 为 `amazonia-wfi-dn`，唯一图层 ID 为 `wfi-ms`。固定 L4 DN collection、UTC 日历月份和原始 AOI；单位保持 raw DN，图层不注入缩放或替代 NoData。四项模块测试已实际执行通过，覆盖闰月、跨年、前归档/非法日期/其它产品拒绝，以及国家/命名区域/季节拒绝和原值科学声明；命令为 `cargo test -p geod-runtime datasets::amazonia_wfi::tests -- --nocapture`，4 passed、786 filtered out。见 [测试收据](../../.verification/amazonia-wfi-20261014/module-tests.json)。共享注册、前端目录和中文由主任务处理；另提供包含全部可见模块文字的 [24 项中文映射 fragment](../../.verification/amazonia-wfi-20261014/zh-CN-fragment.json)，供主任务合并。

独立下载和软件自身下载是两条证据。[软件验收 helper](../../.verification/amazonia-wfi-20261014/native_fresh_smoke.py)使用主任务合并后的固定编译原件，exe SHA256 为 `2457521848c9f1ee6dcbdd6be10d407674b4518dd6f42815498433f8b47dde84`，于 **2026-10-13 14:52 UTC** 完成。它只调用正常 native HTTP API、创建新私有存储并自行下载原件，失败时仅使用正常 jobs retry；不把研究原件注入软件下载目录，不手工拼接软件 partial。三次实际尝试全部保留：

| 尝试 | 真实传输模式 / 起点 | 本次耗时 | 最终累计字节 | 终态 |
| --- | --- | ---: | ---: | --- |
| 1 | fresh / 0 | 266.391 秒 | 8,306,446 | `error decoding response body`，failed |
| 2 | resumed / 8,306,446 | 208.203 秒 | 48,496,115 | `error decoding response body`，failed |
| 3 | resumed / 48,496,115 | 234.250 秒 | 163,671,005 | succeeded，完整 SHA256 与 publisher / 独立解码一致 |

总软件下载观测耗时 **709.0 秒**，不是一次整件请求成功。最终 native 检查得到同样的 16,146 × 14,253、Int16、EPSG:32718、64 米网格、NoData 0；六个原始/NoData 像元全部等于独立全文件解码结果。软件生成真实 **768 × 678** 首波段灰度 PNG，显示拉伸范围 `[96,1023]` 只用于预览，不改变 DN。私有 kernel 重启后 project / snapshot / inspection / 六个像元原样重开，仍仅 1 个原下载 job，没有第二次传输。helper 的私有 runtime 已关闭。

证据：[native 完整验收与三次尝试](../../.verification/amazonia-wfi-20261014/native-acceptance.json)、[真实 native 北京空列表](../../.verification/amazonia-wfi-20261014/native-beijing-empty.json)、[原始资产 metadata 未改写的 snapshot](../../.verification/amazonia-wfi-20261014/native-snapshot.json)、[native 栅格/PNG 检查](../../.verification/amazonia-wfi-20261014/native-inspection.json)、[六个 raw 像元](../../.verification/amazonia-wfi-20261014/native-pixels.json)、[重启重开收据](../../.verification/amazonia-wfi-20261014/native-reopen.json)、[实际软件预览 PNG](../../.verification/amazonia-wfi-20261014/native-preview.png)。这里的 `nativeAccepted=true` 只证明完整 BAND1 软件下载/检查/重开；BAND2–4 目前仅 HEAD/元数据核查，桌面和浏览器在线地图仍需主任务独立记录。

此类 stripped Deflate 原件没有 overviews。在线预览会读取实际原文件，OL 的 256 行合并逻辑块在该 16,146 像素宽的 Int16 文件中为 8,267,752 字节，需保留逻辑读取上限和每个 HTTP 分段不超过 4 MiB / 30 秒的限制。独立完整文件成功并不等于任意网络下在线地图已通过。若来源中途 EOF 或超时，应展示真实错误、允许受限重试，不更换数据产品或用缩略图冒充原件。
