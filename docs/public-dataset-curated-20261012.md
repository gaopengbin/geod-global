# 新渠道原生原件验收 · 2026-10-12

本批新增三个数据产品、六项代表性原件：INPE CBERS 的四种 L4 地表反射率集合、Impact Observatory 年度土地覆盖和 HILDA+ 历史土地利用。六项都完成真实完整下载、全文件独立解码、软件原生预览与像元核对、原件 SHA-256 和重新打开工程验证。最终通过时间为 `2026-10-12T05:04:54.835869+00:00`。这份报告只证明列出的原件与查询，不把单个样本推广为全部年份、图层或地区可用。

## 验收结果

| 代表原件 | 完整字节数 | 原始宽 × 高 | 类型 / CRS | 原生首次下载至落盘耗时 | 最终验收 |
| --- | ---: | --- | --- | ---: | --- |
| CBERS‑4A MUX L4 SR · BAND5 | 32,374,526 | 7,339 × 7,551 | int16 / EPSG:32723 | 73.078 s | 通过 |
| CBERS‑4 MUX L4 SR · BAND5 | 44,837,777 | 7,362 × 7,016 | int16 / EPSG:32723 | 41.016 s | 通过 |
| CBERS‑4A WFI L4 SR · BAND13 | 207,441,145 | 15,776 × 16,295 | int16 / EPSG:32723 | 523.297 s | 通过 |
| CBERS‑4 AWFI L4 SR · BAND13 | 224,039,192 | 16,253 × 13,912 | int16 / EPSG:32724 | 474.172 s | 通过 |
| IO 2024 · 29W supercell | 12,593,521 | 29,338 × 89,448 | uint8 / EPSG:32629 | 3.719 s | 通过 |
| HILDA+ 2019 · OpenLandMap v1.0 | 40,383,673 | 43,200 × 17,923 | uint8 / EPSG:4326 | 34.515 s | 通过 |

最终六份验收原件合计 **561,669,834 B**；独立 Rasterio/GDAL 对六个原始波段完整解码 **3,988,749,261 像元**，不是抽样或把 overview 计作原图；软件原生核对 **38 个位置 / 38 个原始波段值**。六份原件、原始来源快照、工程与 job 均在服务重启后重新读取并核验。独立解码环境：Rasterio 1.3.9 / GDAL 3.6.4。

表中耗时是原生 job 首次排队至 settled 的实际观测值，包含正常网络传输与落盘；不是 Python 外部研究小样耗时，也不是后续缓存重检耗时。WFI 原件约 207 / 224 MB，本次分别约 8.7 / 7.9 分钟，接入完成不能代表网络速度已加快。

最终冻结运行文件 SHA-256：`df0c681841690af165bc8956058b1bca00d349d10b2374e5fbb932193b45ec63`。初始下载与部分检验保留为私有 `verification-before-*.json`（初始 hash `dcba5a38672db7b3c07ed486f4c09b1aa6db49fabae987cefeb474c806de1bb2`，下一次部分检查 hash `ddf29f0034f9ebb8f2cbbf1925038d649ab8cde533746ce6342a8af1452bf4c4`）。五份原件在最终二进制上复用缓存重检。HILDA 最初元数据错误地用概念枚举 1–6 解释原始编码，旧 pin 未获得兼容放行；新二进制在启动校验时拒绝旧 proof。仅在自建私有验收 store 完整备份旧 registry/project/job 三份索引后，隔离旧 HILDA 的三个活动指针，保留原件、不可变文档/快照和完整旧记录；没有操作用户 store，也不是软件自动修复或用户数据迁移。服务启动后，实际读取旧 snapshot 返回 reviewed preset provenance 不一致的 HTTP 错误，未伪称测试过旧 job 的 inspect HTTP。随后重新连接正确声明、保存新 pin 并完整下载 40,383,673 B。这次额外传输与旧文件 SHA 相同，未修改原始数据。当前表仅统计六份最终验收原件一次，不把额外传输重复计入。

## 来源与完整性

CBERS 四项采用 [INPE 官方 STAC](https://data.inpe.br/bdc/stac-spatiotemporal-asset-catalog/) 实际 item、collection、搜索请求和原始 asset JSON；保存原始文档哈希并逐字段对照，没有为原始 properties 添加合成 datasetPreset。完整文件同时匹配发布方 `checksum:multihash` 的 SHA-256 和 `bdc:size`。BAND5 / BAND13 是数值科学波段，不是 PNG 快视图或 CMASK。官方 STAC 声明反射率 scale 0.0001，而实际 TIFF scale 为 1；原生像元保留 raw Int16，未自动乘比例、施加质量掩膜或推断反射率正确性。

IO 使用 [正式 AWS Open Data 发布页](https://registry.opendata.aws/io-lulc/) 对应 `https://api.impactobservatory.com/stac-aws/` 的年度分类 supercell 原件。AWS registry 与 collection description 声明 CC BY 4.0，原始 collection 的 license 字段却为 proprietary；客户端保留两者及冲突提示，没有更改原始 STAC。此次 29W 原件实际含大量嵌入 NoData=0，仅约 0.586% 非缺失，不是整块范围都具备有效分类。

HILDA 使用 [PANGAEA v1 来源](https://doi.pangaea.de/10.1594/PANGAEA.921846) 与 OpenLandMap 固定文件 recipe；明确标记为 reviewed-client-preset，而不是合成的官方 STAC。文件 EPSG:4326、实际 1/120° 网格，纬度范围约 −61.988333° 至 87.37°，不是完整南北极覆盖。OpenLandMap 原始 STAC 经纬度范围有矛盾，未伪装为修复后的发布方 metadata。

[作者 KIT 页面](https://landchange.imk-ifu.kit.edu/hilda) 与完整原件值频率支持 0 海洋、11 城市、22 耕地、33 牧场/放牧地、44 森林、55 非管理草地/灌木地、66 稀疏或无植被、77 水域、99 声明缺失。本份历史原件未观测到新版本额外编码。官方 OpenLandMap SLD/QML 把 77 标为 sparse、漏掉 66/99，另存为冲突证据，不作为自动显示/处理规则。原 TIFF 没有 NoData tag，原生 `noData` 保持 None；新增实际原生点读 77@(8941,966) 和 99@(22954,901) 都返回原始值、noData=false。99 的 390,238 个像元是作者声明辅助统计，未修改原图或原生掩膜。Raw decode 的 100% unmasked 不表示科学意义上的 100% 有效覆盖。详见 [类别证据](./research/hilda-legend-verification-20261012.json)。

## 检索、分页与 AOI 边界

最终二进制在另一个全新 store 对六项再次 resolve/connect，保存原始真实响应。四种 CBERS 小范围查询分别返回 2 / 1 / 4 / 9 景，IO 返回 1 个年度 supercell；这些限定查询的 page 为 complete。连接阶段未创建下载任务。另用巴西广域、limit=2 的 CBERS 查询连续读取两页，每页 2 景，互不重复；第二页仍有 nextCursor，因此不能写成全域检索完整。两页共享 canonical collection / bounds / datetime / limit，原始请求与 document hash 留证，下载任务数始终为 0。

完整 page、影像 bbox 与 AOI 相交、某一个有效像元，都不能替代 AOI 几何覆盖、云质量或科学有效性验证。两个 MUX 样本只与请求区薄边相交，本次在 AOI 窗口内未找到有效原始像元，保留如下限制：

| 原件 | 有效原始位置 column,row | 原始值 | 样本范围 |
| --- | --- | ---: | --- |
| CBERS‑4A MUX L4 SR · BAND5 | (1362, 174) | 686 | 原件其他位置；本次未确认 AOI 中有有效样本 |
| CBERS‑4 MUX L4 SR · BAND5 | (1331, 180) | 305 | 原件其他位置；本次未确认 AOI 中有有效样本 |
| CBERS‑4A WFI L4 SR · BAND13 | (11458, 9460) | 5773 | AOI 窗口中的一个有效原始像元；不等于范围覆盖验证 |
| CBERS‑4 AWFI L4 SR · BAND13 | (5959, 728) | 773 | AOI 窗口中的一个有效原始像元；不等于范围覆盖验证 |
| IO 2024 · 29W supercell | (15578, 11630) | 1 | AOI 窗口中的一个有效原始像元；不等于范围覆盖验证 |
| HILDA+ 2019 · OpenLandMap v1.0 | (22398, 10448) | 55 | AOI 窗口中的一个有效原始像元；不等于范围覆盖验证 |

## 预览兼容修复与范围

实际 WFI 原件的嵌入 overview 使用整数缩小后的 floor 尺寸，早期读取器只识别 ceil，跳过 overview 后超出累计预览预算。最终读取器在同一整数因子、相同类型/波段/NoData 与 reduction 标记条件下兼容 floor/ceil。4A‑WFI 16295×15776 的 overview 到 254×246；4‑AWFI 13912×16253 的 overview 到 217×253。最终预览成功，原始像元仍读取原图。

IO 的主 IFD 为 PlanarConfiguration=2、但仅一个波段，overview 为 PlanarConfiguration=1；最终支持这个等价单波段情况，未开放未实现的多波段 separate-plane 解码。此次 29W 高度 89,448 像元通过；超出 131,072 轴限制的极区对象仍被拒绝。Tiled 原图仍受每轴 131,072 / 总网格 8 billion / chunk count 131,072 约束，stripped 保持每轴 65,536，128 MiB 单 chunk、32 MiB 中间缓冲、512 MiB 累计预览、30 秒读取期限保持。

六份 PNG 在文件层视觉检查通过，显示实际单波段灰度原图/overview；IO 小岛仅占 supercell 的小部分，HILDA 显示数值类别分布。没有把灰度显示称为主题着色、模型精度验证或地表变化分析。本报告未做桌面原生窗口点击验收；[浏览器预览的真实操作与检查](../prototype/qa/public-dataset-curated-ui-verification.json)另有记录：CBERS 和 IO 两个完整下载流程、真实 20→40 条分页、年月切换、原始像元读取及 768 像素布局均通过。未自动裁剪 AOI、重投影、反射率定标、质量掩膜、生成源产品 ZIP、调用模型、读取凭据或运行 Earth Engine 计算。

## 原件 SHA-256

| 原件 | 完整文件 SHA-256 |
| --- | --- |
| CBERS‑4A MUX L4 SR · BAND5 | `d24255698a18e8d751e91e9f7f02423ce685fde85785e2f34504911e6693bc59` |
| CBERS‑4 MUX L4 SR · BAND5 | `a1d870d1493e01f60a746593160f16f8444149877d64405033a64d80a08fb476` |
| CBERS‑4A WFI L4 SR · BAND13 | `941f64f55f7b294f7c541b3465e4093d040aec09507960bf096f7d1d3d130cb7` |
| CBERS‑4 AWFI L4 SR · BAND13 | `db255c6b4333e1cb281810141592a2955f9e56731659c39996fdb7d1d61dfa90` |
| IO 2024 · 29W supercell | `ed2f9bebdd374a1f725821db3981c4b8b4e0ec07a2200bbfc0cfba588d297ac9` |
| HILDA+ 2019 · OpenLandMap v1.0 | `0c375f36c6de18012dd9c05df0bc99d85972e1c96662d7ed95489c079507181d` |

## 回执

- [六项完整原生验收 JSON](./public-dataset-curated-20261012.json)
- [最终二进制全新 metadata/connect 回执](./public-dataset-curated-metadata-20261012.json)
- [真实分页回执](./public-dataset-curated-pagination-20261012.json)
- [可复用验收脚本](../scripts/verify-curated-stac-connectors.py) 与 [固定六项选择](../scripts/public-dataset-curated-cases.json)
- 私有原件、不可变原始 STAC 文档、PNG 与旧 receipts：`.verification/public-datasets-curated-native-20261012`；私有完整分页响应另在最终 metadata run。

两个独立验收服务已停止；用户服务、桌面进程、账号和配额未由此脚本操作。前两批六渠道与五渠道的历史验收报告未覆盖。下一候选 [WorldCereal](./research/worldcereal-candidate-20261012.md) 仍为研究阶段，不能算作已完成原生接入。
