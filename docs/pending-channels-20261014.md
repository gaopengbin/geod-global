# 数据源接入待办清单 — 2026-10-14

本页当前数字更新至 2026-10-15；历史批次账本保留原时点。[2026-10-13 清单](pending-channels-20261013.md)及各历史验收文件保留原文。本次直接读取当前 `prototype/src/source-directory.js` 的 `SOURCE_DIRECTORY`，没有用旧研究 CSV 或计划数量推断当前状态。

## 当前目录和本批范围

| 状态 | 目录条目数 | 含义 |
| --- | ---: | --- |
| Ready | 110 | 具备可操作入口，含具体产品、通用协议、地图/矢量、用户 GEE 导出和本地文件能力；每项完整验收范围另列 |
| Auth | 2 | Copernicus 与 GSCloud 仍有账号或交付范围待办；GSCloud 已验证官网登录及六类公开地形产品完整 ZIP 下载，其他产品与预订尚未验收 |
| Planned | 21 | 仍有接入范围缺口，含具体产品、额外产品家族和平台门户；GEBCO / Wayback / Earth Search / Planetary Computer / MODIS 更多集合及 MERIT IHU 仅开放有限动作 |
| 合计 | 133 | 产品和连接路径的目录条目数，不是 133 个独立机构或全部原件均已验收 |

## 本批：五个 NASA Auth 验收与一个原 Planned 核销

当前 **133 / 110 Ready / 2 Auth / 21 Planned**，仍剩 **23 条目录/账号待办**。[GSCloud](gscloud-integration-20261015.md) 从 Planned 转为 Auth：公开目录、官网登录及六类地形原件 ZIP 已实测，原生队列下载与重开通过；[本批新增五类公开产品](gscloud-public-originals-20261015.md)。ZIP 内为 ERDAS IMG 或 DEM / NUM GeoTIFF，尚不支持压缩包内预览；其他产品和预订仍待接入，因此暂不核销平台级原件待办，也没有增加目录 ID。

此前批次完成 **5 条 Auth 原件验收 + 1 条原 Planned 核销**，从当时 29 条降至 23 条待办。原 36 条 Planned 中已完成 14 条，1 条 GSCloud 转入 Auth，21 条仍为 Planned。原生 `/datasets` 是 **72 个产品 / 114 个数值图层选项**，另有 16 个固定影像/原件提供方；GSCloud 的 124 个公开目录产品不混入已验收产品计数。

[五个 NASA 渠道](nasa-original-acceptance-20261015.md)包括 HLS L30、SRTMGL1 与三类 VIIRS，七件完整原件、九件原网格准备波段、独立完整数组与普通重开通过。[SRTM NUM](srtm-num-original-acceptance-20261015.md)是单独的来源码产品，完成一件北京完整原件、整幅数值、统计、预览和重开。Ready 仍只表示已验收的产品链路，不代替用户自己的授权或所有全球分幅的逐件下载测试。

[GEBCO 2026 全球 TID](gebco-tid-originals-20261015.md)也完成八件原 TIFF、56 组原值、全部预览和普通重开；它是来源类型，不是全球高程。父 `gebco` 保留 Planned，原计划核销 0。整幅高程、更多产品家族及门户仍按下方范围继续保留。

## 上一批 EnMAP：固定官方高光谱示例

既有 `enmap` 在**一幅官方内华达 L2A 示例**范围内晋升 Ready：完整原包与 224 波段 ENVI BSQ、HDR 身份均核对，软件下载、波段地图、原始数值及正常重开通过。原计划核销 **1 / 新目录 ID 0**；其他地区、其他处理级别及全球目录检索仍未接入，不将波段数作为渠道数。[实际验收](enmap-original-acceptance-20261015.md)。

## 新增有限动作：MERIT Hydro IHU

既有 `merit-hydro` 下接入作者 IHU v1.0 的 5 角分流向、上游汇水面积和河流级序。三个完整软件下载共 27,481,019 B、整件 SHA、29 次原值、地图与正常重开通过；原 90 米产品仍待授权，父条目保留 Planned + partialAction。**本次原计划完整核销 0 / 新目录 ID 0**，不是以较粗产品替换原版清账。[本次原件验收](merit-ihu-originals-20261015.md)。

## 历史 CLCD 批次： 原计划核销 1 项，额外新增 0 项

固定 v1.0.5 全国 2004 年 CLCD 原件通过完整软件下载、整件 SHA、原始分类颜色、161 次原值/旗标、正常重启与实际 GUI 重开；Explore 新检索复用完整原件，新增下载为 0。仅原稳定 ID `clcd` 晋升 Ready，其他年份、省级文件没有计入。[CLCD 完整原件验收](clcd-national-original-20261015.md)。

EnMAP 批次结束时为 133 / 104 Ready / 6 Auth / 23 Planned，原 36 条 Planned 累计核销 13；本批之后的最新数字见上表。224 个光谱波段只算一个产品，不按波段增加渠道数。

[OISST 与 MCD64A1](additional-pc-original-acceptance-20261015.md) 已完成三个有限原件闭环，父 `planetary-more` / `modis-more` 仍 Planned，核销 0 / 新增 0。[GEBCO 区域子集](gebco-regional-subsets-20261014.md)也保留有限动作，未核销整个产品范围。

## 上一批 SinoLC：历史账本

本批核销原稳定 ID **`sinolc-1`**，仅对应 2023 年 3 月固定发布的上海完整分类 ZIP/TIFF。软件第 5 次同任务恢复完成完整原件，实际“我的数据”分类地图、原值、普通重启重开，以及 Explore 重新检索后复用同一原件均通过，新增下载任务为 0。粗 34×30 颜色预览保留 132000×114000 原网格；混合 2019–2021 时间、NoData 0、未解释代码和分类数据许可边界不变。SinoLC 原 ZIP 的浏览器/UI 导出仍未执行验收。[SinoLC 原件验收](sinolc-shanghai-original-20261014.md)、[公开摘要](sinolc-shanghai-original-20261014.json)。

| 本批进度 | 数量 | 计数边界 |
| --- | ---: | --- |
| 原 Planned 完成并核销 | **1** | 仅 `sinolc-1` 的固定上海完整分类原件 |
| 清单外新增并接入 | **0** | 没有新增源 ID；ZIP、成员、类别和原值不重复计数 |
| 原 36 条 Planned 累计核销 | **11** | 前轮十项 + 本批 SinoLC 一项 |
| 原 Planned 仍未核销 | **25** | GEBCO、Wayback、Earth Search 更多集合的有限动作不核销整个原范围 |
| 原 Auth 账号原件验收 | **6** | 不以公共分类或区域子集替代真实账号验收 |

## 前轮原计划核销十项（历史账本）

前轮从原来的 36 条 Planned 中完成并核销 **10 个原稳定 ID**：`worldclim`、`fabdem`、`glc-fcs10`、`glc-fcs30d`、`soilgrids`、`vantor-open`、`ms-buildings`、`geofabrik`、`overture-maps`、`local-raster`。这些范围的真实 Explore / 本地导入、完整原件、原 SHA、数值或原属性、地图和普通重开已通过，矢量及本地原件的实际导出哈希另列。原目录总数仍为 133，没有用清单外产品或换变量、波段、年月、成员、分区充数。[前轮有限范围验收](original-backlog-channel-acceptance-20261014.md)与[完整哈希摘要](original-backlog-channel-acceptance-20261014.json)。

| 前轮进度 | 数量 | 计数边界 |
| --- | ---: | --- |
| 原 Planned 完成并核销 | **10** | 十个原稳定 ID，各自保留有限发布范围与实际原件/UI 回执 |
| 清单外新增并接入 | **0** | 没有新增源 ID；变量、波段、成员和分区均不另算渠道 |
| 原 Planned 仍未核销 | **26** | Wayback、Earth Search 更多集合仍 Planned；有限动作不核销整个家族 |
| 原 Auth 账号原件验收 | **6** | 未改状态，不以公共产品验收替代账号验收 |

前轮结束时为 **133 / 101 Ready / 6 Auth / 26 Planned**，当时还剩 32 条目录/账号待办；本批后的最新数字见上表。Ready 只覆盖下面所链接的有限产品：WorldClim 非商用条款、FABDEM CC BY-NC-SA、Vantor CC BY-NC、微软 CDLA、2017 SoilGrids ODbL 等各自保留，不作统一 CC 授权。GLC 两整包已正常完整软件下载；粗略显示未替换其完整原网格，GLC30D 的 23 波段年度文件仍不支持。

## 先前清单外扩展批次（历史账本）

以下保留上一批“清单外新增 10 / 原计划核销 0”的事实，不用前轮十项或本批 SinoLC 覆盖历史。其当时的 133 / 91 / 6 / 36 是原十项核销批次起点，当前进度见上表。


用户要求继续接入剩余渠道时，应优先推进既有待办。上一扩展批次实际新增了清单外的十个产品，**原来的 36 条 Planned 核销为 0**；新增产品有真实接入与验收，但不能报告为消减了原待办。这是本批推进顺序的偏差，不能通过把未完成条目标记为 Ready 来修正数字。

| 本批进度 | 数量 | 计数边界 |
| --- | ---: | --- |
| 原 Planned 条目完成并核销 | **0** | 必须对应原稳定 ID、明确完成范围及实际验收 |
| 清单外新增并接入 | **10** | 单列新增扩展，不抵扣原 Planned |
| 原 Planned 目录条目仍未核销 | **36** | 其中混有具体产品、平台/更多产品家族和本地能力，不能称为 36 个有限产品任务 |
| 原 Auth 账号验收仍待完成 | **6** | 与新公共产品接入分别统计 |

后续优先从下面的原清单选择具体待办，记录原 ID、完成标准、验收结果与核销数；有阻碍则保留实际原因。平台门户与“更多产品”家族另列范围和具体子任务，不能用长期保留的大入口当作有限剩余产品总数。用户明确要求扩展新范围时，将新增量单独列出。每批报告必须同时给出“原计划完成几项”和“额外新增几项”，不再仅以 Ready 总数增长说明原计划进度。

上一扩展批次结束时为 **36 条待对接目录条目，另有 6 条待完成账号原件验收，共 42 条待办，数量没有减少**。前一批从 114 / 71 Ready / 6 Auth / 37 Planned 新增九个具体条目，并将既有 `srtm-gl3` 晋升 Ready，得到 123 / 81 / 6 / 36。这一批再新增十个独立具体产品，当时为 **133 / 91 Ready / 6 Auth / 36 Planned**，由实际目录及选源回归读回确认；没有将平台家族条目提前晋升，或把换年月、变量和图层重复计数。

这一批新增 GHSL 建成区面积、体积、常住人口、ETH 冠层高度、HydroSHEDS 汇流积累、ETOPO 大地水准面分离值、HREA、BII、NOAA OHC 与 NClimGrid 十个独立产品。HydroSHEDS ACC/ACA 是一个产品两层，NClimGrid 四变量也只计一个产品。**实际 UI 发起的 11 件完整原件均 succeeded，共 1,429,629,635 字节**；完整 SHA、66 个代表原值/旗标及 PNG 经标准重启后不变，11 个工程真实 UI 重开的成果地图与参考原值全部通过，无新增传输。V 与 BII 的真实恢复记录保留；仅这些有限原件闭环通过，Hydro 暗全幅预览及 BII 原 mask 排除点严格 raw 400 仍明确保留。[本批验收记录](new-public-channel-next-ten-20261014.md)与[公开摘要](next-ten-public-channel-acceptance-20261014.json)不扩张为全索引逐件验收。

## 前一批已完成的十项

前一批十项：SGLI 蒸散、SGLI 蒸散指数、SGLI/MODIS 两种独立气溶胶光学厚度、AMSR2 海面风速、AMSR2 微波海温、GSMaP SPI、SRTMGL3、GEDI 平均 RH100、SRTM15+。十件完整软件原件共 541,137,211 字节，完整哈希、各 6 个原像元、PNG 及私有重启重开均通过。SRTM15+ 坏 overview 已限定所验收原件 SHA 选择同一文件有效层修复并复验；旧透明 PNG 保留。[前一批十项真实验收记录](new-public-channel-ten-products-20261014.md)明确科学、许可和固定发布范围，原文件与该轮回执保持不变。

**前一批十项实际 Explore 浏览器闭环和 UI store 重启重开均已通过。**真实分类选源、检索、十项地图、保存与完整下载、成果打开、十个 UI 原值及普通重开已有回执；浏览器独立下载十件原件共 541,137,211 字节。重启后 10 jobs、10 projects、10 snapshots、10 个包含 PNG 的检查对象及 60 个原值/旗标逐字段不变，没有新增传输。双半球多选、独立隐藏/恢复和 960 像素窄布局通过；同名图层名称已用经度前缀区分，最终浏览器回读、窄窗口截图和本地构建均通过。上述既有成功不代替这一批十项的实际新 store 回执。Ready 仍不表示所有分幅、地区、波段、版本或网络场景都已验收；每项以具体回执为准。

JAXA Earth 已接多个有限具体产品，`jaxa-earth` 全平台入口仍为 Planned；更多 MODIS、VIIRS、Earthdata 家族仍待逐产品细化。不能将有限日期文件或一个分幅的原件通过，扩张成全平台全部产品均可用。商业影像仅聚合用户已有授权数据，产品暂不承担采购。

## 当前仍未核销的 21 条 Planned

| 范围 | 原稳定 ID |
| --- | --- |
| 具体产品及授权接入 | `usgs-landsat`、`commercial-imagery`、`piesat-open-samples`、`dynamic-world`、`irsamap`、`merit-hydro`、`gebco`、`era5-land`、`eog-nightlights`、`resdc-cnlucc` |
| 额外产品家族及历史底图 | `modis-more`、`viirs-more`、`nasa-more`、`earth-search-more`、`planetary-more`、`copernicus-more`、`wayback`；其中 Earth Search、Wayback、Planetary Computer 与 MODIS 仅开放有限动作，不核销整个家族 |
| 平台门户 | `esa-tpm`、`jaxa-earth`、`geodata-china`、`tpdc`；`gscloud` 已转 Auth，原件待办仍保留 |

## 前轮十项开始前的 36 条 Planned 基线名单

| 分组 | 数量 | 基线待接条目（原名单保留；前轮十项及本批 SinoLC 按各自账本核销） |
| --- | ---: | --- |
| 光学与航片 | 7 | `usgs-landsat`：USGS Landsat 直连；`modis-more`：更多 MODIS 产品；`viirs-more`：更多 VIIRS 产品；`nasa-more`：更多 NASA Earthdata 产品；`commercial-imagery`：吉林一号/商业影像授权入口；`vantor-open`：较新 Vantor 灾害事件影像；`enmap`：EnMAP 高光谱 L2A |
| SAR 雷达 | 1 | `piesat-open-samples`：PIESAT-1 免费 SAR 样例 |
| 高程与海底地形 | 3 | `srtm-num`：SRTM NUM 质量/来源产品；`fabdem`：FABDEM；`gebco`：GEBCO 海底地形 |
| 土地覆盖 | 6 | `dynamic-world`：Dynamic World；`glc-fcs30d`：GLC_FCS30D；`glc-fcs10`：GLC_FCS10；`clcd`：CLCD 中国年度土地覆盖；`sinolc-1`：SinoLC-1；`resdc-cnlucc`：RESDC CNLUCC |
| 水文 | 1 | `merit-hydro`：MERIT Hydro |
| 土壤 | 1 | `soilgrids`：SoilGrids 250 m |
| 气候 | 2 | `era5-land`：ERA5-Land；`worldclim`：WorldClim 2.1 |
| 夜间灯光 | 1 | `eog-nightlights`：EOG VIIRS 夜光 |
| 矢量数据 | 4 | `irsamap`：IRSAMap 标注样本；`ms-buildings`：微软全球建筑轮廓；`overture-maps`：Overture Maps；`geofabrik`：Geofabrik 区域 OSM 包 |
| 历史影像底图 | 1 | `wayback`：Esri Wayback |
| 更多栅格目录/产品 | 3 | `earth-search-more`：Earth Search 更多集合；`planetary-more`：Planetary Computer 更多集合；`copernicus-more`：Copernicus 更多产品 |
| 本地文件 | 1 | `local-raster`：独立本地 GeoTIFF/COG 导入 |
| 平台门户 | 5 | `esa-tpm`：ESA Third Party Missions；`jaxa-earth`：JAXA Earth 全平台入口；`geodata-china`：国家地球系统科学数据中心；`tpdc`：国家青藏高原科学数据中心；`gscloud`：地理空间数据云 |
| 合计 | **36** | 每个稳定 ID 只计一次；SRTMGL3 已晋升，不再重复列为 Planned |

## 当前 1 条 Auth

仅 `copernicus`：既有访问令牌已过期，尚无新的完整 SAFE 原件验收。不能用公开 COG 或 NASA 原件核销此账号路径。

## 原 6 条 Auth 基线（历史）

| 稳定 ID | 产品 | 当前边界 |
| --- | --- | --- |
| `nasa-viirs-noaa21` | NASA VIIRS NOAA-21 | 受保护原件需真实 NASA Earthdata 账号验收 |
| `nasa-viirs-noaa20` | NASA VIIRS NOAA-20 | 同上 |
| `nasa-viirs-npp` | NASA VIIRS Suomi-NPP | 同上 |
| `nasa-srtm` | NASA SRTMGL1 | 原 NASA 交付需账号验收；已有 OpenTopography 公共 COG 与本批 GL3 是其他明确路径 |
| `copernicus` | Copernicus Data Space Sentinel-2 | 受保护原始产品需真实授权和完整原件验收 |
| `nasa-earthdata` | NASA HLS Landsat L30 | 需真实账号、原件和科学产品验收 |

## 下一步优先队列

以下是基于当前目录和已有研究的工程安排，不是新实时下载成功声明。每项应重新核对可取得的真实发布件、许可及软件完整闭环。

| 优先级 | 工作 | 完成标准 |
| --- | --- | --- |
| P1 | 已接入条目的残余显示/解码限制与下一批产品 | 此前清单外十项的 11 件实际下载、66 个代表像元、PNG、成果地图及重启重开已完成，不再重复列待验。保留 Hydro 全分幅几乎黑色与 BII 指定 mask 排除海域点严格原生 400；后续只按实际科学/资源边界修复，不把零值或不可解码点伪装为通过。 |
| P1 | 既有 WPM/WFI 等不稳定在线预览和后续新增产品 | 前一批十项（含 GEDI 和 SRTM15+）实际 Explore、成果打开及重启重开已完成，不再重复列作待验。继续处理旧批不稳定地图，并对后续每个新增产品独立保留完整原件及实际 UI 回执，不能以本地 PNG 替代在线验收。 |
| P1 | 尚未接入的具体 MODIS/VIIRS/JAXA 产品 | 选择新的独立科学量和真实发布原件，不重复已接产品或用换年月/波段凑数。原单位、NoData/错误码、采集区间、发布层级与科学 QA 分别核查。 |
| P1 | 当前接入范围外的更多 USGS 1 m 项目 | Santa Cruz 独立原件已验证，但历史 TNM 目录及控制查询有上游错误；须取得真实 record 并完成软件闭环才扩大现有单新泽西瓦片范围。不能伪造目录。 |
| P2 | GLC_FCS30D 范围外的年度产品 | 已接并验收三波段 1985/1990/1995 历史文件。2000–2022 的 23 波段原件及旧 FTPS 路径仍待安全受限接入，不删波段或放宽解码预算。 |
| P2 | 范围外建筑矢量、区域样本和压缩包产品 | 微软预算内整分区、有限 GLC10 分幅已有前轮原件和 GUI 回执；SinoLC 仅固定上海完整原件已通过本批验收。IRSAMap、CLCD 其他年份、SinoLC 其他城市、超预算分区和其他国家大包仍分别核查；现有受限 ZIP 基础不等于全部原件可用。 |
| P3 | 账号、专项授权和复杂科学格式 | EnMAP、PIESAT SLC、ERA5-Land、ESA TPM、国内门户等按真实可取得授权细化产品和格式，不提前承诺无授权下载。 |

## 已接入范围与不稳定预览

| 已接入产品或路径 | 当前证据 | 仍需保留的边界 |
| --- | --- | --- |
| SinoLC-1 固定上海原件 | 完整软件 ZIP/TIFF 双 SHA、CRC、7 个原值、实际分类地图、“我的数据”原值与普通重启重开通过；Explore 重新检索复用同原件、新增下载任务 0，点击原代码 1 经实际成员 Rasterio/GDAL 独立比对 | 34×30 粗分类预览，132000×114000 原网格保留；混合 2019–2021 时间、NoData 0、代码 11/250/255 未解释；仅分类数据 CC BY 4.0，SinoLC 原 ZIP 的浏览器/UI 导出未执行。 |
| GEBCO 官方区域数值子集 | 实际 Explore 生成、数值地图/原值、完整官方 ZIP 与数值 TIFF 双 UI 导出、普通重启重开通过 | 官方生成区域子集不替代全球静态原始整包或 TID；原 ID 仍 Planned + partialAction，核销 0 / 新增 0。 |
| 此前清单外新增十项 | 11 件软件完整原件及 SHA、66 个代表原值/旗标、PNG、原快照/工程在同一内核标准重启后逐字段不变；11 个实际 UI 工程重开的成果地图与参考原值均通过，无新增传输 | ETH 2,651 件原目录、Hydro/Geoid 355 件头部与 PC 四个有限 Item 不代表逐件验收；Hydro 全幅几乎黑色，BII 列7689/行4010原 mask 排除点严格 raw 400 保留，未计为额外通过采样。 |
| 前一批已完成十项 | 十件独立完整原件及软件实际下载、完整哈希、60 个原像元、PNG、私有重开通过；另外十项实际浏览器地图/完整下载/打开/原值和重启重开通过 | 每项仅对应固定产品及验收原件。JAXA 日海温仅固定 2026-10-11，月份不是整个日档案入口；实际 UI 成功不补齐科学 QA 或扩大完整文件范围。 |
| SRTM15+ `02_10` | 116,895,811 B 软件完整原件；坏 4/8/16 倍 overview 原样保留，限定完整 SHA 选择有效 2 倍层的原生 PNG 复验通过；实际在线地图和 UI 策略说明通过 | 修复仅此原件，不能扩张为全部目录条目已完整验收或坏层均已通用修复。 |
| GEDI 平均 RH100 | 407,711,565 B 完整发布衍生件、所有 547,680,042 像元独立解码和原生检查通过 | 已由发布方重投影，实际有效观测约南北纬 52°且不连续；原件宽到约 85°不代表极地观测，小 AOI 不减少整件体积。 |
| AMSR2 SST / GSMaP SPI | 原数值、源填充值声明与缺少 TIFF NoData 标签均保留；原生 scalar 旗标 false 如实比对通过 | 填充值不是温度、极端干旱或有效观测；灰度 stretch 不能伪装科学质量掩膜。 |
| 既有 CBERS-4A WPM | BAND1 软件完整原件 174,662,589 B、6 原值、PNG、重开通过；PAN 在原生接口拒绝 | 既有在线预览仍不稳定；不能扩张为四个波段或 2 m PAN 均验收。 |
| 既有 AMAZÔNIA-1 WFI | BAND1 软件自身真实中断后经 3 次尝试与源绑定恢复完成；163,671,005 B、完整 SHA、6 原值、PNG、重开通过 | 四波段在线地图仍不稳定，BAND2–4 完整下载尚未验收。原值不是反射率，STAC/原 TIFF 冲突保持。 |
| 既有 SGLI SST 与 OpenLandMap 夜光 | 上批实际浏览器地图、原件下载、打开工程和原值检查通过 | SGLI 仅 2026-09 白天东北原件；夜光仅 2024 年作者处理 DN，未解决的定标冲突不变，不能宣称物理辐射单位或真实 resume 已验收。 |

上述预览或其他科学边界属于已接入条目的后续验收，不重复计为 Planned。Ready 不会消除真实网络失败、坏金字塔、缺失 QA 或授权限制。

## 其他研究阻碍

| 候选 | 已有证据与仍未解决的边界 |
| --- | --- |
| AMSR2 北极海冰 | 原件可完整下载/解码，但 publisher Item bbox/geometry 是 EPSG:3995 投影米，当前标准地理 Item/地图路径不直接支持。没有合成经纬 bbox，也不列为已接入。 |
| SoilGrids | 前轮已接并验收 corrected WRB 2017 的 530/531、最可能类别 / Acrisols 原始分数四个 250 m 名义原件（ODbL）。历史 1 km pH 的自定义 Interrupted Goode Homolosine 无 EPSG 路径仍不支持；不冒称最新深度属性或用聚合件替换 250 m 原预测。 |
| Vantor 较新事件 | 仅 Nepal B120001101250A10、2026-09-07 完整 JPEG YCbCr 视觉原件已真实软件下载、解码与 GUI 原值重开；Y/Cb/Cr 为视觉组件、Y 灰度预览，非 RGB / 科学定标，93% 云量保留。其他事件、分片及旧 Maui 分析路径不互相替代。 |
| iSDA | 正式目录及 21 个产品体量已研究，文件约 3.48–57.23 GB，部分轴长超限；没有正式产品 Item 的小 test.tif 不替代。 |
| WorldClim | Nepal NPL 30 角秒 tavg / prec 两个完整 12 波段月常年值已真实软件及 GUI 验收，共一个原 ID。保留 1970–2000、学术/非商用与商业/再分发事先许可；其他国家、分辨率、变量和 19 波段 bioclim 不在当前范围。 |
| FABDEM | V1-2 整 ZIP 21,627,700 B 及 N08E050 / N09E050 完整 TIFF 已安全受限解包，独立 archive/member SHA、实际合并检索、多选下载和重开通过；CC BY-NC-SA 4.0。另一个 Vietnam 2021 的 962,040,263 B / CC BY 4.0 原件仍超 512 MiB，不套用其许可或放宽预算。 |
| GEBCO | [官方区域数值子集](gebco-regional-subsets-20261014.md)已真实生成、地图/原值、双 UI 导出和重启重开；原 ID 仍 Planned，只开放有限动作。全球静态原始整包及适合现有预算的单地形原件仍待接入；TID 是来源类别而不是水深，区域子集不称原件或抵扣全球待办。 |
| EOG 夜光 | 既有匿名年度路径返回 401；具体产品授权流程与真实账号原件验收仍待完成。 |
| Overture / Geofabrik | Overture 仅完整 2026-09-23.1 bathymetry GeoParquet part，原件与 AOI 预览分开；Geofabrik 仅 Monaco 2026-10-11 免费 Shapefile 整 ZIP，非 raw PBF。两项完整原件导出、地图/属性和 GUI 重开通过；其他主题、国家及大型 PBF 仍未覆盖。 |

前轮十个原 ID 及本批 SinoLC 的有限范围已有各自真实软件和 GUI 回执；其余未核销阻碍仍以已有历史研究为准，未重新逐个运行网络测试，不把旧探测结果描述为今天的实时服务状态。细节见 [历史研究和证据索引](pending-channels-20261013.md#接入范围已知阻碍及已有证据)。

## 最新证据索引

- [本批 SinoLC 固定上海原件验收（原核销 1 / 额外新增 0）](sinolc-shanghai-original-20261014.md)
- [SinoLC 完整原件、失败恢复、实际 Explore 与普通重开摘要](sinolc-shanghai-original-20261014.json)
- [GEBCO 官方区域数值子集验收（原核销 0 / 额外新增 0）](gebco-regional-subsets-20261014.md)
- [GEBCO 区域路径公开摘要](gebco-regional-subsets-20261014.json)

- [前轮原 Planned 十项有限范围验收（原核销 10 / 额外新增 0）](original-backlog-channel-acceptance-20261014.md)
- [前轮完整原件、双 SHA、GUI 与存储恢复摘要](original-backlog-channel-acceptance-20261014.json)

- [这一批新增十项完整原件与重启验收](new-public-channel-next-ten-20261014.md)
- [本批公开验收摘要](next-ten-public-channel-acceptance-20261014.json)
- [GHSL 三个物理/统计量原件研究](research/next-ghsl-20261014.md)
- [ETH 冠层高度原始目录与完整原件研究](research/next-eth-canopy-20261014.md)
- [HydroSHEDS 汇流积累与 ETOPO 大地水准面研究](research/next-hydro-geoid-20261014.md)
- [四个有限 Planetary Computer 产品与原始 mask 回归](research/next-planetary-20261014.md)
- [前一批十项原生、实际 UI 与重启重开验收](new-public-channel-ten-products-20261014.md)
- [JAXA 四项陆地/大气研究](research/ten-jaxa-land-20261014.md)
- [JAXA 三项海洋/气候研究](research/ten-jaxa-ocean-20261014.md)
- [公开 GL3、GEDI、SRTM15+ 原件及坏 overview 修复](research/ten-other-public-20261014.md)
- [上一轮 SST、夜光、AMAZÔNIA-1 实际验收](new-public-channel-sst-nightlights-amazonia-20261013.md)
- [上一轮 WPM、日本概览及辐射实际验收](new-public-channel-landcover-radiation-wpm-20261013.md)
- 当前目录、入口及发布策略：`prototype/src/source-directory.js`、`prototype/src/providers.js`、`prototype/src/release-policy.js`。

完整下载、原始数值、实际在线地图、科学产品质量、使用许可、真实账号、模型流程、安装版和生产发布应分别留下证据。此快照不将某一层成功扩张为产品全链路、平台全目录或未执行的发布验收。
