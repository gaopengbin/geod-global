# 遥感替代渠道与社区下载经验调研 · 2026-10-11

> 最新扩展已覆盖非 Landsat 的 11 个产品集合，19 个完整文件/数值响应和 52 条研究记录，详见 [多产品实测与调研报告](multisource-live-tests-20261011.md) 与 [扩展研究目录](multisource-channel-research-20261011.json)。下文“本轮”仍指早期调研快照，不能作为最新下载状态。
>
> 前一轮 JAXA AW3D30、OpenTopography COP90、Esri Landsat L1子集的结果，及真正C2L2/国内三家的限制，保留在 [真实下载报告](alternative-download-live-tests-20261011.md)。

这轮从论坛、GitHub、中文博客和公众号原文/可读转载寻找线索，再用当前平台资料校准，整理 **28 条社区与文章证据、30 条渠道/服务候选**。数量包含云处理和专题产品，不表示 30 个独立镜像，更不表示已经接入。

目标是给 GeoD Global 建立可切换的实际下载路线，尤其缓解 Landsat、MODIS、SAR 等当前传输问题。商业影像采购不在范围内。不同平台有相同卫星数据，也可能提供不同 Collection、处理级别或派生成果，必须逐项核对。

候选清单：[结构化渠道目录](alternative-channel-candidates-20261011.json) · [社区证据登记](alternative-channel-community-sources-20261011.json) · [根代理搜索结果索引](alternative-channel-search-ledger-20261011.json) · [本机访问记录](alternative-channel-access-probes-20261011.json)。搜索索引是发现过程，不是全部网页已全文核验的声明。

## 对当前产品最有用的结论

| 用户要的数据 | 优先验证的替代路线 | 需要守住的区别 |
|---|---|---|
| 北京等国内区域 Landsat | **GSCloud 同景原件；AI Earth / PIE 自带账号区域导出**；现有 GEE 作为计算导出路线 | GSCloud 需查 C2L2、年份、波段；平台裁剪成果不一定保留原网格。暂不能保证比当前源快 |
| Sentinel-2 | 已有 Earth Search 保留；补 **GEODES、GCS 公共 S2** | JP2/COG、L1C/L2A、大气校正基线及同景 ID 分开；不能自动降级为另一种产品 |
| Sentinel-1 | **ASF 原始 GRD/SLC、AWS GRD COG** | 这两条不能静默替换 PC RTC；预制 OPERA RTC 与 HyP3 按需加工另列 |
| MODIS/VIIRS/HLS | **NASA LPCLOUD、LAADS** 原件；AppEEARS/ORNL 小区域路径 | 产品编号、Collection、QA 层、分辨率必须一致；HLS 30m 不是 S2 10m |
| DEM | **OpenTopography 静态 STAC、JAXA Earth AW3D30**；现有 CopDEM 保留 | DSM/DTM、垂直基准、版本不同。OpenTopography 的目录与商业裁剪 API 不是同一种权益 |
| 夜光、土壤、覆盖、降水等 | **OpenLandMap、JAXA Earth、NOAA JPSS、Satlas** | 丰富专题产品生态；不能宣传为最新原始光学影像的同源镜像 |

目前没有证据支持“所有慢下载都换成一个新的免费高速镜像”。更可行的是给同一产品准备多条经验证的原件路线，再给只需要小区域的任务提供裁剪导出。

## 本轮实际访问与数据验证

没有新建账户、读取用户凭据、支付费用或使用 GEE 项目计算额度。下面是本机匿名请求；时长含请求开销，不能作为影像吞吐率测速。

| 路线 | 实际结果 | 可成立的结论 |
|---|---|---|
| GEODES collections | HTTP 200；29,413 字节；2.797 秒；返回分页及 beta 扩展 STAC | 目录可访问。只取到第一页，不能说全库只有 10 个集合；未验证鉴权下载/通用 STAC 兼容 |
| OpenTopography Raster STAC | HTTP 200；43,555 字节；1.255 秒；STAC 1.1.0 | 新目录可读，原件/裁剪 API 未验证 |
| JAXA Earth STAC | HTTP 200；24,479 字节；0.824 秒；112 个 child 链接 | 找到 AW3D30 v3.2 的日本 Wasabi 集合；还没有完成 COG 文件下载 |
| OpenLandMap STAC | HTTP 200；20,915 字节；1.871 秒；104 个 child 链接 | 专题产品目录可读，COG 完整下载未验证 |
| ORNL products/dates | HTTP 200；46 个产品；北京位置列出 612 个日期 | 能检索，但日期列表不保证像元已可获取 |
| ORNL 北京历史 NDVI | 2018-02-18；9×9，共81个像元；JSON 1,142 字节；15.211 秒 | 实际小区域数值子集已获取；不是整幅 HDF/GeoTIFF 下载，也不是最新数据验收 |
| ORNL 官方示例位置 | 同历史日期，81个像元；JSON 1,167 字节；8.595 秒 | 第二个位置的小区域数值查询成功 |
| ORNL 北京目录最新日期 | 2026-09-14，带/不带 NDVI 波段参数均 HTTP 200 **但 subset 为空** | **未通过新鲜度/有效数据验证**；不能以 HTTP 成功或目录有日期标记可下载 |

实际历史子集保存于 [北京 NDVI 样本](ornl-beijing-ndvi-2018-20261011.json)。原始整景下载能力仍需要单独验收。先前当前软件渠道的真实文件测试见 [已接入渠道实测](../channel-live-audit-20261011.md)，不要把那次验收移植到本轮新候选。

## 30 条候选渠道/服务

“优先”表示下一步应测试，不是已接入或已验证高速。用户账号导出表示使用用户自己的平台账号；托管计算/下载则需要单独确认平台商用与服务许可。

### 国内与光学替代

| 候选 | 产品与交付方式 | 账号、费用及限制 | 本轮状态 / 优先级 |
|---|---|---|---|
| D01 [地理空间数据云 GSCloud](https://www.gscloud.cn/sources) | Landsat、MODIS、DEM、部分 Sentinel/国产影像；原件/平台产品，逐集合核对 | 用户账号；开放稳定 API 与当前 C2L2/Landsat9 完整度未确认 | 目录/文档；社区有成功原件经历，本轮未下载；**优先同景下载测试** |
| D02 [阿里 AI Earth](https://help.aliyun.com/zh/document_detail/128320.html) | Landsat、Sentinel、MODIS 分析/区域导出；平台处理/裁剪结果；是否可取得完全同源原件需核对 | 阿里账号/SDK身份；有下载等增值用量，现行免费额度待登录核验 | 官方计费说明可读；详细配额页抓取不完整；本轮未下载；**优先用户账号导出测试** |
| D03 [PIE-Engine Studio](https://engine.piesat.cn/engine-studio/) | Landsat、Sentinel、MODIS 等分析就绪数据；imageToCloud 等区域导出；可能重投影/采样 | 用户账号；当前外部 SDK、下载额度与商用权限待确认，不沿用旧论文配额 | 社区导出代码/团队论文；本轮未完成平台下载；**优先用户脚本导出测试** |
| D04 [CNES GEODES / THEIA](https://geodes.cnes.fr/support/api/) | Sentinel-1/2、VENUS、SPOT历史、THEIA 产品；独立分发原产品与加工产品；THEIA Landsat 属历史集合 | 目录匿名；下载用户账号/API Key；逐集合许可 | 本机 STAC collections HTTP 200；原件未下载；API返回 beta 扩展版本需兼容检查；**优先 S1/S2 下载测试** |

### 雷达与多云分发

| 候选 | 产品与交付方式 | 账号、费用及限制 | 本轮状态 / 优先级 |
|---|---|---|---|
| D05 [ASF Vertex / ASF 档案](https://docs.asf.alaska.edu/asf_search/downloading/) | Sentinel-1 GRD/SLC、ALOS、OPERA 等；真实独立分发；预制 RTC 与原始 GRD/SLC 分开 | Earthdata 认证及产品 EULA；asf_search 仅为客户端 | 当前下载文档/社区经历；本轮未下载；**优先原件替代测试** |
| D06 [AWS Sentinel-1 GRD](https://registry.opendata.aws/sentinel-1/) | Sentinel-1 GRD COG；独立云拷贝；非 SLC、非 RTC | 注册页明确无 AWS 账号可读；eu-central-1 | 当前桶/格式文档；本轮未下载；**优先匿名 COG 测试** |
| D07 [Google Cloud 公共 Sentinel-2](https://docs.cloud.google.com/storage/docs/public-datasets) | S2 公共数据；Google Landsat 公共档案仍为 C1；GCS 对象存储；和 GEE 计算服务分开 | 公共对象 HTTPS API 无需认证；核对处理级别/基线/是否有同景 | 当前 Google 文档；本轮未下载；**补 S2 存储备份** |
| D08 [WEkEO HDA](https://help.wekeo.eu/en/articles/6771578-how-to-use-the-harmonized-data-access-rest-api-with-curl) | Sentinel、EUMETSAT、Copernicus Land/Marine/ECMWF；联邦访问；并非所有底层存储独立于 CDSE | 免费用户账号、接受产品许可证；有请求/订单限制 | HDA API 文档；本轮未下载；**第二批认证测试** |

### NASA与按需子集

| 候选 | 产品与交付方式 | 账号、费用及限制 | 本轮状态 / 优先级 |
|---|---|---|---|
| D09 [NASA LPCLOUD / Earthdata HTTPS](https://github.com/nasa/MODIS-VIIRS-Data-Resources/blob/main/guides/after_datapool.md) | 对应云集合 MODIS、VIIRS、HLS 等；NASA 云档案；earthaccess/CMR/STAC 不是多个独立镜像 | 免费 Earthdata 用户身份；云内 S3 临时授权条件与跨网 HTTPS 分开 | 云迁移/认证文档；本轮未下载原件；**优先 MODIS/VIIRS/HLS 原件测试** |
| D10 [NASA LAADS DAAC](https://ladsweb.modaps.eosdis.nasa.gov/tools-and-services/data-download-scripts/) | MODIS/VIIRS 辐射、陆地/大气等所列产品；不同 NASA 分发档案；产品覆盖不能与 LP DAAC 全部互换 | Earthdata 认证/Bearer token；不同 Collection/波段需要核对 | 下载脚本/社区文档；本轮未下载；**优先相关 MODIS/VIIRS 原件测试** |
| D11 [AppEEARS](https://appeears.earthdatacloud.nasa.gov/api/) | 所列 HLS、MODIS、VIIRS、SRTM 等；任务化裁剪/重投影/格式转换；非整景镜像 | Earthdata 账号；排队；产品清单逐项确认，HLS 30m 不替代 S2 10m | API/真实用户队列经历；本轮未提交任务；**优先小 AOI 导出测试** |
| D12 [ORNL TESViS](https://modis.ornl.gov/data/modis_webservice.html) | MODIS/VIIRS 等支持产品的小区域数值/时序；CSV/JSON 子集，不是完整 HDF/GeoTIFF 原件 | 本轮数值 REST 请求匿名；普通 subset 最多10日期 | 目录46产品；北京2018 NDVI实际81像元；目录最新2026-09-14返空，未通过新鲜度；**带数据新鲜度问题进入测试** |
| D13 [CDSE Sentinel Hub Process / openEO](https://documentation.dataspace.copernicus.eu/APIs/SentinelHub/Process.html) | S1/S2 等指定波段、指数、区域导出；加工路径；同步/批处理有不同延迟，不增加独立原件镜像 | CDSE/OAuth；独立请求、处理单元/计算额度 | 官方 API/用户同任务延迟比较；本轮未提交；**第二批小 AOI 同步测试** |
| D14 [Google Earth Engine（现有路线）](https://developers.google.com/earth-engine/guides/exporting_images) | 平台收藏中的 Landsat、Sentinel、MODIS等；计算/区域导出；需保留产品/处理/采样记录 | 用户项目；计算额度及商用规则；大任务异步导出 | 先前下载证据另存；今日原临时链接已过期；本轮未新增下载；**完善稳定授权与大任务导出** |

### 高程与专题开放生态

| 候选 | 产品与交付方式 | 账号、费用及限制 | 本轮状态 / 优先级 |
|---|---|---|---|
| D15 [OpenTopography](https://opentopography.org/developers/) | CopDEM、SRTM、NASADEM、AW3D30、区域 lidar DEM等；独立 DEM 分发/COG 与裁剪 API 分开 | 目录可匿名；Global API个人Key有额度，商业软件集成需 Enterprise Key，不能共享免费Key | 本机新 Raster STAC HTTP200，STAC1.1.0；原件未下载；**优先静态 STAC；商业 API 后置** |
| D16 [JAXA Earth API](https://data.earth.jaxa.jp/en/) | AW3D30、森林、降水、温度、NDVI等公开转换产品；静态 STAC/COG；同名产品的分辨率/合成周期不一定相同 | 当前官方说明免注册/Key且无调用次数限制；逐集合许可/署名 | 本机STAC HTTP200，112 child链接；发现日本Wasabi AW3D30 v3.2；原件未下载；**优先匿名 STAC/COG样本测试** |
| D17 [JAXA G-Portal](https://gportal.jaxa.jp/gpr/information/download) | GCOM-C/W、GPM等任务产品；原任务产品通道；不等同 Earth API 转换版 | 下载需用户注册；HTTPS/SFTP等 | 下载说明；本轮未下载；**第二批具体产品认证测试** |
| D18 [OpenLandMap / OpenGeoHub](https://docs.openlandmap.org/020-stac.html) | 土壤、覆盖、夜光、FAPAR、地形等；很多为预测/融合/合成产品，非最新原始光学景 | 公开 STAC/COG；许可逐集合核对 | 本机STAC HTTP200，104 child链接；原件未下载；**优先匿名专题 COG 样本测试** |
| D19 [USGS The National Map / 州级原始数据](https://www.usgs.gov/the-national-map-data-delivery/gis-data-download) | 3DEP、lidar、NAIP等美国数据；原数据下载索引与可视化 ImageServer 分开 | 门户有公开下载路线；具体文件权限/格式核实；只覆盖相应美国区域 | 当前下载说明；本轮未下载；**第二批美国 DEM/NAIP测试** |

### 费用/保留期受限云存储

| 候选 | 产品与交付方式 | 账号、费用及限制 | 本轮状态 / 优先级 |
|---|---|---|---|
| D20 [AWS USGS Landsat C2](https://www.usgs.gov/landsat-missions/landsat-commercial-cloud-data-access) | Landsat C2 L1/L2/L3/美国ARD所列产品；真实云对象/STAC，不是匿名免费镜像 | Requester Pays，AWS签名账号；请求/流量付费归属 | 当前USGS说明；本轮未产生费用/下载；**有费用归属后测试** |
| D21 [AWS NAIP](https://registry.opendata.aws/naip/) | source/analytic四波段，visualization三波段；原始/无损分析/有损可视化三桶不能互换 | 三个桶均Requester Pays；当前多数产品2010–2023 | 当前注册页；本轮未下载；**按产品及费用再测试** |
| D22 [NOAA JPSS AWS/GCS/Azure/CLASS](https://registry.opendata.aws/noaa-jpss/) | VIIRS SDR/EDR、DNB、火点等NOAA产品；多存储通道；不是NASA VNP13A1同源镜像 | AWS公共桶可匿名；2026-09-14起部分仅保留90天，其他长期保存；具体GCS/Azure权限另核实 | 当前保留策略/分发文档；本轮未下载；**按VIIRS产品单独测试** |

### 国产/机构/研究候选

| 候选 | 产品与交付方式 | 账号、费用及限制 | 本轮状态 / 优先级 |
|---|---|---|---|
| D23 [中国资源卫星 CRESDA](https://data.cresda.cn/#/home) | 开放权限所覆盖GF、ZY、HJ等；原产品/申请订单，权限按产品和用户 | 实名/审核/授权；注册不代表全部可下载；不涉及商业影像采购 | 社区实际GF1经历；当前权限下载未核验；**第二批自有账号权限测试** |
| D24 [风云 NSMC](https://data.nsmc.org.cn/DataPortal/cn/support/faq.html) | 风云及服务清单所列气象产品；原产品/订购、官方客户端下载 | 注册实名及用户级别/下载限额 | 当前FAQ/客户端文档；本轮未下载；**第二批气象产品路线** |
| D25 [海洋卫星 NSOAS](https://osdds.nsoas.org.cn/) | 海洋水色/动力环境所列产品；专题及原产品，不替代陆地10m光学影像 | 用户账号/订单/逐产品许可；稳定外部API待确认 | 公众号转载线索/门户；本轮未下载；**第二批海洋专题路线** |
| D26 [NODA 遥感卫星地面站数据](https://www.noda.ac.cn/rsgs/datasharing/aboutEverybody) | Landsat及地面站开放档案；机构分发；历史CEODE入口须迁移核对 | 账号/申请权限；当前可读性/API未确认 | 搜索目录线索；本轮网页抓取遇到拦截页，未下载；**候选，先解访问与权限** |
| D27 [CASEarth DataBank](https://www.cbas.ac.cn/kypt/databank/) | 长时序Landsat、全球/全国RTU镶嵌等；处理后RTU产品，非所有原始景 | 公开介绍未证明外部免费API、商用许可或下载权益 | CBAS介绍可读；本轮未验证下载入口；**候选，先许可/API核验** |
| D28 [OGE 开放地球引擎](https://openge.org.cn/) | Landsat、MODIS、Sentinel、珞珈等页面所列数据；云处理/工作流平台 | 登录后额度、稳定公开API、下载/商用许可未确认 | 本轮早期抓取曾见维护；最终web读取首页正常，不能判全站停用；未下载；**候选，先账号/API导出核验** |
| D29 [AllenAI Satlas](https://github.com/allenai/satlas/blob/main/GeospatialDataProducts.md) | 树冠、风机、太阳能设施等派生成果；AI专题产品；超分不能标成真实新增空间细节 | 所列开源/开放产品许可，逐成果核对 | 维护者成果下载说明；本轮未下载；**专题成果扩展** |
| D30 [Esri Living Atlas ImageServer](https://developers.arcgis.com/rest/services-reference/enterprise/export-image/) | Landsat/Sentinel等服务对应产品；动态exportImage；不默认包含原始场景全部波段/QA/元数据 | 部分服务可匿名，部分需订阅；核实该层使用条款/尺寸/分析权限 | 当前服务/REST文档；本轮未下载具体影像；**区域分析/可视化候选** |

## 社区与公众号证据摘录

下列是推荐路线或真实用户经历，保留发表时间。历史个案可以提示问题，不能承诺我们今天的下载速度。其余出处和文章登记在上方社区 JSON 中。

| 出处 | 发现 | 对产品的用途 |
|---|---|---|
| [CSDN 多平台下载实验](https://blog.csdn.net/m0_56808314/article/details/134957523) · 2023-12-12 | GSCloud Landsat 成功；GF1 压缩包损坏，重申链接后成功 | 国内原件路线值得同景试验；需要文件完整性和失效链接处理 |
| [AI Earth 用户问答](https://developer.aliyun.com/group/rsimganalys/?pageNum=11) · 2024-01 | 合并波段下载、坐标、ST_B10缺失及年份覆盖问题 | 不能只测平台登录/预览，要测字段、产品和输出 |
| [PIE 数据下载器](https://developer.aliyun.com/article/1273990) · 2023-07-14 | 按区域/时间/云量过滤并导出云盘 | 可以先交付复现脚本与成果导入，再核实直接 API |
| [PIE 导出经验](https://www.cnblogs.com/GISer-Liu/p/16340688.html) · 2022-06 | 先筛选拼接裁剪；云盘空间和流量有限 | 借鉴按需导出，但不沿用旧免费额度 |
| [博客园 Sentinel-1 实操](https://www.cnblogs.com/Vicrooor/p/17396546.html) · 2023-05-25 | ASF与ESA互相兜底，也有网络失败和离线等待 | 增加真实不同分发渠道；错误类型与等待时间要显示清楚 |
| [GIS知乎 6GB下载失败恢复](https://zhihu.geoscene.cn/m/article/1831) · 2016-05-29 | 下载接近完成时中断 | 断点续传、文件验证比无条件整文件重试重要 |
| [气象家园多源清单](https://bbs.06climate.com/forum.php?mod=viewthread&tid=53920) · 2017-06-19 | 列出国内镜像、风云、DEM与机构平台 | 发现候选；旧域名和停用服务要刷新 |
| [公众号 RGB 3S 作者博客](https://www.ixxin.cn/2016/11/27/landsat8freedata/) · 2016-11-27 | 推荐GSCloud/USGS；评论有注册失败与中断 | 保留用户痛点，但旧网盘和下载技巧不直接变成产品能力 |
| [疯狂学习GIS 可读转载](https://developer.cloud.tencent.com/article/2523270) · 2025-05-21 | 除USGS还有GSCloud、ASF、LAADS、海洋卫星等 | 拓展产品来源；文中旧 Open Access Hub 名称需修正 |
| [国产卫星权限文章转载](https://cloud.tencent.com/developer/article/2514795) · 文内2025-02更新 | CRESDA高分/资源/环境卫星权限路线 | 用合法自有账号；不把注册即全量下载写入宣传 |
| [Reddit Landsat替代入口](https://www.reddit.com/r/remotesensing/comments/1cqw2yj) · 2024-05-13 | 原站故障时推荐AWS、GEE、Esri、STAC | 需要多入口；推荐不是速度对照试验 |
| [Reddit 小区域下载问题](https://www.reddit.com/r/remotesensing/comments/1dzxs0p) · 2024-07-10 | 为小研究区下载整景造成过量传输 | 原件和AOI导出分别提供，不默认强制整景 |
| [STEP 并发/慢下载](https://forum.step.esa.int/t/parallel-download-limit-for-large-volumes/10606) · 2018-06-25 | 光纤也慢；推荐ASF、AWS、地区镜像 | 问题并非用户带宽一个原因；地区和归档范围也影响可用性 |
| [STEP SNAP/ASF使用经历](https://forum.step.esa.int/t/do-you-download-sentinel-data-via-the-product-library-in-snap/13864?page=2) · 2019至2025后续 | 美国用户称ASF更快；后来仍有人检索正常下载失败 | 历史速度个案，不保证当前中国网络或代理表现 |
| [PEPS下载器维护者](https://github.com/olivierhagolle/peps_download) · 当前README | 旧PEPS失效，转GEODES/EODAG | 应采用GEODES，不继续增加失效入口 |
| [Earthdata VIIRS云分发讨论](https://forum.earthdata.nasa.gov/viewtopic.php?t=6535) · 2025-03-10 | LAADS延迟后询问AWS，支持人员确认特定LPCLOUD产品 | MODIS/VIIRS可选实际不同档案，但编号必须相符 |
| [Earthdata HLS/子集比较](https://forum.earthdata.nasa.gov/viewtopic.php?t=7149) · 2025-08-19 | AppEEARS订单约2小时；部分集合不支持Harmony裁剪 | 加工服务可能排队，不能当无等待的高速镜像 |
| [CDSE openEO vs Sentinel Hub](https://forum.dataspace.copernicus.eu/t/openeo-takes-much-more-time-than-sentinelhub/2975) · 2024-12 | 特定批任务7分40秒，同区域处理API30秒 | 对小AOI测试同步路径；此数值不泛化为所有任务 |
| [GIS StackExchange 四波段NAIP](https://gis.stackexchange.com/questions/142209/where-to-download-4-band-naip-imagery-for-california) · 2015原问题/后续回答 | 三波段可视化不能提供NIR | 不能为了速度把用户的科研四波段默默换成RGB |

## 公众号“免费资源”需要进一步拆解

这轮也搜到了 [免费下载高清卫星影像](https://mp.weixin.qq.com/s/bd8wmr3bHidLdUrbGIsutA)、[全国高清影像网盘](https://mp.weixin.qq.com/s/i8o1Zt6Wx_B6A1wos2zVsg)、[全国33.7GB高程包](https://mp.weixin.qq.com/s/nuLXGnNRnLKDG_Ue8J6ARg)。这三条登记为**搜索索引中的公众号线索，未核实发布日期、当前分享有效性和重分发许可**。

对应厂商[说明](https://rivermap.cn/docs/show-2277.html)显示部分包是DAT/IDX离线地图，部分导出要VIP。它们可能适合制图浏览，但不能直接替代带采集日期、科学波段、QA、投影和处理级别的原始遥感产品。网盘包要先核来源、许可、完整性、格式和日期；暂不进入自动下载兜底。

## 淘汰或降级的旧线索

- **PEPS → GEODES**：旧入口已关闭，参考 [CNES迁移说明](https://geodes.cnes.fr/peps-clap-de-fin-pour-la-plateforme-francaise-dacces-aux-donnees-sentinel-peps/)。
- **EO Browser /旧 Sentinel Hub Dashboard**：已于2026-03-20停止访问，采用当前 [Planet/Copernicus入口说明](https://community.planet.com/product-updates/important-update-sunset-of-eo-browser-and-sentinel-hub-dashboard-6426)。
- **Google Landsat C1公共档案**：当前 [Google公共数据文档](https://docs.cloud.google.com/storage/docs/public-datasets)仍标C1。FeLS、pylandsat等旧工具不能被当成最新C2L2替代。
- **NASA旧MODIS Data Pool直链**：需要按 [NASA迁移说明](https://github.com/nasa/MODIS-VIIRS-Data-Resources/blob/main/guides/after_datapool.md)更新集合/provider和授权；客户端名字不是新镜像。
- **旧国内FTP/CEODE/网盘地址**：只有历史文章，不等于当前可读、可下载或可商用；保留线索和失败状态，不计入已接入数量。

## 转成接入工作的顺序

1. **先做同景比较**：Landsat先查GSCloud/AI Earth/PIE是否真的有同日期C2L2、完整目标波段；S1用ASF和AWS GRD，S2用GEODES/GCS；MODIS/VIIRS按产品号匹配LPCLOUD/LAADS。不能以替代产品的速度证明原目标已完成。
2. **无新账号样本先验收**：JAXA Earth、OpenTopography静态STAC、OpenLandMap，走目录→集合→资产→真实文件→完整解码→来源/许可→软件内地图与成果显示。要检查STAC版本和相对链接，现有协议实现不代表零适配。
3. **区域任务减量**：AppEEARS、CDSE Process、AI Earth/PIE/GEE导出应显示实际波段、原产品、重采样/重投影、时间合成、QA及分辨率；保留完整原件选项。
4. **费用归属分开**：用户自带平台账号是个人平台额度；若用我们的托管账号则要有商用许可、服务器鉴权、预算和计量。OpenTopography商业API明确需要Enterprise，不可把免费个人Key共享给用户。[开发者政策](https://opentopography.org/developers/)
5. **每条路线都测失败恢复**：过期链接刷新、Range是否支持、已下载字节校验、压缩包/像元解码、空结果、限流/鉴权/网络错误分开。不因目录请求成功就标“已接入”。

第一批建议形成8组样本：GSCloud/国内账号路线、GEODES、ASF、AWS S1 GRD、NASA LPCLOUD/LAADS、JAXA Earth、OpenTopography静态STAC、OpenLandMap；AppEEARS作为区域加工路径另测，ORNL先定位新日期返空问题。

这份研究已保存为可执行的候选与证据清单。它没有修改产品支持状态，也没有将尚未完成真实下载验收的候选写成已上线能力。

