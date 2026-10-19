# WSF / Global Mangrove Watch：公开原件调查与独立验证

2026-10-12。两类产品、三份完整原始 GeoTIFF 已独立下载并逐块解码。**本记录不等于软件原生渠道验收**；尚需接入后验证软件的下载、预览、像素读取及工程重新打开。全部结果、发布者 STAC、可用瓦片编号和分页清单哈希见 [JSON 记录](new-channels-20261012-wsf-gmw.json)。

## 来源与实际可用目录

- [DEAfrica WSF 说明](https://docs.digitalearthafrica.org/en/latest/data_specs/World_Settlement_Footprint_specs.html) 指明公开 `deafrica-input-datasets/wsf_{year}/` 目录。[DLR WSF2019 页面](https://download.geoservice.dlr.de/WSF2019/) 说明该产品为历史聚落二值分布，采用 CC BY 4.0。
- [DEAfrica GMW 说明](https://docs.digitalearthafrica.org/en/latest/data_specs/Global_Mangrove_Watch_specs.html) 指明相同公开桶的 `gmw/<year>/` 目录，提供非洲副本。[UNEP-WCMC GMW 元数据](https://data.unep-wcmc.org/pdfs/45/GMW_001_Metadata.pdf?1657017163=) 列出 v3.0 和 CC BY 4.0；[原发布记录](https://zenodo.org/records/6894273) 为 v3.0 数据来源。

实际逐页列举 `https://deafrica-input-datasets.s3.af-south-1.amazonaws.com/`，直到 `IsTruncated=false`：

| 产品 | 全部分页 | 实际原件 | 可接入年份 |
| --- | ---: | ---: | --- |
| WSF2015 v2 | 2 | 771 个瓦片 | 2015 |
| WSF2019 v1 | 2 | 771 个瓦片 | 2019 |
| GMW | 3 | 996 个 v3 瓦片、7 份旧版洲际文件 | v3 仅 2017–2020，各 249 个瓦片 |

WSF 两期瓦片编号相同，名义联合范围为经度 −26–64、纬度 −48–38，但联合外包框内并非处处有文件。GMW 只有已列出的 DEAfrica 区域沿海瓦片，包含部分非洲相邻区域。**不能标成全球下载镜像**。这份完整目录是发现证据；未逐个下载所有目录文件。

随后实际读取并审查了 **2,538 份支持瓦片的发布者 STAC**，用时 318.6 秒；逐项核对真实 ID、产品、年份、瓦片编号、年度日期、投影、网格大小、文件路径与 self 链接，并保存全部解析 JSON 树的固定结构摘要。紧凑的 [原生发布索引](../../crates/geod-runtime/src/datasets/settlements-mangrove-published-index.json) 为 694,876 字节。索引是验证材料，连接仍必须读取实际网络文档。所有元数据被审查不代表所有 TIFF 原件都已经完成下载验收。

## 完整原件验证

| 样本 | 完整下载 | 网格 | 全量解码像素 | 实际原值 |
| --- | ---: | --- | ---: | --- |
| WSF2015 v2 `36_-2` | 23,464,870 B / 27.688 s | 22,488 × 22,488 / EPSG:4326 | 505,710,144 | 0、255 |
| WSF2019 v1 `36_-2` | 14,197,753 B / 17.313 s | 22,488 × 22,488 / EPSG:4326 | 505,710,144 | 0、255 |
| GMW2020 v3 `S05E039` | 123,267 B / 2.281 s | 4,500 × 4,500 / EPSG:4326 | 20,250,000 | 0、1 |

总计 **37,785,890 B、1,031,670,288 个原网格像素**。三份文件大小均匹配实际 S3 清单，独立 MD5 匹配 S3 ETag；再保存独立 SHA-256。Rasterio 1.3.9 / GDAL 3.6.4 按 TIFF 块解码，未以缩略图、Range 片段或全零文件代替原件。

WSF2015 SHA-256：`5c0ecae92d1a9f68e33651679507e884c0c5c1f8ebc7ce3693cd49ab00f560d7`

WSF2019 SHA-256：`ab2afef0121f1fc3691a3b27ee3985451af8a1af97502d409f69ad4c8b4948a8`

GMW2020 SHA-256：`ceb39b5bca28c0f9120fe34e0de0fa527130148275b1d24a1310de8f7bf23514`

三份实际 STAC：

- [WSF2015 Nairobi](https://deafrica-input-datasets.s3.af-south-1.amazonaws.com/wsf_2015/36_-2/WSF2015_v2_36_-2.stac-item.json)
- [WSF2019 Nairobi](https://deafrica-input-datasets.s3.af-south-1.amazonaws.com/wsf_2019/36_-2/WSF2019_v1_36_-2.stac-item.json)
- [GMW2020 S05E039](https://deafrica-input-datasets.s3.af-south-1.amazonaws.com/gmw/2020/S05E039/GMW_S05E039_2020_v3.stac-item.json)

## 原值与网格必须保留的区别

**WSF 实际聚落值是 255，原 TIFF 没有嵌入 NoData 标签。** 两期内罗毕列 9240、行 14472 均读出 255；像素中心为 `36.82001211819833,-1.2900256637598388`。零背景同样可读，`noData=false`。ODC 文档中的填充值 0 是加载约定，不能用它改写原 TIFF 的 NoData，尤其不能把 255 错当成缺失值。两期原格网步长为 `0.00008983152841195215°`，发布者约 0.010° 的瓦片边缘扩展保持不变。

GMW 实际原 TIFF 嵌入 `NoData=0`，红树林正值为 1。列 483、行 92 读出 1，中心为 `39.10744444444434,-5.020555555555535`；该瓦片有 173,512 个正值。这里的缺失与非红树林背景共用原始编码，不能从零值推断有效地表覆盖。v3 样本步长是 `0.000222222222222°`（约 25 m），和旧版文档的 `0.0002°` 不同。

全量元数据审查还发现：WSF 原始瓦片的轴长分别为 22,487 或 22,488，不能把内罗毕样本网格套给所有文件；GMW 的 `N26E064` 是区域镜像东缘，四个年份均只有 4,500 行 × 2,250 列，东界为 `64.4999999999995°`。固定索引保存这些真实差异，不按文件名把半宽瓦片扩成整度。

GMW 1996–2016 洲际原件虽约 469 MB、压缩字节小于 512 MiB，网格为 **454,300 × 431,600**，超过现有原生轴长与网格上限。接入应选实际 v3 小瓦片，保留原有资源限制。

## 建议接入契约

按真实清单中的瓦片编号与年份生成固定发布条目地址。WSF 使用 `WSF2015_v2` / `WSF2019_v1`；GMW 只使用 `GMW_{tile}_{year}_v3`。WSF 为 2° 名义下左角瓦片；GMW 名称按 1° 左上角编码，例如 `S05E039` 对应 `[39,-6,40,-5]`。缺年、月份筛选、无可用瓦片的用户区域应明确拒绝，不替换成演示区域。

发布者 STAC 中的 `s3://deafrica-input-datasets/<key>` 必须原样存档；仅对经过审查的同一路径增加固定 HTTPS 下载地址。样本 STAC 没有 collection/license 链接，许可来自独立来源说明，不能伪造发布者声明。软件验收时使用 JSON 中的正值坐标、背景值、年度时间范围和每个文件的独立网格验证。
