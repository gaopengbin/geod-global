# 下一批四项独立 Planetary Computer 产品 — 2026-10-14

本次选择 HREA 电力可达性模型、Impact Observatory 生物多样性完整度、NOAA 海洋热含量异常与 NOAA NClimGrid 月度气候。四者具有独立研究对象、算法或观测来源；不是把既有 Landsat、Sentinel、MODIS NDVI 改名，也不把同一气候产品的四个变量重复计数。

这是有限版本接入：每项保留一个已核对的真实 Item。Collection 和 Item 原样存于 `crates/geod-runtime/src/datasets/next-planetary-reviewed-{collections,items}.json`，不修改发布方资产、范围、NoData、时间或许可。完整下载回执、研究原件和独立解码仅保存在忽略的 `.verification/next-ten-products-20261014/catalog-sources/`；研究原件不注入软件数据目录。

## 发布范围与科学意义

| 接入 ID | 产品与固定范围 | 选定原件 | 原始网格 / 原值 | 许可边界 |
| --- | --- | --- | --- | --- |
| `pc-hrea` | HREA v1，香港 2019 年居民点通电可能性模型 | `HREA_Hong_Kong_2019_v1` / `lightscore` | EPSG:4326，2,183 × 1,469，约 1 角秒，Float32 模型概率；真实有效零保留 | 原 Collection `CC-BY-4.0`，注明研究团队、产品、版本与来源 |
| `pc-biodiversity` | Impact Observatory / Vizzuality v1，2020 年北京周边已发布分幅 | `bii_2020_41.9239854959007_114.35276577801187_cog` / `data` | EPSG:4326，7,992 × 7,992，0.0008983152841195215°，Float32 原始模型评分 | 原 Collection `CC-BY-4.0`，保留作者与白皮书归属 |
| `noaa-ohc` | NOAA OHC CDR，2020 年第四季度、0–700 米海层，相对 WOA09 的热含量异常 | `ocean-heat-content-2020-Q4-700m` / `heat_content` | EPSG:4326，360 × 180，1°，Float32，`10^18 joules`；NaN、负异常与有效零保留 | 原 Collection `proprietary`；链接 NOAA Use Agreement，不能擅改成 CC |
| `noaa-nclimgrid` | NOAA NClimGrid，2022 年 9 月美国本土；站点观测推算的月度气温、降水 | `nclimgrid-202209` / `tavg`、`tmax`、`tmin`、`prcp` | EPSG:4326，1,385 × 596，0.04166667°；Float32，气温摄氏度、月度降水毫米；NaN 保留 | 原 Collection `proprietary`；NOAA 使用与分发约束及 DOI 引用保留 |

HREA 是年度模型概率，不能称夜光亮度、用电量、供电接入登记或实测停电。模型只在居民点像元有值；香港 TIFF 实际外缘和 Item bbox 略有差异，二者均不改写。[官方 HREA 产品说明](https://planetarycomputer.microsoft.com/dataset/hrea)。

生物多样性完整度使用人为压力、丰度和组成相似性建模。它不是物种计数或实地调查。实际 TIFF **没有标量 NoData 标签，但每个影像层均有独立内部 1 位有效性掩码**，比例 1、偏移 0。掩码排除的像元仍可能存放有限原值（例如 0.6769765、0.7444649），不能把原值有限当作科学有效。标量 NoData 与原始有效性掩码分别记录；不往 TIFF 注入新的 NoData 声明。Item 渲染 URL 中 `0.97*(data_b1**3.84)` 作为原始元数据保留，但软件不将其应用到存储原值，也不把视觉表达式称独立定标。局地质量仍受模型适用性限制。[作者白皮书](https://ai4edatasetspublicassets.blob.core.windows.net/assets/pdfs/io-biodiversity/Biodiversity_Intactness_whitepaper.pdf)。

OHC 是分层海洋能量异常，正负均有意义，单位不能换成摄氏度或热通量。当前为 2020-10-01 至 2020-12-31 固定历史季度，不声称当前预测、其他海层或整个档案已验收。COG 由发布平台从 NOAA NetCDF 制作，因此是发布衍生原件而非原 NetCDF 的逐字节镜像。[NOAA 产品页](https://www.ncei.noaa.gov/products/climate-data-records/global-ocean-heat-content)。

NClimGrid 来自 GHCN-D 地面观测，气温不是卫星地表温度；四变量属于同一个月度气候产品。当前仅固定美国本土 2022 年 9 月，不扩大到日档案、阿拉斯加、夏威夷或保证最终质量状态。NOAA 的原始发行格式是 NetCDF / TXT，这里是 Planetary Computer 的发布 COG。[NOAA 元数据与约束](https://www.ncei.noaa.gov/access/metadata/landing-page/bin/iso?id=gov.noaa.ncdc:C00332)。

## 匿名访问与接入边界

四项 Azure Blob 原始 URL 在匿名直读时实际返回 404 或 409；官方匿名 PC 签名端点给出短期只读能力后可下载。临时 SAS 不是新的用户账户、云导出或付费计算，也不修改用户凭据。它只用于当前传输；没有把 SAS 写进 Item、Collection、代码、快照、设置或研究回执。[Planetary Computer 官方数据访问说明](https://planetarycomputer.microsoft.com/docs/concepts/sas/)。

原生 `next_planetary::access_scope` 先验证完整不可变的源证明、Item、精确选定 layer、canonical href，再返回四个固定账户和容器。`planetary-anonymous-sas` 传输回执保存同一 canonical href 和适配器名称，不保存签名。其他原始资产元数据仍保留，但未选择的数值或视觉资产不因此获得传输权限。保持原有 512 MiB、维度、像元、预览网络及解码限制。

精确 Collection / Item URL：

- [HREA Collection](https://planetarycomputer.microsoft.com/api/stac/v1/collections/hrea) · [香港 2019 Item](https://planetarycomputer.microsoft.com/api/stac/v1/collections/hrea/items/HREA_Hong_Kong_2019_v1)
- [BII Collection](https://planetarycomputer.microsoft.com/api/stac/v1/collections/io-biodiversity) · [2020 北京周边 Item](https://planetarycomputer.microsoft.com/api/stac/v1/collections/io-biodiversity/items/bii_2020_41.9239854959007_114.35276577801187_cog)
- [OHC Collection](https://planetarycomputer.microsoft.com/api/stac/v1/collections/noaa-cdr-ocean-heat-content) · [2020 Q4、700 米 Item](https://planetarycomputer.microsoft.com/api/stac/v1/collections/noaa-cdr-ocean-heat-content/items/ocean-heat-content-2020-Q4-700m)
- [NClimGrid Collection](https://planetarycomputer.microsoft.com/api/stac/v1/collections/noaa-nclimgrid-monthly) · [2022 年 9 月 Item](https://planetarycomputer.microsoft.com/api/stac/v1/collections/noaa-nclimgrid-monthly/items/nclimgrid-202209)

## 已取得的完整原件证据

四项完整研究原件及 NClimGrid 四层均已独立下载、完整哈希和全像元解码。软件原生验收另列，不能由研究原件代替。完整哈希、全像元独立解码和采样按每项 `*-original-acceptance.json` 留档。

| 研究原件 | 字节 | 完整 SHA-256 | 全像元独立解码 |
| --- | ---: | --- | --- |
| HREA lightscore | 343,602 | `c3a8d91858e609e06dfb701901bfb9582b7d472b173f1ad4e3c413ee84237718` | 3,206,827；有效 108,309，有效零 306，NoData 3,098,518；范围 0–1 |
| Impact Observatory BII | 295,765,273 | `a76ee54f166d68ce8ad6d61b6a58f262e04460e0dc9618a9a723404d55d5e843` | 63,872,064；有效性掩码内 48,527,607，无有效零；有效范围 0.101001956–0.974067450。掩码排除 15,344,457，标量 NoData 数为 0 |
| NOAA OHC heat_content | 157,352 | `f9892384908eeb92abcde4d8bcfa8805b23d584e380e1d34efd9da12a17b7c6d` | 64,800；有效 41,458，有效零 743，负值 10,093；范围 -59.1701622–66.7835617 |
| NOAA NClimGrid tavg | 1,173,286 | `cf3c7b6511ef8a3887a4c7d5c031d7a969a4b5afb8825395908e8219a346af19` | 825,460；有效 469,758；范围 0.849609375–33.5302734375 |

NClimGrid 另外三层也已完整独立下载解码：`prcp` 1,596,625 B / SHA `fee7d1e32efb51047503d5a72cd434b10bcdc7fbb1b1edf301529385611fa29a`（含 64 个有效零）；`tmax` 1,228,740 B / SHA `a66086f45dca5b59828d2f85e5e81daa732a871ac1936bf97d6fdb2824c56083`；`tmin` 1,209,928 B / SHA `7bc3e8e1648225f05033e601ed7955959c4d92f76ab94cda1aaa67ddaf653444`（一个真实负值 -3.2001953125°C）。各 825,460 像元。具体回执 `noaa-nclimgrid-extra-originals.json`；不将这三层重复计为新渠道。

## BII 内部有效性掩码修复与实际原件回归

原 TIFF 的主影像和四级缩小图都有配对 1 位有效性掩码。既有 `tiff` 解码器拒绝 `PhotometricInterpretation=4`；本次沿用原有只读 tag view，在同时确证 `NewSubfileType & 4`、`BitsPerSample=1`、单样本和 Photo=4 时，将解码视图中的 Photo 看作 BlackIsZero 以读取打包有效性位。**原件、像元流、mask 位和元数据均不改写**，没有把 mask 写成新标量 NoData。

PNG 从与选定影像层尺寸完全一致的原 mask 采样，mask=0 的像元透明并从显示拉伸范围中排除。mask=1 的有效零仍显示；标量 NoData 独立处理。重复 mask、错尺寸、非 1 位 mask、非 MSB 位顺序、缺配对层或超预算均拒绝预览。保持 16 层预览扫描、128 IFD 只读视图、4,096 个 IFD 标签、128 MiB chunk、512 MiB 累计解码与 30 秒限制。

真实 295,765,273 B 原件回归使用完整 SHA 绑定，通过 1.61 秒原生读取和 PNG 生成。独立 Rasterio 对原件 999 × 999 factor-8 层和配对 mask 采样，**768 × 768 RGBA 逐通道完全一致，差异 0**：461,029 像元有 alpha，128,795 像元透明；PNG SHA `dd1ad91d61bd81a8e0ca9cb3bd476dee23775d7608673a4d6ee561b3b06d2118`。源文件 hash 不变；海区有限原值 `0.6769765019416809` 的标量 NoData 标记仍为 false，且原 PNG alpha 按实际 mask 为透明。已实际查看 PNG，陆地纹理可见，海区由原 mask 排除。

相关原生单元检查 23 项通过、2 项真实文件回归默认忽略；本次 BII 真实原件回归已另外显式运行通过。覆盖 mask 排除有限值、有效零、标量 NoData、非字节对齐行边缘，以及重复/错尺寸/非 1 位 mask 拒绝；数据集模块两项不可变源证明/科学量检查通过。回执 `bii-mask-native-regression.json`。这是实际原件解码回归，软件 UI 新建工程、下载与重启重开仍由下一节的真实新 store 回执独立验收。

## 软件 UI 新 store 验收

主任务在真实 UI 分类选择、检索、地图、保存、完整下载和成果打开后，本组用 GET-only 审计读取软件新 store。审计不创建工程、不请求二次下载，也不注入研究原件。按真实 job 的源快照识别本四项和同批 ETH 原件，校验软件实际落盘字节与 SHA、六个原值/标量 NoData、PNG 以及工程关系；BII PNG 额外比对同一软件原件内部 mask 和匹配层，HREA PNG 比对同一软件原件完好主影像。重启由主任务统一执行，审计逐字段比对原 job、snapshot、inspection、pixels、project 及无新增传输。

HREA 修复后的最终 runtime `d53a94ef14b5d25b1328556ae3b3a984ddb274004cff2a4572859cf11576b992` 对本组五件完成读取：**468,571,872 B 软件实际原件**，五份 SHA 与独立研究原件一致，30 个原始值/标量 NoData、五个 PNG、源快照与工程关联均通过。此时全新 UI store 共 11 个成功 job、11 个工程。基线 `.verification/next-ten-products-20261014/catalog-sources/ui-verification-before-restart/summary.json`；每项 `*-job`、`*-snapshot`、`*-inspection`、`*-pixels`、`*-project` 和实际 PNG 分开保留。软件原件没有由研究文件拷贝或注入，子任务没有使用 POST。

软件 HREA 768 × 517 PNG 仍与同一软件原件主影像逐 RGBA 一致，13,491 有效 alpha；软件 BII 768 × 768 PNG 与原件配对缩小图/内部 mask 逐 RGBA 一致，461,029 有效 alpha、128,795 透明。所有五件实际 PNG 已检查；NOAA 海洋与陆地气候分布、ETH 地区冠层、HREA 居民点和 BII 陆地结构可见。软件读取结果不扩大固定年份、分幅、许可或科学质量范围。

**BII 额外原始块局限保留：**手动读取 `column=7689,row=4010` 的原生接口返回 HTTP 400 / `no lzw end code found`；独立 GDAL 原件读得有限原值 `0.7444648742675781`，但原始有效性掩码为 0，标量 NoData 为 false。严格解码器拒绝该源块，不把 tolerant GDAL 读取当作原生成功，也不宣称所有 base chunks 均可原生解码。负向回执 `pc-biodiversity-source-masked-raw-limit.json` 保留。BII 六个独立代表坐标取原始独立报告的六个不同点，其中两个 mask 排除的有限值与四个有效点均原生读取通过；PNG 确实应用原 mask，而不是靠更换采样掩盖预览问题。代表六点通过不等于全 base 每个 chunk 均可原生读取。

首次 GET-only 审计在这个额外 mask 排除点返回 400，整次脚本以 exit code 1 结束；该失败没有被称作通过。第一次 `before-restart/summary.json` 被后来成功重跑的同路径摘要覆盖，在发现后依据本轮捕获的真实工具输出忠实恢复初始 BII 失败行，**明确标注 recovered、不声称原初始文件逐字节未覆盖**。恢复证据 `.verification/next-ten-products-20261014/catalog-sources/ui-verification-initial-failure-400/summary-recovered-from-tool-output.json`；真实 400 负例响应另行保留并在后续每次审计重测。替换代表六点仅覆盖可读原块，不改变这个失败事实、原文件、mask、PNG 或 strict decoder。

主任务随后以**同一最终 runtime SHA** 标准重启（PID 188752），本组再次只用 GET 审计：全局 11 个 job ID 和 11 个 project ID 完全相同；本组五个快照、五个 inspection（含 PNG 数据）、30 个原值/标量 NoData 与五个工程逐字段一致。HREA 和 BII 独立 RGBA 检查再次通过；额外 BII 400 负例仍明确拒绝，负例 JSON 也与重启前完全一致，未计为第七个通过采样点。无新增传输、重连或数据注入。最终回执 `.verification/next-ten-products-20261014/catalog-sources/ui-verification-after-restart/summary.json`，`restartObjectsIdentical=true`、`noNewTransfers=true`。本组有限产品闭环验收通过，已明确保留源文件损坏块与科学有效性边界。

### 真实 UI 发现的 HREA 缩小图损坏

HREA 已从真实 UI 保存并完整下载、SHA 和原值通过，但原生成果预览暴露 `no lzw end code found`，不能以研究 Rasterio 可读取或 raw sample 成功替代 PNG 验收。逐层用当前原生解码器核对全部 chunks：主影像 54/54 正常；factor-2 的 chunk 8 缺 LZW end code；factor-4 的 15/15 正常；factor-8 的 chunk 2 同样损坏；factor-16 的 2/2 正常。回执 `hrea-native-ifd-audit.json`。

修复只绑定上述完整 SHA、2,183 × 1,469 网格、area、EPSG:4326、Float32 和实际 TIFF NoData 声明，直接从已完整解码的主影像生成 PNG，不放宽 LZW 校验、不重写损坏的缩小图，也不建立未知文件的通用容错。全部主影像 3,206,827 像元，保守 sample 解码估计 25,654,616 B（约 24.47 MiB），保持原 128 MiB chunk、512 MiB 累计及 30 秒限制。

需要区分三个真实值：TIFF 原始 tag 字符串 `-3.39999999999999996e+38`；API 如实保留的 Float64 声明 `-3.4e38`；实际存储 Float32 填充值 `-3.3999999521443642e38`。原值读取按存储精度判断 scalar NoData，声明元数据不被换成存储值。软件数据集定义和原 publisher Item 均没有因此改写。GET-only 审计已分开检查声明与存储 fill。

真实完整文件回归 0.50 秒通过，768 与 160 PNG 各与独立 Rasterio 主影像最近邻采样逐 RGBA 完全一致：768 × 517 有效 alpha 13,491、透明 383,565；160 × 108 有效 alpha 610、透明 16,670，二者显示范围均为 0–1，差异 0。原 SHA、原值 `0.9527872204780579` 和原标量 NoData 不变。实际 PNG 已查看，居民点可见，非居民点由真实 NoData 透明处理。回执 `hrea-native-base-regression.json`；UI 活跃 BII 任务未被子任务停止，运行中的旧 runtime 需由主任务在安全时点替换后再做最终软件成果重验。

## 本轮筛除的候选

HGB 是 129,600 × 52,201 全局原件，尺寸超出当前可验收原生范围，不以局部请求或新标签替代完整原件。Deltares 水可用性和洪水集合的真实资产是 NetCDF / kerchunk；没有以服务渲染图或凭空生成 TIFF 冒充当前原始栅格闭环。这些研究元数据和访问探测同样留在私有证据目录，不晋升为本批已接入。
