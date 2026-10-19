# 多产品渠道扩展：真实下载、数值校验与社区研究

检查日期：2026-10-11。接续 [上一轮替代渠道实测](alternative-download-live-tests-20261011.md)。

本轮从光学影像扩展到雷达、静止气象卫星热红外、土地覆盖、人口、土壤、降水、海洋海温、数值天气预报和太阳辐射。**11 个数据产品集合、19 个完整文件或完整数值响应**已下载并解码，共 46,495,465 字节科学数据；全部再次由主任务读盘校验长度与 SHA256。其中包含同一产品的多镜像、同一集合的不同日期或区域复测，不能理解成 19 家平台。账号、授权、样本范围、版本和格式兼容性分别记录。

本轮没有读取用户密钥、新建账号或长期凭据、改变代理，也没有调用 Earth Engine 计算任务。新解码依赖只安装在忽略的验证目录中。这是公开渠道小样本实测，**尚未将新渠道接入桌面软件，也未验收整个 AOI 的检索、确认、下载和成果展示闭环**。

## 下载与解码结果

时间为当前网络下单次数据响应下载时间，不包含全部搜索/元数据/解码时间，也不是长期速度承诺。人口与土地覆盖采用小文件海外区域，高程沿用上一轮样本，本轮不重复统计。

| 产品集合 | 实际渠道与样本 | 下载表现 | 完整性及适用范围 |
|---|---|---|---|
| Sentinel-2 L2A | Earth Search COG 与 Google 公共 SAFE JP2，北京同景 B01；另取 SCL | B01 COG 6.24 MB / 1.94 秒；JP2 3.77 MB / 2.68 秒；SCL 2.48 MB / 0.73 秒 | 三个文件全读；B01 60 米、SCL 20 米。不是 10 米真彩色下载验收；两个渠道原 DN 有处理差异 |
| Sentinel-1 OPERA RTC | 公开应用样本桶，2024-01-10 法国近海 VV 和 mask | VV 6.17 MB / 1.65 秒；mask 43.9 KB / 0.35 秒 | 完整 burst，30 米；有效范围约 40.24%。是归档样本，正式全球生产下载仍需要 Earthdata 授权 |
| GOES-19 ABI L1b | NOAA AWS，2026-10-11 C13 热红外 mesoscale 区域 | 359 KB / 1.74 秒 | 完整 NetCDF，Rad/DQF 全读，发布者 MD5 匹配；标称星下点 2 km，原生静止卫星投影；区域在美洲 |
| ESA WorldCover | AWS，2021 v200、Svalbard N78E015 三度瓦片 | 20.25 MB / 4.94 秒 | 36000×36000、10 米，逐块全解码 12.96 亿像元；数值类别，非彩色底图；有效约 75.02% |
| WorldPop | 官方直链，Niue 2020 Global1 整国家 | 156.8 KB / 2.63 秒 | 人数/像元的浮点人口估计；214×245；有效约 63.79%。不是最新版 Global2 验收 |
| SoilGrids 2.0 pH | ISRIC WCS，北京城区及北部农村 | 城区 1.3 KB / 2.70 秒；农村 4.3 KB / 1.27 秒 | 两个 80×80 数值子集；原模型约 250 米；整数除 10 得 pH。城区仅 2.19% 有效，农村 99.58% |
| CHIRPS 2.0 月降水 | NOAA ERDDAP，北京 2024-01、0.05° | 8.1 KB / 1.94 秒 | 21×21 数值 NetCDF，441 个有效值；卫星与雨量站融合月累计，不是实时气象影像 |
| NOAA OISST v2.1 | NCEI 2025-09-15 与 2025-01-01 整日全球文件 | 1.53 MB / 2.82 秒；1.55 MB / 2.93 秒 | sst/anom/err/ice 四个科学变量全读；0.25°、L4 融合插值海温，不是 AVHRR 原场景 |
| NOAA GFS | AWS、Google、Azure；2026-10-11 00Z、f000、二米温度 | 每份 875 KB，分别 1.87 / 2.24 / 19.59 秒 | 三份完整 GRIB2 消息，1440×721，**SHA256 一致**；没有下载约 493.5 MB 全变量文件 |
| ECMWF IFS Open Data | AWS 与 Google；同日期时次二米温度 | 每份 654 KB，2.47 / 1.34 秒 | 两份完整消息 SHA 一致；ecCodes 全读 1,038,240 个值。当前 GDAL 3.6.4 不能读此 CCSDS 编码 |
| NASA POWER | 匿名 Daily API，北京 2025-09-01—07 气温及太阳辐射 | 838 B / 1.03 秒 | 完整 JSON，两个变量共 14 条有效记录、单位验证。是卫星/再分析派生时间序列，不是场景栅格 |

全文件 URL、请求时间、产品标识、坐标、NoData、标度/偏移、像元统计和 SHA256 见 [机器可读实测凭据](multisource-live-tests-20261011.json)。原样文件与预览在 [验证目录](G:/code/geod-global/.verification/multisource-expansion-20261011/)。

## 同源镜像的科学一致性

同场景、同波段和同网格，并不必然保留相同的存储像元。所测 Sentinel-2 SAFE 的处理基线为 05.11，原 XML 写明 `BOA_ADD_OFFSET=-1000`、量化值 10000。[Google 公共档案](https://cloud.google.com/storage/docs/public-datasets/sentinel-2)提供 SAFE/JP2 路线；[Earth Search](https://github.com/Element84/earth-search)另提供处理过的 COG。

对 3,348,900 个同网格非 NoData 像元：原 DN 相等率为 0%；GCS DN 减 1000 后，相等率 97.49022%；使用 `COG DN = max(GCS DN - 1000, 1)` 后，匹配率 100%。84,050 个像元、约 2.50978%，被截到 COG DN=1；原低端值包含负反射率与零反射率，截断后无法恢复其中的原负反射率。所测 STAC 同时声明 BOA 偏移已经应用、B01 又带 offset=-0.1，存在再次扣除的风险。这个冲突已原样保存，不能依据一个字段静默推断。

因此换源策略应保留 provider、产品等级、基线、原始网格、scale/offset、QA 和处理记录；按来源规则只校准一次，并对截断和元数据冲突提示。需要保留负反射率时应选原 JP2。该结论来自 [本轮逐像元比较](G:/code/geod-global/.verification/multisource-expansion-20261011/optical-radar/s2-cross-host-radiometry.json)，不推广为所有年份、基线或波段的固定转换公式。

GFS 和 IFS 所测多个镜像的完整消息则保持字节一致。可以在通过产品、时次、变量、网格和消息哈希验证后回退，但仍应从各镜像自己的索引定位记录；同名文件不能直接借用另一版本的字节偏移。

## 下载问题及软件兼容性

- **解码失败与下载失败要分开。** IFS 两条路线都下载完整，本次 Python Rasterio 自带 GDAL 3.6.4 报不支持 GRIB2 CCSDS 模板 5.42；独立 ecCodes 2.49.0 解码成功。数据值域为 215.81—308.81 K，没有缺测。尚未验证桌面原生处理器，不能推断桌面当前一定失败，也不能声称桌面端已支持；接入时要确认其解码能力，必要时升级或添加处理器。[ECMWF 当前说明](https://www.ecmwf.int/en/forecasts/datasets/open-data)确认公开产品使用 CCSDS 压缩和多云副本。
- **同一个服务器也可能只有部分集合成功。** NOAA CoastWatch 的 CHIRPS 数值响应成功，OISST 的 `.dds` 元数据成功，但数值 `.nc` 请求返回 503。错误正文已保留；扩展名为 `.nc` 的文字错误没有计入成功。NCEI 的 OISST 两个整日文件成功，故它可作为这类失败时的另一发行路线，而非说明 ERDDAP 整个平台不可用。
- **覆盖不足不会因下载成功消失。** SoilGrids 城区样本约 97.81% 空值，人口与雷达的海域/边界外也有 NoData。必须以真实数据有效覆盖验收，不能仅以 TIFF 成功打开验收用户目标。
- **小子集不等于全区域大任务。** 本轮没有测试多年全球时序、完整北京多景拼接或商业托管高并发；历史小文件也不证明最新数据的更新延迟。
- **许可元数据也需保留来源冲突。** 所测 GOES 文件内部写“Access is restricted to approved users only”，而 [NOAA 管理的 NODD 当前发布说明](https://registry.opendata.aws/noaa-goes/)明确公开数据可使用，匿名文件访问也成功。本轮依据正式公开发行规则测试，不把内部旧文本当鉴权失败；两份原文均保留，产品发布时应明确使用依据。

## 社区经验与新增候选

完整记录见 [按产品组织的渠道研究目录](multisource-channel-research-20261011.json)。本轮整理 **52 条产品/渠道研究记录**，包含新发现和现有渠道的补充、同平台不同产品和重叠经验，记录数不能当独立镜像数或接入数量。中文文章、论坛、GitHub 和教程作为发现线索，下载状态单独记录；打不开的公众号原文或仅搜索摘要不会当完整读过。

| 产品方向 | 值得继续验证的渠道 | 目前依据与门槛 |
|---|---|---|
| 光学/SAR/大气 | CBERS 当前 `brazil-eosats`、MEEO Sentinel-3/5P、AWS S1 GRD、ASF、CDSE、GEODES | 公共目录、原件、RTC、平台 Process API 分开；部分需要个人授权或 requester-pays。Indigo RTC 是暂停更新的 CONUS 历史路线 |
| 土地/城市/人口 | WorldCover 年合成、Esri IO10m、Dynamic World、GHSL、WorldPop Global2、JRC Global Surface Water、CGLS LC100 | 不把分类产品说成原始高分辨率影像；新版与历史样本分开。JRC 当前页面已指向 v1.5/至 2024 的发行 |
| 中国高分辨率专题数据 | CLCD、GLC_FCS30D、CHM_PRE，作者 Zenodo 与科学数据中心 | 作者公开元数据已验证 open access、CC BY 4.0；没有下载 GB 级包，尚未解码验收。土地分类/融合降水产品不能当国产卫星原场景 |
| 高程/土壤 | SoilGrids WCS、OpenLandMap、GEDTM30、FABDEM、多 DEM 的 OpenTopography | 逐产品核对 DSM/DTM、版本、许可；FABDEM 为非商业许可，不能直接商用；本轮未重复已有高程下载 |
| 气象/降水 | GFS、ECMWF、多种 ERA5 ARCO/NCAR/EDH、CHIRPS、PERSIANN、风云/MOSDAC | GRIB 消息与 Zarr 分块适用于不同请求；个人 API、云计算部署与公开对象存储不是同一种服务 |
| 海洋/冰雪 | Copernicus Marine、OB.DAAC、PO.DAAC、NSIDC、海洋卫星平台 | 原始轨道/L2/L3/L4融合产品分开；NASA 部分正式集合需 Earthdata，旧站/API 和 S3短期授权需维护 |

[Herbie](https://github.com/blaylockbk/Herbie)维护者已经实现按变量下载和多镜像查找，值得借鉴获取逻辑。[ERA5 用户讨论](https://github.com/PyPSA/atlite/issues/495)记录 CDS 长排队以及 NCAR/EDH/Zarr 替代，说明排队和分块也会导致慢；其中他人的耗时不能当本机实测。Google 的 [ARCO ERA5 官方实现](https://github.com/google-research/arco-era5)明确区分原 Gaussian 网格的 cloud-optimized 数据与规则网格的 analysis-ready 数据，并提供更新时间字段。

Copernicus Marine 的 [维护者问题讨论](https://github.com/mercator-ocean/copernicus-marine-toolbox/issues/267)解释了 geo-series 和 time-series 的块组织差异；按最终请求范围开数据，避免先开多年数据再逐日抽取。[当前官方说明](https://help.marine.copernicus.eu/en/articles/7949409-copernicus-marine-data-access-options)要求免费个人账号，并说明无下载量/带宽配额。本轮没有登录或提交 Marine 任务，仍是有依据的候选。NASA [海色官方教程](https://nasa.github.io/oceandata-notebooks/notebooks/modis/modis_explore_l3.html)明确要求 Earthdata 登录，并强调同短名集合存在多个时间聚合和分辨率版本。

国内专题数据还核到了作者的 [CLCD v1.0.3](https://zenodo.org/records/12779975)、[GLC_FCS30D](https://zenodo.org/records/8239305)与 [CHM_PRE v2.1.1](https://zenodo.org/records/20378336)公开发行元数据，许可均为 CC BY 4.0；本轮没有下载这些 GB 级科学数据包。CLCD 所核版本覆盖 1985—2023；GLC_FCS30D 为全球 30 米分类变化产品，1985—2022 且前后年份时间步长不同；CHM_PRE 为中国大陆 0.1°融合降水，1960—2025。它们是有明确版本的分析数据，不能宣传成最新卫星原片渠道。CBERS 的 [当前 AWS 注册表](https://registry.opendata.aws/cbers/)改为 `brazil-eosats`/us-west-2，社区旧地址要迁移核验；本轮没有下载 CBERS 栅格。

## 接入优先级

1. **公共直连，优先形成闭环：** GCS Sentinel-2 原 JP2 与现有 COG 的来源/校准处理；WorldCover、WorldPop；CHIRPS 与 OISST；支持数值的 WCS。GOES 可扩展美洲天气影像，但应提供原投影和辐射/温度说明。
2. **镜像路由与格式支持：** GFS/IFS 按变量提取、来源索引核验、多镜像回退；IFS 解码兼容性完成前，不能列成已支持。
3. **自带账号的数据连接：** ASF 正式生产、Marine、NASA海色/冰雪、部分国产与印度数据平台，先取得用户自己的合法授权，再跑完整检索至成果流程。
4. **官方托管能力：** 只有涉及我们自己的代理计算、长期监测、托管账号与配额时才评估 Pro 成本和授权；公开影像本身不因列入目录就变成收费采购。非商业数据许可、共享 Key 禁止条款和正式生产账号边界不能省略。

具体来说，[OpenTopography 当前开发者规则](https://opentopography.org/developers/)区分个人 Key 与商业软件集成；公共静态文件下载成功不等于可以把免费个人 Key 共享给所有用户。正式服务的收费/容量设计要基于它的 Enterprise 授权。各连接器须单独完成性能、错误、重试/断点、数值、许可和 UI 闭环验收后，才能更改“已接入”状态。
