# 2026-10-14 下一批十项公开产品接入验收

这一批新增 **10 个独立具体产品**：GHSL 三个建成环境/人口统计量、ETH 冠层高度、HydroSHEDS 汇流积累、ETOPO 大地水准面分离值及四个有限 Planetary Computer 科学产品。当前目录为 **133 条：91 Ready、6 Auth、36 Planned**。尚待对接和账号验收仍为 42 条，没有把平台家族或未接产品消掉；同一产品的年月、波段和变量没有重复计数。

**本批有限原件闭环已通过**：实际 Explore 分类选择、检索、十一件所选在线地图、保存、软件完整下载、成果地图/原值和标准重启后的工程重开均有真实回执。十个产品对应 **11 个 succeeded jobs、11 个工程，共 1,429,629,635 字节**；每件完整 SHA 与独立原件一致，66 个代表原像元/旗标和 PNG 在重启后不变，没有新增传输。HydroSHEDS ACC/ACA 共一个产品、两件原件，其余九项各一件；研究 TIFF 未复制或注入软件 store。这不表示全部索引、地区、年份、变量或原生压缩块都已验收，已知局部限制见下文。[公开验收摘要](next-ten-public-channel-acceptance-20261014.json)。

[前一批十项验收](new-public-channel-ten-products-20261014.md)的 541,137,211 字节、十个 UI 工程和重启回执独立保留，本页不覆盖或复用为这轮成功证明。

## 十个产品与有限发布范围

| 稳定 ID / 产品 | 当前接入范围与原始科学量 | 实际原网格 / 存储 | 许可与署名 |
| --- | --- | --- | --- |
| `ghsl-built-surface` / GHSL 总建成区面积 | R2023A V1.0、2020 参考年份、全球 30 角秒；每像元平方米总量 | EPSG:4326，43,201 × 21,384，UInt32；scale 1 / offset 0；原 TIFF 无 NoData 标签 | JRC 管理目录 CC BY 4.0，保留产品 DOI 与作者 |
| `ghsl-built-volume` / GHSL 总建成区体积 | 同版本和参考年份；每像元立方米总量，与面积产品为独立量 | EPSG:4326，43,201 × 21,384，UInt32；scale 1 / offset 0；无 NoData 标签 | CC BY 4.0；不将该值解释为单栋实测高度 |
| `ghsl-population` / GHSL 常住人口 | 同版本和参考年份；每像元人口模型数量，不是实时人口或密度 | EPSG:4326，43,202 × 21,384，Float64；小数和有效零保留；无 NoData 标签 | CC BY 4.0；保留 GPW/GHSL 模型及引用 |
| `eth-canopy-height` / ETH 冠层高度 | 2020 固定 CHM；GEDI RH98 监督、Sentinel-2 生成的冠层顶高预测；未接 STD 不确定度 | EPSG:4326，3° 分幅、36,000²，0.3 角秒，UInt8 米；有效 0、原 NoData 255 | 作者 CC BY 4.0；保留数据及论文 DOI；名义 10 m GSD 不变成恒定 10 m 地理像元或原始激光测量 |
| `hydrosheds-accumulation` / HydroSHEDS 汇流积累 | v2r0 1 角秒美洲部分合格文件；ACC 为包括当前格的汇入像元数，ACA 为纬度修正的 km²；两层共一个产品 | EPSG:4326，Float64，scale 1 / offset 0，原 NoData 1.79e308 | CC BY 4.0；保留 HydroSHEDS/DLR/WWF/Confluvio 署名和发布方不要整体镜像原件的额外请求 |
| `etopo-geoid` / ETOPO 大地水准面分离值 | ETOPO 2022 v1，EGM2008 相对 WGS84 椭球的垂直分离值；不是地表/海底高程 | EPSG:4326，15 角秒，3,600²，Point 注册，Float32 米；无 NoData 标签，负数和零保留 | NOAA 元数据 CC0 1.0 / 原件 NGA 公共领域声明；保留来源和不用于导航约束 |
| `pc-hrea` / HREA 电力可达性 | 仅香港 2019 v1 Item 的 `lightscore`；居民点通电可能性模型，不是夜光亮度、用电量或供电登记 | EPSG:4326，2,183 × 1,469，约 1 角秒，Float32 概率；有效零保留 | 原 Collection CC BY 4.0；保留研究团队、版本与来源 |
| `pc-biodiversity` / 生物多样性完整度 BII | 仅 2020 北京周边既定 Item 的 `data`；压力、丰度与组成相似性的模型评分 | EPSG:4326，7,992²，约 0.0008983152841195215°，Float32；无标量 NoData，有各层独立内部有效性 mask | 原 Collection CC BY 4.0；保留 Impact Observatory/Vizzuality 与白皮书 |
| `noaa-ohc` / NOAA 海洋热含量异常 | 仅 2020 Q4、0–700 m Item；相对 WOA09 的热含量异常，不是海温或热通量 | EPSG:4326，360 × 180，1°，Float32，10^18 J；NaN、负异常和有效零保留 | 原 Collection `proprietary`，保留 NOAA Use Agreement，不擅改为 CC |
| `noaa-nclimgrid` / NOAA NClimGrid 月度气候 | 仅美国本土 2022-09 Item；站点观测推算的 tavg/tmax/tmin/prcp 四变量，共一个产品 | EPSG:4326，1,385 × 596，0.04166667°，Float32；气温 °C、月降水 mm，NaN 保留 | 原 Collection `proprietary`，保留 NOAA 使用/分发说明及 DOI |

GHSL 2020 是模型参考年份，不生成虚构的卫星采集日期；不同年份、投影与 NRES 非住宅量未替换进来。JRC 产品表声明 S/V 缺测码 4294967295、POP −200，但这三件实际原 TIFF 均无 NoData 标签，全像元研究也没有这些代码。软件保留这一差异，不补造科学掩膜，不将有效零透明化。原始全球外边缘略超合法经度的文件保留原网格，仅查询边界与合法经纬度求交。

HREA 原 Item 的时间点为 2019-12-31T00:00:00Z，BII 则保留 2020-01-01 至 2020-12-31 的真实区间；OHC 固定第四季度、NClimGrid 固定九月。NOAA 两项是平台从原 NetCDF/TXT 发布的 COG 衍生件，不叫原 NetCDF 逐字节镜像。灰度显示范围仅控制视觉对比，不改变原 DN、概率、单位、尺度或 NaN。

## 原目录、头部核查与完整原件的区别

| 组别 | 实际发布证据 | 已完成独立研究检查 | 本轮软件验收对象 |
| --- | --- | --- | --- |
| GHSL | 三个 JRC 官方 AWS 完整原件，固定 2020 / R2023A V1.0 / 30 角秒 | 三件完整 SHA、全像元分块解码和 18 个原值；总计 785,079,046 B | 三件全球原件，各一个真实 UI 工程/任务 |
| ETH | 作者原 HTML 中完整 2,651 个 CHM 实际 href 和原几何；不是假造 STAC | 四件真实头部；仅 N51E003 完整 SHA、全部 1,296,000,000 像元和六个原值；171,132,359 B | N51E003 原件，荷兰陆地区域 [5.1,51.6,5.3,51.8] |
| HydroSHEDS / Geoid | 40 ACC、27 ACA、288 geoid 实际原件；355 次 32 KiB Range/206 头部核查成功 | 仅 n00w050 ACC/ACA 和 N45E105 geoid 三件完整 SHA、全像元和 18 个原值；175,978,717 B | 三件原件，ACC/ACA 各一件仍只计一个产品 |
| PC 四项 | 四个固定真实 Collection/Item 原样保存，精确层、时间、账户、容器与 canonical href 绑定 | 四个所选层及 NClimGrid 另外三个变量均完整研究下载、SHA 和全像元；另外三变量不计新产品 | HREA、BII、OHC、NClimGrid tavg 四件，共 297,439,513 B |

ETH 的 2,651 个候选、Hydro/Geoid 的 355 个成功头部检查、PC Collection 的更大整体范围都不表示每件已经完整下载、地图验收或有科学有效观测。所有文件沿用既有 512 MiB 单件上限、轴长/格网预算、分块解码与预览时限；索引内超限或未接格式不转成可用下载。不会换地区、改年份、制造小型服务导出或把 partial 当完整原件。

## 11 件完整原件与软件实测摘要

以下软件实际完整文件与独立研究原件的字节和 SHA 均一致。每件摘要是本地完整文件校核；发布方没有提供 SHA 的，不称发布方校验。所选原件之外的目录范围、仅核过头部的文件和另行研究的变量不包括在这十一件软件验收中。

| 原件 / 图层 | 软件完整字节 | 完整原件 SHA-256 |
| --- | ---: | --- |
| GHSL BUILT-S 2020 | 173,085,842 | `c622f77878056a29f41496a0d8412bf381f4501c546ef07e7d5a76696cefbc5b` |
| GHSL BUILT-V 2020 | 187,376,914 | `5f61071d872b5877cb506fcc3fac6793487568428ef8e00e0b54d6e392793b06` |
| GHSL POP 2020 | 424,616,290 | `b6eec24c613d0787c14166214865a3e0578b419ceebbaff00e46904e6a61e708` |
| ETH N51E003 CHM | 171,132,359 | `2d1247ba652d858dd1152c418d5ccd9bcdf5f8d34ec307b0275899ded550e7ac` |
| HydroSHEDS n00w050 ACC | 76,824,474 | `5a67c5fb2705b65117546dadc9437275a0d2ea9a2ecdd33b7eb59de3d35e14b4` |
| HydroSHEDS n00w050 ACA | 79,213,241 | `f4dd75675c54c74656bca10bdf010500421b3c9fc286095090f24a299b0edf93` |
| ETOPO N45E105 geoid | 19,941,002 | `5664f943ad524f4415cbdb068962d4b26e9f3ae0364f0662ea592e9561fa2ff8` |
| HREA lightscore | 343,602 | `c3a8d91858e609e06dfb701901bfb9582b7d472b173f1ad4e3c413ee84237718` |
| BII data | 295,765,273 | `a76ee54f166d68ce8ad6d61b6a58f262e04460e0dc9618a9a723404d55d5e843` |
| NOAA OHC heat_content | 157,352 | `f9892384908eeb92abcde4d8bcfa8805b23d584e380e1d34efd9da12a17b7c6d` |
| NOAA NClimGrid tavg | 1,173,286 | `cf3c7b6511ef8a3887a4c7d5c031d7a969a4b5afb8825395908e8219a346af19` |
| 合计 | **1,429,629,635** | 十个产品、十一件原件 |

小 AOI 不减少 GHSL 全球原件、ETH 整分幅或 HydroSHEDS 整分幅体量。传输耗时须来自本轮任务实际记录；研究下载、冷预览和固定带宽承诺不是同一项证据。

## PC 临时匿名只读 SAS 与源绑定

四项原始 Azure URL 需要官方临时匿名只读 SAS 才能读取。软件重读不可变源快照、原始 Item 和源证明，精确验证四个产品、资产 key、原始 Item URL、时间、未签名 canonical href 及原文属性。仅四个固定账户/容器范围可取得官方只读 token；任意自定义 STAC 或其他资产不会因为声明适配器名称而获得权限。

SAS 仅在进程内四容器缓存，合并同范围并发请求，过期前至少留 90 秒余量；token 端点请求限 15 秒、JSON 响应限 16 KiB。签名后的 origin/path 必须与未签名资产一致，DNS 校验和客户端仍从未签名地址建立；没有放宽全局公开 URL 规则。临时查询参数不写入 Item、Collection、工程、快照、设置、日志或持久文件；错误不返回原 token。软件保存的传输证明仍是 canonical href 与适配器名称。

这批没有 GEE 运行、账号凭据、云计算导出、自建镜像或商业影像采购步骤。PC 临时匿名签名不是用户项目的 GEE 额度；也不将原 Collection 的使用条件变成新商用托管或再分发权益。

## BII 内部有效性 mask 与原值回归

实际 BII TIFF 无标量 NoData 标签，主层及缩小层分别有原始内部 1 位有效性 mask。mask 排除的海区像元可仍存有限值，例如 0.6769765019416809；原生标量 NoData=false 与 mask 排除必须分开表达，不能给它补成 NaN 或假科学有效像元。有效零同样不得被自动透明化。

原生 PNG 仅在读取视图中正确解码原始 mask，源 TIFF、像元流和标签不变。限定同层同尺寸、1 位、MSB、唯一配对 mask，错层/重复/错尺寸/超预算拒绝。实际完整 BII 原件的原生 768² RGBA，与独立 Rasterio 对原 999² factor-8 层及其 mask 的参考采样 **逐通道差异为 0**：461,029 个有 alpha，128,795 个透明。PNG SHA 为 `dd1ad91d61bd81a8e0ca9cb3bd476dee23775d7608673a4d6ee561b3b06d2118`，原 TIFF SHA 不变。独立参考、实际软件落盘原件及重启后的 RGBA 复查均一致；原生六个代表点通过与全部主影像压缩块可读取仍是不同结论。

## 最终软件与真实界面验收

| 阶段 | 最终结果与证据边界 |
| --- | --- |
| 分类选择、检索与在线地图 | 十个独立产品、十一件所选原件均走实际 Explore 流程，保留各 AOI 和源身份；十一件在线地图均显示 |
| 软件完整下载 | 11 jobs 均 succeeded，共 1,429,629,635 B，完整 SHA 均与独立原件一致；九件首个任务完成，V 与 BII 保留实际中断/恢复记录 |
| 原生检查 | GET-only 六件组核对 36 个代表原值，五件组核对 30 个代表原值；原始网格、实际 NoData/内部 mask、完整文件、PNG、源快照和工程关系通过。不是原生读取全部压缩块的证明 |
| 标准重启与逐字段比较 | 同一最终内核重启后，11 个 jobs/projects ID、原快照、检查对象（含 PNG）及 66 个代表值/旗标与重启前相同，无新增传输；BII 额外 400 负例仍拒绝，未计为第七个通过点 |
| 实际 UI 工程重开 | 十一个工程重开的成果地图及参考原值均通过，`ui/ui-reopen-after.json` 为 11/11、allPassed；原始像元失败点显示友好中文，随后正常点恢复且 alertCount=0 |
| 代码与构建 | 元数据实际 58 个连接器、95 个数值层；源目录 133/91 Ready/6 Auth/36 Planned，17 组。相关合同/UI回归、仓库检查、原生预览 24 项常规及四个真实文件回归均通过；最终前端和两份桌面本地构建通过 |

以上结果可由[公开验收摘要](next-ten-public-channel-acceptance-20261014.json)查阅。真实界面[最新 BII 成果截图](images/next-ten-biodiversity-result-20261014.png)使用最终前端，显示正常参考像元并已从失败点恢复；截图不是全部主影像像元可读或科学质量无误的证明。

### 实际失败、修复与恢复

GHSL POP 原经度边缘略超 ±180°，旧成果地图因此拒绝定位。修复只绑定三件 GHSL 完整 SHA、EPSG:4326、dtype/NoData、网格/变换/边缘与不可变源证明；地图 fit/clip 使用合法 WGS84 交集，ImageStatic 保留原 PNG 范围，原网格没有被裁剪或拉伸。真实 UI 的列 35,568 / 行 5,903：POP 为 4307.731181065241、S 为 237713、V 为 5954942；POP 首列和末列 43,201 的有效零不变，全部 43,202 列保留，显示裁边说明已实际看到。错误来源、年份、量层及任意越界文件不获得该例外。

HREA 的坏 LZW 缩小层曾导致 PNG 为 null。修复仅绑定完整 SHA、2,183 × 1,469、EPSG:4326、area、Float32 和实际缺测声明，从同一文件完好的主影像生成 PNG，不放宽 LZW 校验或改写坏 overview。3,206,827 主像元的保守解码估计为 25,654,616 B（约 24.47 MiB），保持原资源预算。实际软件 768 × 517 PNG 与独立主影像参考逐 RGBA 一致，13,491 个有效 alpha；标准重启及真实成果重开后相同。这是该原件的限定修复，不推广到未知损坏文件。

实际 UI 的 GHSL BUILT-V 第二次尝试从 56,053,305 字节恢复并完成 187,376,914 B；BII 第三次尝试从 220,282,880 字节恢复并完成 295,765,273 B，最终 SHA 为 a76ee54f166d68ce8ad6d61b6a58f262e04460e0dc9618a9a723404d55d5e843。来源与强校验标识绑定、未换另一版本或注入科研文件；旧失败与 partial 记录保留，不抹去真实网络中断。

### 保留的材料限制

HydroSHEDS n00w050 整幅仅约 **0.207%** 像元为有效陆地，其余主要是海洋 NoData。768² 全分幅原生 PNG 各只有 1,256 个 alpha 可见像元、5 个灰阶，默认极值拉伸使它几乎黑色。完整文件、六个代表原值及 Explore 局部地图正确，不能称全分幅显示清晰或整体视效良好；没有改 mask、原值或伪造陆地。

BII 附加海域点 **列 7,689 / 行 4,010** 被原始有效性 mask 排除，独立解码的存储值为 0.7444648742675781、标量 NoData=false；当前严格原生 LZW 读取仍返回 **HTTP 400 / no lzw end code found**，标准重启后同样拒绝。该负例没有被算入六个通过点，不宣称全部 BII base chunks 原生可读。第一波段 PNG 按原 mask 透明，六个代表点、完整 SHA 和重启通过，并不消除这个局部读取限制。实际 UI 现提示“该位置的原始像元无法解码，请选择其他位置查看。”，随后正常点再次读出 0.49738889932632446，未留下错误 alert。

### 最终版本与回执

最终检查和标准重启使用 runtime **d53a94ef14b5d25b1328556ae3b3a984ddb274004cff2a4572859cf11576b992**（64,310,784 B），验收重启 PID 为 188752；旧内核及失败回执独立保留。最终前端为 **index-Cw8AuK_M.js**，SHA 75dd2939bc8b2d6757feb2ef2297c57052821c967bb91d73f80e7ae200b8bca9；中文包为 i18n-DZwPDypj.js。

当前隔离桌面 geod-next-ten-preview-final-ui.exe 为 146,521,600 B / SHA d65426e9a7fb5790e7d59c661f9671d8d2fdaa2ad29df87bf66be6345f74c2f4；标准 target/debug/geod-global-desktop.exe 同体量 / SHA 77d111b7331ed58492cf880fbc2e87a398a6e98e4927e25e3a61a8f490cc9ab9。仅进程内 TAURI_CONFIG 覆盖，源码配置和前端 SHA 前后不变；旧构建保留。**桌面编译不等于窗口启动、安装包签名、干净设备验收或生产部署**，本轮未据此声明这些步骤通过。

私有证据根目录为 `.verification/next-ten-products-20261014/`：六件最终回执在 `ui/native-six-final-after/`，五件在 `catalog-sources/ui-verification-after-restart/`；二者均由真实 UI 创建的同一 store 只读核查。`ui/ui-reopen-after.json`、`ui/final-pixel-error-ui-acceptance.json` 留下实际浏览器重开与错误恢复；`desktop-final-ui-build-acceptance.json` 留下最新双桌面校核。首次元数据、旧构建、错误图像与中断记录全部保留，科研 TIFF、临时 SAS 和私有数据未加入仓库。

## 尚未接入的研究与后续

WorldClim 此轮已从官方分发取得无需 ZIP 的尼泊尔 NPL 原件：12 波段 tavg 为 6,827,324 B、prec 为 4,524,406 B，完整 SHA、所有像元和真实存储尺度已独立核对。二者作为气候常年值产品的变量，不计两个新产品，也没有连接到软件。官方学术/非商业限制与商业/再分发许可需单独核对；19 波段 bioclim 超当前 16 波段能力。

全球 FABDEM v1-2 官方 ZIP 两个 TIFF 已完整独立研究下载与解码，但仍需安全受限解包，许可为 CC BY-NC-SA 4.0。另一个 Vietnam 2021 CC BY 4.0 区域件为 962,040,263 B，超现有 512 MiB，未完整下载且不扩大上限。HGB 全球轴长、Deltares NetCDF/kerchunk、大型 GEDTM/ANADEM 等仍超当前直接原件路径的格式或体量范围；没有生成服务 subset 冒充原件。

详细官方源、完整原件和科学说明见 [GHSL](research/next-ghsl-20261014.md)、[ETH 冠层高度](research/next-eth-canopy-20261014.md)、[HydroSHEDS 与 ETOPO geoid](research/next-hydro-geoid-20261014.md)、[PC 四项](research/next-planetary-20261014.md)。完整 [待接清单](pending-channels-20261014.md)保留 36 Planned 与 6 Auth；后续以独立产品和可取得的真实原件细化，不用目录数量取代功能闭环。
