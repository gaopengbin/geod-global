# 数据源接入待办清单 — 2026-10-13

## 统计口径

本次审计直接运行当前 `prototype/src/source-directory.js` 模块读取 `SOURCE_DIRECTORY`，没有用旧研究 CSV 的“待接入”字段推断软件现状。

| 状态 | 目录条目数 | 含义 |
| --- | ---: | --- |
| Ready | 71 | 已有操作入口，包含具体产品、通用协议、地图/矢量、用户 GEE 导出和本地文件入口 |
| Auth | 6 | 已有目录或授权适配，受保护原件仍等待真实账号验收，当前不能据此宣称原始下载已开放 |
| Planned | 37 | 尚无可用操作入口，包含具体产品、额外产品家族、平台门户及本地能力 |
| 合计 | 114 | 产品与连接路径的目录条目数，不能当作互不重复的图源机构数或全部真实下载验收数量 |

因此当前有 **37 条待对接目录条目，另有 6 条待完成账号原件验收**，共 43 条待办。这一口径不表示 43 个相同难度的卫星源。新增独立产品可能增加 Ready 和总数，同时保留同平台的通用 Planned 入口。

例如 JAXA Earth 已有多个明确产品接入，但 `jaxa-earth` 全平台通用入口仍为 Planned；这不否定已接入产品，也不能因为几个产品通过验收，就宣称整个 JAXA 目录都能下载。已接入具体产品还可能只覆盖固定年份、月份、主题或区域，应查对应验收记录。

此前新增 GSMaP 降水率，并将既有 1 m DEM 条目晋升为单一新泽西瓦片接入。上一批新增 JAXA 日本土地覆盖概览和 Terra MODIS 短波辐射两个具体产品，将既有 `cbers-4a` 晋升为 CBERS-4A WPM L4 DN 接入。更多 1 m 项目和全国无缝覆盖仍未实现。

上一批三个产品的 fresh 私有原生闭环已通过。日本土地覆盖和短波辐射在浏览器与桌面实际地图显示通过；WPM 在线预览修复后仍有范围读取失败，桌面单 BAND1 初次及一次重试均未显示，浏览器四波段也未达到稳定显示，保留为明确的预览待办。WPM 的 BAND1 原件为 174,662,589 B，软件新完整下载耗时 178.312 秒；完整哈希、6 个原像元、PNG、工程重开均通过，PAN 在保存、预览和下载三个原生 API 被拒绝。此结果不扩张为四个多光谱波段或 2 m PAN 全部验收。JAXA 日本概览仅为 2024 年 0.025° 分类概览；短波辐射接入保留 2026-09 发布文件实际声明的 9 月 1–15 日观测时段，两者的原件、原像元、PNG 和重开均通过。Ready 仅表示已有可操作入口，不代表所有地区、波段及网络预览均已完整验收。

本轮新增 `jaxa-sgli-sst` 和 `amazonia-wfi-dn` 两个具体产品，并将既有 `openlandmap-nightlights` 晋升 Ready。三项均通过真实产品原生解析、连接、完整下载、原像元检查、PNG 和私有重开；不是向软件下载目录注入研究原件。SST 仅接 2026-09 白天月度产品，完整原件验收限东北分幅；WFI 仅有一个 BAND1 完整原生验收，真实中断后经三次尝试、源绑定恢复完成；夜光仅接 2024 年发布者处理后的 Int16 DN，首次新完整下载成功，没有实际发生断点恢复。三项的浏览器/桌面在线地图验收应查主任务另行记录，不用本地 PNG 代替。三份新研究记录文件名中的 `20261014` 是批次标识，实际本轮运行日期仍为 **2026-10-13 UTC**。

本轮浏览器验收：SST、夜光实际地图显示、完整原件下载、工程重开与原值读取通过；WFI 三景检索及四波段多选保存、工程打开通过，但四个在线图层均未显示（BAND1/3 超时、BAND2 未返回分块、BAND4 无法读取）。这项已接入产品的预览稳定性仍为待办，未计入 37 条 Planned。详见 [本轮完整验收记录](new-public-channel-sst-nightlights-amazonia-20261013.md)。

## 37 条 Planned 全名单

| 分组 | 数量 | 当前待对接条目（稳定 ID 与名称） |
| --- | ---: | --- |
| 光学与航片 | 7 | `usgs-landsat`：USGS Landsat 直连；`modis-more`：更多 MODIS 产品；`viirs-more`：更多 VIIRS 产品；`nasa-more`：更多 NASA Earthdata 产品；`commercial-imagery`：吉林一号/商业影像授权入口；`vantor-open`：较新 Vantor 灾害事件影像；`enmap`：EnMAP 高光谱 L2A |
| SAR 雷达 | 1 | `piesat-open-samples`：PIESAT-1 免费 SAR 样例 |
| 高程与海底地形 | 4 | `srtm-gl3`：SRTMGL3；`srtm-num`：SRTM NUM 质量/来源产品；`fabdem`：FABDEM；`gebco`：GEBCO 海底地形 |
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
| 合计 | **37** | 每个 ID 只计一次 |

“商业影像授权入口”仅指用户已有授权数据的聚合，当前产品不承担采购。更多集合和国内平台门户必须细化到具体产品、资产和当前许可后再验收，不把平台宣称的资源总量计入软件可下载数量。

## 6 条 Auth

| 稳定 ID | 产品 | 当前边界 |
| --- | --- | --- |
| `nasa-viirs-noaa21` | NASA VIIRS NOAA-21 | 受保护原件需真实 NASA Earthdata 账号验收 |
| `nasa-viirs-noaa20` | NASA VIIRS NOAA-20 | 同上 |
| `nasa-viirs-npp` | NASA VIIRS Suomi-NPP | 同上 |
| `nasa-srtm` | NASA SRTMGL1 | 原 NASA 交付路径需账号验收；已接入的 OpenTopography 公共 COG 是另一条明确路径 |
| `copernicus` | Copernicus Data Space Sentinel-2 | 受保护原始产品需真实授权和完整原件验收 |
| `nasa-earthdata` | NASA HLS Landsat L30 | 需真实账号、原件与科学产品验收 |

## 下一批优先队列

下列优先级是基于现有研究和已验证内核的工程安排，并非新的实时下载成功声明。各候选仍须重新核对当前发布目录、版本和完整原件。

| 优先级 | 候选/工作 | 原因与完成标准 |
| --- | --- | --- |
| P1 | 现有新泽西瓦片以外的明确 USGS 1 m 项目 | Santa Cruz 原件已完整独立验证，但 TNM 目录与已接入新泽西控制查询同样返回上游错误，尚未接入该项目；待真实 record 恢复后核对水平/垂直基准、目录与原件一致性及完整软件闭环。不能仅凭 S3/XML 原件验证扩大现有接入范围。 |
| P1 | 尚未接入的 JAXA 独立产品及额外 MODIS/VIIRS 集合 | 继续沿已验证的公开原件路径筛选新的具体产品，不把本轮已接 SST、WFI、夜光重复列为待接入。每项分别核对原始 DN、比例因子、NoData、时间聚合和发布层级；不能将 overview 网格间距当原传感器分辨率。通用 JAXA 门户继续保留 Planned，直到其范围被实际实现。 |
| P2 | GLC_FCS30D 的 FTPS 和年度平面支持 | 原件已完整下载、独立解码。先补 TLS 控制/数据通道传输、取消/恢复及安全暂存，然后支持 23 个年度平面与年度选择；具体块解码预算见 [接入前置](research/glcfcs-integration-blockers-20261013.md)。保留不可变原件，避免整幅约 7.92 GB 样本数组进内存。 |
| P2 | 公共建筑矢量、区域样本和地形压缩包 | 微软建筑、IRSAMap、GLC_FCS10/CLCD/SinoLC 等需逐一确认实际完整包、许可、坐标系、体量和受限解包；不能以现有本地导入宣称全国大型包已可用。 |
| P3 | 账号/专项授权平台、商业授权、复杂科学格式 | EnMAP、PIESAT SLC、ERA5-Land、ESA TPM、国内门户等按实际可取得授权细化；先明确产品和格式，不做无授权下载承诺。 |

## 接入范围、已知阻碍及已有证据

| 条目/路径 | 已取得证据 | 当前限制或阻碍 |
| --- | --- | --- |
| GLC_FCS30D | 一份完整 FTPS 原件 21,832,902 B；23 个年度平面已独立完整解码 | 软件暂无 FTPS 适配；当前原始栅格解析最多 16 个样本/像元；还需年度选择与分类展示。18,554² 空间网格符合当前通用栅格上限，不能误用旧 SCL 上限解释阻碍。 |
| OpenLandMap 2024 年度夜光处理 DN（Ready） | 专用产品实际 resolve/connect/project/download 闭环完成；首次新完整软件下载 251.864 秒，63,713,625 B；完整 SHA 与独立原件、实时发布 MD5 一致；86,400 × 35,849、Int16、EPSG:4326、15 角秒原网格，7 个原值/NoData 像元、PNG 和重启重开通过 | 仅此 2024 年文件，发布者比例因子 10 与说明所暗示的 0.1 存在冲突，保留处理后 DN，不输出未经核实的物理辐射单位。NoData 仅 −32768，零和非缺失负值保持原值。此次未发生实际 resume，不能称夜光续传已实测成功；在线地图验收单独记录。旧 17,648,832 B 中断记录保留为历史，不再作为当前 Planned 阻碍。 |
| AMAZÔNIA-1 WFI L4 DN（Ready） | 固定 INPE `AMZ1-WFI-L4-DN-1`；真实 BAND1 原件 163,671,005 B 独立全像元解码完成；软件自身三次尝试经实际中断与源绑定恢复完成，总观测耗时 709.0 秒；完整 SHA、16,146 × 14,253、Int16、EPSG:32718、64 m、NoData 0、6 个原像元、PNG 和重开均通过 | 完整原生验收限所核查 scene 的 BAND1；BAND2–4 目前仅 HEAD/元数据核对。不是一次完整 GET 成功。STAC DN 校准/NoData 声明与实际原件冲突，保留两者，不当表面反射率。原件没有 overview，在线地图仍需独立验收；不能用缩略图冒充原件。 |
| JAXA SGLI 2026-09 白天海表温度（Ready） | 东北分幅 13,714,492 B 完整独立及软件自身下载通过，软件观测耗时 22.367 秒；完整 SHA、3,600 × 3,600、UInt16、EPSG:4326、0.025°、NoData 65535、6 个原像元、PNG 和重开通过；保留该分幅实际观测时段 2026-09-01T00:25:58Z 至 2026-09-30T11:03:17Z | 仅固定年月、白天 SST 层级；八个发布分幅有真实元数据，完整原件验收限东北分幅，不能扩张到其余七件。0.025° 是概览网格间距，不是传感器分辨率；保留 JAXA 许可及商用事先通知要求。浏览器/桌面在线地图验收单独记录。 |
| USGS 更多 1 m 项目（现有接入范围外） | Santa Cruz 35,443,043 B 原件与官方 S3/XML、完整哈希及全部原像元独立解码通过；实际 NAD83 / UTM 10N、NAVD88 米制 | 官方 TNM 产品目录及新泽西控制请求返回同一上游错误，未取得 Santa Cruz 真实 TNM record；暂不扩大接入范围，不伪造目录记录，不以独立研究恢复替代 native 验收。 |
| Vantor 较新事件 | 已完整传输 Nepal 2026 的一个视觉原件 | 该原件 JPEG/YCbCr 不受当前原值解码器支持；旧 Maui 数值分析原件的成功不覆盖新目录。 |
| SoilGrids | 发布者 1 km 聚合 pH 原件完整下载、发布 SHA 匹配、577,632,308 像元完整独立解码 | 自定义 Interrupted Goode Homolosine 无 EPSG 标识，当前原生/地图投影无法准确展示；1 km 聚合也不能冒充 250 m 原预测。 |
| iSDA 土壤（研究候选，未单列目录） | 正式目录、21 个产品文件体量和科学声明已核对 | 正式文件约 3.48–57.23 GB，部分轴长超限；小 test.tif 没有正式产品 Item，不能替代。 |
| WorldClim | 官方 ZIP 返回 200；测试裸 TIFF 返回 404 | 需受限 ZIP 提取及非商业/再分发许可核对，未完成原件下载验收。 |
| FABDEM | 原作者与第三方镜像路径/许可边界已研究 | 原件大 ZIP 与镜像许可差异未完成下载验证；非商业免费不能自动放入商业托管 Pro。 |
| GEBCO | 当前发布页给 8 个 90° 分幅组成的约 4 GB ZIP；TID 包较小 | 尚未完整验证适合软件字节上限的单个原始地形 TIFF；TID 是来源类别而非深度，服务生成子集不能称发布原件。 |
| EOG 夜光 | 测试年度匿名原件路径返回 401；公开说明可读 | 需要具体产品认证流程与真实账号原件验收。 |
| Overture | 发布格式与许可已研究 | 原生 GeoParquet 能力未接入；已有 PMTiles 支持不能直接当作完整 Overture 主题支持。 |
| Geofabrik | 公共区域包入口已研究 | 全国大 PBF 的完整下载队列和解析能力需独立验收。 |

## 审计来源

- 当前目录和状态：`prototype/src/source-directory.js`、`prototype/src/providers.js`、`prototype/src/release-policy.js`。
- 本次新增降雨率与 1 m 高程：[验收记录](new-public-channel-rainfall-dem-20261013.md)。
- 最新已接入植被/海洋产品：[真实验收记录](new-public-channel-vegetation-ocean-20261013.md)。
- 上一批 JAXA 日本概览与短波辐射：[原件及原生验收记录](research/jaxa-next-products-20261013.md)。
- 上一批 WPM 原生验收及 WFI 初始候选研究：[历史研究及验收记录](research/cbers-wpm-20261013.md)；WFI 当前状态以本轮专属记录为准。
- 本轮 SGLI 海表温度：[原件、科学边界及原生验收](research/jaxa-more-products-20261014.md)。
- 本轮 OpenLandMap 夜光：[真实产品原生复验](research/nightlights-native-recheck-20261014.md)。
- 本轮 AMAZÔNIA-1 WFI：[原件及软件实际源绑定恢复验收](research/amazonia-wfi-20261014.md)。三份文件名标识 20261014 批次，实际运行日期为 2026-10-13 UTC。
- 更多 USGS 1 m 原件及 TNM 阻碍：[研究记录](research/usgs-1m-next-20261013.md)。
- 土壤与 JAXA：[原件研究](research/next-channels-20261012-soil-jaxa.md)。
- 气候、夜光、GEBCO：[原件研究](research/new-channels-20261012-climate.md)。
- iSDA 原件体量：[研究记录](research/new-channels-20261012-cgls-isda.md)。
- 夜光旧软件失败与恢复条件：[2026-10-12 历史连接诊断](next2-nightlights-connection-diagnostic-20261012.md)，不代替本轮成功回执。
- GLC_FCS30D 的完整转移、独立解码和 native 阻碍：私有本地验收目录 `.verification/glcfcs-probe-20261013/`，不提交下载原件或凭据。

接入验收应继续分别记录真实发布元数据、完整原始文件、大小/哈希、原生像元、地图预览、工程落盘和重开证据。Ready 是目录操作状态，尚未完成或不稳定的在线预览须明确列为待办，不能用原生 PNG 代替地图验收；范围受限的一个成功原件，不扩张为该平台全部产品和所有区域成功。
