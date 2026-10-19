# GLC_FCS30D 接入前置与实际解码预算

核查日期：2026-10-13。状态：**原件下载和独立科学解码已验证，软件原生渠道尚未接入**。

本记录解释为什么不能仅放宽波段上限就将渠道标记为可用。此次核查只读取既有公开证据和代码，未改动运行时代码、启动界面或读取私密配置。本文不包含发布平台的账号值。

## 已验证的原件

既有实测证据位于 [.verification/glcfcs-probe-20261013/report.md](../../.verification/glcfcs-probe-20261013/report.md)、[transfer.json](../../.verification/glcfcs-probe-20261013/transfer.json)、[independent-decode.json](../../.verification/glcfcs-probe-20261013/independent-decode.json)、[layout-tags.json](../../.verification/glcfcs-probe-20261013/layout-tags.json) 和 [acceptance.json](../../.verification/glcfcs-probe-20261013/acceptance.json)。这些是本地验收材料，不应当误认为已经公开上传到代码仓库。

| 项目 | 实际证据 |
| --- | --- |
| 原件 | `GLC_FCS30D_20002022_V1_maps/GLC_FCS30D_20002022_E0N25_Annual.tif` |
| 传输 | 发布者显式 FTPS，控制连接验证 TLS，数据连接使用 `PROT P` |
| 原件字节 | 21,832,902；完整传输成功，服务器返回 `226 Transfer complete.` |
| SHA-256 | `46b7d3b162c93a88b397d9ff6083a527b74abc20f71fe3daaae9f512dce50fe0` |
| 网格 | 18,554 × 18,554；EPSG:4326；实际经纬度像元间距 0.0002694945852358564° |
| 实际范围 | `[0, 19.99979746553392, 5.00020253446608, 25]`，保留原件边界微小越界值 |
| 存储 | LZW；256 × 256 tiled；`SamplesPerPixel=23`；`PlanarConfiguration=1`；UInt8 |
| 年份 | 发布者指南声明 band 1 = 2000，依次至 band 23 = 2022；原件没有嵌入波段描述 |
| 分类 | 发布者图例定义的土地覆盖类别，不是反射率或可用于 RGB 合成的光谱波段 |
| 填充值 | 发布者指南声明 0、250 为 filled values；原件没有 TIFF NoData 标签 |
| 独立解码 | 所有 5,329 块、23 年度平面均已读取；合计 7,917,771,068 样本；所有观测类别均存在于发布者图例 |

此前单次完整传输耗时 1.859 秒，约 11.7 MB/s，只代表该工作站、该路由和当时这一个文件。没有发布者的单文件校验和可供比对，以上 SHA-256 记录实际收到的原件字节。

指南与图例证据见 [publisher-legend.json](../../.verification/glcfcs-probe-20261013/publisher-legend.json)、[user-guide.pdf](../../.verification/glcfcs-probe-20261013/user-guide.pdf)。指南 MD5 `25cfd070e8a38ab0e534da939e99856f` 与发布者记录一致。图例颜色属于发布者外部表现信息，不能描述成 TIFF 内嵌颜色表。

## 当前代码存在三个独立阻碍

### 1. 现有下载是 HTTPS 请求体系

[features.rs 的 public_url](../../crates/geod-runtime/src/features.rs#L177)要求公共 HTTPS、443 端口，不允许 URL 内凭据；[stac.rs 的 public_url](../../crates/geod-runtime/src/stac.rs#L238)继续应用该要求。[lib.rs 的 download_response](../../crates/geod-runtime/src/lib.rs#L1104)返回 `reqwest::Response`，现有 [transfer.rs](../../crates/geod-runtime/src/transfer.rs)断点机制绑定 HTTP 状态、ETag、Range 与 Content-Range。

当前 Cargo.lock 未包含 FTP/FTPS 传输库。不能将 FTPS 地址伪装成 HTTPS，也不能为一个产品放开通用自定义数据源的公共地址验证。应新增固定发布者范围的原件传输适配器，并独立保存传输与来源凭证。

### 2. 原生和地图两处都拒绝 23 个样本

- [stac/raster.rs 的 decode_identified](../../crates/geod-runtime/src/stac/raster.rs#L388)仅接受 1–16 samples per pixel。
- [public-dataset-preview-source.js 的 validateProductRasterImages](../../prototype/src/public-dataset-preview-source.js#L129)也仅接受 1–16 samples per pixel。[ProductRasterSource](../../prototype/src/public-dataset-preview-source.js#L182)目前固定选择 band 1，并使用原始值灰度样式。

这张原件的空间网格符合当前 tiled 轴长 131,072、空间像元 80 亿和 chunk index 131,072 的限制。阻碍不是旧 SCL 读取器的 64M 像元限制，也不是原件下载超过 512 MiB。

### 3. 全景缩略图会超过累计解码预算

[stac/raster.rs 的 chunk](../../crates/geod-runtime/src/stac/raster.rs#L606)已按块解码，单块检查保守使用 `width × height × bands × 8`，上限 128 MiB。[preview](../../crates/geod-runtime/src/stac/raster.rs#L770)还会将每个读过的块累计，超过 512 MiB 就拒绝。

实测原件没有 embedded overviews。按当前预览源码映射复算：无论生成 160 × 160 缩略图，还是 768 × 768 预览，都将触及全部 73 × 73 = **5,329 个块**。缩小输出尺寸没有消除遍历原始块的成本。

| 预算项 | 计算与结果 |
| --- | --- |
| 一个完整块真实 UInt8 输出 | `256 × 256 × 23` = 1,507,328 B，约 1.44 MiB |
| 一个完整块当前保守预算 | `256 × 256 × 23 × 8` = 12,058,624 B，11.5 MiB |
| 512 MiB 可容纳的连续完整块 | `floor(512 × 1024² / 12,058,624)` = 44；第 45 个完整块超限 |
| 全部实际网格的 UInt8 样本 | `18,554² × 23` = 7,917,771,068 B，约 7.37 GiB |
| 全部实际网格的当前保守累计预算 | `18,554² × 23 × 8` = 63,342,168,544 B，约 58.99 GiB |
| 将所有边缘块也按完整 256² 估计的上界 | `5,329 × 256² × 23 × 8` = 64,260,407,296 B，约 59.85 GiB |

以上累计量是处理工作量/预算，**不是同时驻留内存**。当前 reader 不会一次分配 7.9 GB 全图。边缘块实际宽/高为 122；因此 59.85 GiB 只是完整块估计上界，实际有效网格预算为 58.99 GiB。

现有 [TIFF 0.11.3 Decoder](https://docs.rs/tiff/0.11.3/tiff/decoder/struct.Decoder.html)和本机该版本源代码支持 `Multiband`、`read_chunk`、`read_chunk_bytes`、`read_chunk_to_buffer`。23 年度平面不要求更换 TIFF 库。但 `PlanarConfiguration=1` 表示年度样本在同一压缩块内交错存储，选定某一年仍要解压这个块的全部 23 年；可以只保留该年的像元，不能声称仅解压一个物理平面。

## 建议实施范围

### 固定发布者 FTPS 原件适配器

1. 固定已审查发布者主机、目录、文件身份和产品时期；发布者访问参数在运行时处理，禁止进入代码、源 URL、日志和持久化凭证记录。
2. 认证前升级显式 TLS，控制连接与 `PROT P` 数据连接均验证证书和主机；不允许明文 FTP 回退或关闭证书校验。
3. 检查被动数据连接的目标地址与端口，拒绝内网、回环及未经批准的第三方跳转；DNS 解析和连接目标需要与当前公共源约束达到相同目的。
4. 有界流式写入新建暂存文件，检查大小、签名、SHA-256、磁盘空间，完整成功后原子落盘；失败和取消时不发布 partial。
5. 独立验证超时、取消、重试和崩溃恢复。FTP `REST`、远端 SIZE/修改时间不等价于 HTTP 强 ETag，不能直接重用 HTTP 断点信任规则；前期允许验证完整文件后安全重新下载。
6. 核查代理支持和 Windows 打包依赖，避免让 FTP 数据连接隐式绕开用户网络设置。

[SuppaFTP 官方文档](https://docs.rs/suppaftp/latest/suppaftp/)提供显式 FTPS、TLS、流式传输和异步支持，可作为候选。其适配器、被动连接边界、具体版本及打包依赖尚未在本项目验证；现有 reqwest 的系统代理支持不自动覆盖新的 FTPS 库。

### 视口分块读取的年度分类图层

1. 将 2000–2022 年份映射作为发布者声明，与不可变原件、指南版本和来源关联；默认年与用户选择都必须可见。
2. 图层按当前视口请求有限原始块；每块解压后提取所选年度样本，有限并发与缓存，不生成或持有整个年度网格。
3. 使用发布者类别颜色和完整图例；明确 0/250 为外部声明的填充值，原件没有 embedded NoData。不把年度当 RGB 光谱波段，不对类别做双线性插值。
4. 全景预览需要单独的有界方案，例如后台分块生成可追溯的预览缓存；缓存是显示派生物，不能覆盖原件或伪称原始 overview。不直接删除现有 512 MiB 防护。
5. 用户流程仍保留同一个 Explore 上下文：区域 → 数据源 → 自动检索 → 文件范围/可用状态 → 保存下载 → 年度分类图层 → 原始像元与成果。不得另建孤立的数据源表单。

合理的先行范围是：下载前显示文件范围，完整下载后提供本地视口分类预览、年份切换、图例和像元检查。若需下载前在线预览，还要额外核查 FTPS 有界读取与取消完成语义；当前 [STAC HTTPS preview](../../crates/geod-runtime/src/stac/preview.rs)依赖精确 Content-Range，不可直接套用。

## 允许标记为已接入前的验收

- 固定目录/文件校验、TLS/数据通道验证、异常大小、超时、取消、错误认证、磁盘不足和失败清理。
- 同一真实原件完整下载、大小和 SHA readback、原始像元 23 年向量与独立解码证据一致。
- 2000、2022 及中间年份切换，填充值与分类色正确；视口改变不会读全图，缓存/并发及取消有实际预算记录。
- 桌面和 Explore 地图结果、工程/成果保存、重开后来源与年份保持一致。
- 年度映射、实际网格、填充值来源、原件 metadata absence、许可和引用明确显示，避免虚构官方嵌入信息。

此前 [acceptance.json](../../.verification/glcfcs-probe-20261013/acceptance.json)明确 `nativeDownloadAccepted=false`、`nativeDisplayAccepted=false`。在上述工作完成前，目录继续标记为计划接入，不以独立 Python 下载成功替代软件链路验收。

## 发布者参考与替代边界

- [CASEarth 发布者页面](https://data.casearth.cn/thematic/glc_fcs30/314)。
- [作者 Zenodo v2 记录](https://zenodo.org/records/15063683)，公开 metadata 声明 CC BY 4.0；该记录提供区域 ZIP 包及指南/图例，没有此次单独 TIFF 的对应下载项。记录版本与原件文件名中的 `V1` 分别保留，不将其混为同一字段。
- [作者论文](https://essd.copernicus.org/articles/16/1353/2024/)描述 5° tiles 与时间序列。论文内容不替代实际原件网格和已下载文件验收。
- 既有核查未验证同一个单独 TIFF 的匿名 HTTPS 镜像。作者 [GLC_FCS30 2015 记录](https://zenodo.org/records/3986872)是不同产品，不能当成 GLC_FCS30D 2000–2022 镜像。
