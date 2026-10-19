# 2026-10-14 JAXA 海面风速、微波海温与 SPI 原件验收

本记录覆盖三个此前未接入的具体产品：AMSR2 标准海面风速、AMSR2 标准微波海面温度和 GSMaP 标准化降水指数。它们不是将已有 SGLI SST、GSMaP 雨率换一个日期或波段重新计数。日产品只索引明确已核实的 **2026-10-11**；年/月字段用于定位该固定日文件，不意味着接通当月所有日或月平均温度。

## 来源与科学边界

- [AMSR 官方海洋产品说明](https://www.eorc.jaxa.jp/AMSR/datacatalog/ocean/index_en.html)：SSW 是海面风速，不包含风向和陆地风场，标准产品在海冰/降水区域存在限制，不能写成另一个全天候 ASW 研究产品。AMSR2 微波 SST 和 SGLI 热红外 SST 是不同传感器及算法产品；JAXA Earth 转换网格间距不等同于原传感器足迹或局地精度。本次不推断转换文件对应标准 HDF 的哪一个微波频率层。
- [GSMaP 官方用户指南](https://sharaku.eorc.jaxa.jp/GSMaP/guide.html)及 [JAXA 对 SPI 的说明](https://earth.jaxa.jp/en/earthview/2020/03/09/2956/index.html)：SPI 是由降水派生的无量纲干旱指标，有效负值代表较干、正值代表较湿。它不是 mm/hr 雨率、累积降水、土壤含水量或预报。本次未核实其基准期、拟合参数或累积窗口，界面不补造这些说明。
- [JAXA 研究数据条款](https://earth.jaxa.jp/en/data/policy/)：Item/Collection 声明 `proprietary`，公开免费分发不能换成 CC 授权；需保留 JAXA、研究名称及指定第三方署名，商业使用须事先通知 JAXA。本次不代发通知。

原始 HTTPS 元数据和科学说明的响应、哈希及时间保存在私有 `.verification/ten-products-20261014/jaxa-ocean/`。发布方 JSON 未改内容；独立原件下载未使用账号、私有桶密钥、代理节点变更或 Earth Engine 算力。

| 软件产品 ID / 原始资产 | 已核实固定发布件 | 原始网格 | 数值和声明 |
| --- | --- | --- | --- |
| `jaxa-amsr2-wind` / `SSW` → `ssw` | 白天月度 2026-09，路径 level-1，8 个 90° 瓦片 | 东北原件 Int16，900 × 900，EPSG:4326，0.1° | STAC：m/s = DN × 0.01，NoData −32768，错误码 −32761…−32767；TIFF scale/offset 为 1/0 |
| `jaxa-amsr2-sst` / `SST` → `sst` | 白天日产品 **2026-10-11**，路径 level-0，2 个半球瓦片 | 东半球原件 Int16，900 × 900，EPSG:4326，0.2° | 温度°C = DN × 0.01；TIFF 内嵌 scale 0.01/offset 0，**缺少 NoData 标签**；STAC 填充值为 −32768 |
| `jaxa-gsmap-spi` / `SPI` → `spi` | 月度标注 2026-09，路径 level-0，2 个半球瓦片 | 东半球原件 Float32，900 × 900，EPSG:4326，0.2° | scale 1/offset 0；无量纲；**缺少 TIFF NoData 标签**；STAC 填充值为 −999 |

0.1° 与 0.2° 在赤道约为 11.1 千米及 22.2 千米，是实际分发网格。不把较粗转换件宣传成任务分辨率；经度网格的实际距离随纬度变化。每个产品只实际完整验收一件原件，其余有限索引保持原发布方元数据，不宣称逐文件已下载。

## 未修改的发布方目录

### AMSR2 海面风速

[Collection](https://s3.ap-northeast-1.wasabisys.com/je-pds/cog/v1/JAXA.G-Portal_GCOM-W.AMSR2_standard.L3-SSW.daytime.v4_global_monthly/collection.json) 和 [实际东北 Item](https://s3.ap-northeast-1.wasabisys.com/je-pds/cog/v1/JAXA.G-Portal_GCOM-W.AMSR2_standard.L3-SSW.daytime.v4_global_monthly/2026-09/1/E090.00-E180.00/N00.00-N90.00.json)。

- 精确区间为 `2026-09-01T00:28:10.283000Z` 至 `2026-09-30T23:50:46.024000Z`；各瓦片区间按原声明保存，不改为整月午夜时间。
- Collection `id=collection` 与 Item 完整产品 ID 不一致；归档这个冲突，不添加别名或宣称匹配 Collection。
- level-1 路径和 Collection 最高层级与 Item `je:img_level` 冲突：Item 把 level-2 标为 raster，0/1 为 null。不依据单一标签写成已验收 level-2 或某个预览层，直接报告原件实测网格。
- Item 宣称有效 DN 极值 34/2662，完整原件实际为 **60/2058**，保留双方，不篡改原值或把宣称极值当作解码正确条件。
- 810,000 个像元全部独立解码：402,225 个嵌入 NoData −32768、31,356 个源错误码 −32767、376,419 个排除源缺测/错误后的存储值。错误码 −32767 的原生标量 NoData 旗标仍为 false，不将其描述为有效风速；不增加科学掩膜。

### AMSR2 微波海面温度

[Collection](https://s3.ap-northeast-1.wasabisys.com/je-pds2/cog/v1/JAXA.G-Portal_GCOM-W.AMSR2_standard.L3-SST.daytime.v4_global_daily/collection.json) 和 [实际东半球 Item](https://s3.ap-northeast-1.wasabisys.com/je-pds2/cog/v1/JAXA.G-Portal_GCOM-W.AMSR2_standard.L3-SST.daytime.v4_global_daily/2026-10/11/0/E000.00-E180.00/S90.00-N90.00.json)。

- 精确区间为 `2026-10-11T00:00:00Z` 至 `2026-10-11T23:59:59Z`；不冒称整月统计。
- 810,000 个像元全部独立解码：447,212 个 STAC 填充值 −32768，362,788 个其他存储值，176 个零值；排除填充值后的原始 DN 范围 **−200…3506**，按源定标为 −2…35.06°C。STAC 极值为 null，不人为裁剪到一般任务范围。
- **STAC NoData 与 TIFF 标签冲突**：原 TIFF 没有 NoData 标签。原生检查和像元读数按原件如实保留旗标 false，填充值不表示有效温度；不静默植入掩膜或修正标签。
- 初版研究脚本曾断言 TIFF 必须嵌入声明的 NoData，解码因此停止。这是检查程序遇到实际元数据冲突，不是下载失败。检查修正为分别记录源声明与嵌入标签，再次真实完整下载得到同一哈希；旧冲突不隐藏为科学验收通过。

### GSMaP SPI

[Collection](https://s3.ap-northeast-1.wasabisys.com/je-pds2/cog/v1/JAXA.EORC_GSMaP_SPI.climate.gnrt6_monthly/collection.json) 和 [实际东半球 Item](https://s3.ap-northeast-1.wasabisys.com/je-pds2/cog/v1/JAXA.EORC_GSMaP_SPI.climate.gnrt6_monthly/2026-09/0/E000.00-E180.00/S90.00-N90.00.json)。

- 发布方标题为 `Monthly Averaged`，实际区间 `2026-09-01T00:00:00Z` 至 `2026-09-30T23:59:59Z`，不推断未说明的累积窗口或基准期。
- 810,000 个 Float32 像元全部独立解码：611,865 个 −999 填充值，198,135 个其他存储值；有效存储范围 **−3.949183464050293…3.7198359966278076**，实际负值保留。
- 同样缺少 TIFF NoData 标签，原生 scalar NoData 旗标按实际为 false。−999 不是极端干旱值；缺测识别需查看原 STAC 声明，不能从灰度图推断。

## 独立完整原件下载

| 产品 | 实际完整字节 | 研究下载耗时 | 独立原件 SHA-256 |
| --- | ---: | ---: | --- |
| 海面风速东北瓦片 | 500,889 | 0.798 秒 | `3a47ad4c3eb5a2ce3975f48ef7ccdd77d69ad2466c7de884fb771523bff3c675` |
| 微波海温东半球 | 549,401 | 0.752 秒 | `26b132eb93da4ca273fce47840dc607be7f21d1574d04a27a5702c8117444fea` |
| SPI 东半球 | 720,502 | 1.253 秒 | `e3301ff0416ebb612034332334330268cd51f7eaa48ef1aa650e101db8db71d9` |

三件均 HTTP 200 完整原件，未用小范围 Range 成功代替下载验收。每件独立读取全部 810,000 像元，并保留 6 个最小/最大/缺测/源错误/实际原值样本。不应用定标、NoData 替换、质量掩膜、重采样或重投影。源 Item 没有文件校验和，表内哈希是本地实际转存哈希，不能标为发布方哈希验证。

## 原生软件专用路径

共享注册编译后，已使用冻结 SHA 的私有 runtime 和三个全新独立 store，通过 `/datasets/connect` → 保存工程 → `/stac/downloads` 再次真实完整下载 → 原件检查、6 个原像元、PNG → 重启重开完成验收。研究 TIFF 没有注入 runtime store。冻结副本 SHA-256 为 `24763d45babbee638f4b7a175c9310e171be6b4082e50ccf90136255653ef014`；本批 JAXA 定向 23 个测试通过（其中本模块 3 项），共享 runtime 构建成功。

| 产品 | 原生首次完整下载轮询耗时 | 原生 job ID | 结果 |
| --- | ---: | --- | --- |
| 海面风速 | 1.510 秒 | `14a6285c-06d4-4d38-8238-4000410a64e2` | 500,889 字节，同独立 SHA；`fresh` / 0 续传字节 / 1 attempt |
| 微波海温 | 1.813 秒 | `a8cd7825-8166-4b60-b59a-c736af1c6b35` | 549,401 字节，同独立 SHA；`fresh` / 0 续传字节 / 1 attempt |
| SPI | 1.812 秒 | `ce8faa3d-6cce-4dc9-8400-b4d18c909a1c` | 720,502 字节，同独立 SHA；`fresh` / 0 续传字节 / 1 attempt |

三个完整原件的原生网格、dtype、原始 bounds 及各 **6 个存储数值 / 实际 TIFF NoData 旗标**与独立解码逐项一致。风速检查包括 −32768/true 和源错误 −32767/false；海温检查包括 −200、3506、−32768/false 和有效 0/false；SPI 检查包括原始 Float32 极值及 −999/false。原生不能通过改写旗标或科学掩膜伪装已补上 QA 缺口。

三件原生 PNG 均真实生成并目视检查，为 768 × 768，可辨东北/东半球岸线。默认原数值范围含错误/填充值，数据部分偏白、变化很浅，故这里只证明 PNG 和原件展示链路成立，不把它写成科学调色或最终界面美观验收。后续灰度 DN 对比度属于显示选择，不修改原件、源缺测、错误码或 NoData 旗标。

每个私有内核重启后，快照、保存工程、原件哈希、检查结果、PNG 和 6 个像元全部相同，均仍只有 **1 个 job**，未再下载。每项错误年、月、图层和命名地区的 4 个专用 API 请求均返回 400，没有替换其他产品或日期。SSW 匹配 Collection 仍为 false 并原样归档 2 个响应文档，SST/SPI 原始完整 Collection ID 匹配，均归档 2 个响应文档。三个私有 runtime 进程已停止。

完整验收根为 `.verification/ten-products-20261014/jaxa-ocean/native-run-d4872823b2cd4e9e87e1391155b41434/`，各 `{datasetId}-receipts/` 保存专用 API 连接、快照、原生原件检查、像元、PNG、错误请求、重启结果和验收报告。`native-acceptance-latest.json` 指向该实际运行。实际软件 Explore 选择、多选、在线地图、保存和打开工程由根任务另行检查，本报告不代替界面实测。

模块为 `crates/geod-runtime/src/datasets/jaxa_ocean_batch.rs`，12 个不改动的 Item 和 3 个 Collection 存入对应 JSON；固定年月/图层、原采集区间、源地址及全部科学解释绑定到不可变 preset。3 项单元测试覆盖选区不变、半开边界、错误产品日期/图层拒绝、源元数据改变拒绝、许可与 Collection 别名拒绝。中文产品说明、所有 warning/limits 的 37 个键在独立片段内，交由根任务合并。

## 排除的海冰候选

北极 AMSR2 `JAXA.JASMES_GCOM-W.AMSR2_ic0.v201_north_daily` 实际 2026-10-09 原件可完整下载和解码（139,792 字节，Int16，2048 × 2048，EPSG:3995，8192 米，NoData 8888，错误码 9999）。但发布方 Item 的 bbox/geometry 直接写投影米 `[-8388608,-8388608,8388608,8388608]`，不能当作 STAC 地理经纬度。当前固定发布方 Item 和地图路径无法原样直接支持，不创建合成 bbox，不扩投影规范，也不将其列为已接入；本批用真实独立科学量 SPI 替换。私有原始冲突及下载记录保留。
