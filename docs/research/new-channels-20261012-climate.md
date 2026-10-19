# 跨平台气候、夜光、海底地形渠道：原件验证

2026-10-12。此次完整下载了 **CHELSA V2.1 BIO1**，并用 GDAL 全量解码，随后直接核对 TIFF 标签发现无效的 UInt16 NoData 声明，**当前严格原生解析拒绝该文件，不接入 BIO1、不放松通用规则**。该文件来自 WSL 的 SWITCH 对象存储，而非 DEAfrica 镜像。另在调查拥有合法 Int16 NoData 的 CMI 原件。WorldClim、EOG 与 GEBCO 的具体访问/许可边界另行记录。[完整证据 JSON](new-channels-20261012-climate.json) 包含原件哈希、HTTP 回执、完整网格解码、原始标签与主来源材料哈希。

**这里仍是独立原件验证；尚未通过软件原生渠道验收，不能标 ready。**

后续找到拥有合法 Int16 / 32767 NoData 的 CMI mean 原件，并完成完整下载与全量独立解码：[CMI 专项记录](new-channels-20261012-chelsa-cmi.md)。BIO1 的拒绝结论保持，CMI 原生接入另行验收。

## CHELSA BIO1：完整原件但声明不符合当前原生规则

[作者 BIOCLIM 产品说明](https://www.chelsa-climate.org/datasets/chelsa_bioclim) 将该产品列为 V2.1，当前许可是 CC0；BIO1 为年平均近地面气温。采用固定 **1981–2010 气候常年期**，不是单个年份、当前天气或 2026 年实测温度。作者论文说明这类整数 GeoTIFF 采用嵌入的 scale/offset，且全球格网有半角秒边界偏移。[作者输出格式说明](https://essd.copernicus.org/articles/14/5573/2022/#section2_4)

实际匿名读取完整原件：[CHELSA_bio1_1981-2010_V.2.1.tif](https://os.zhdk.cloud.switch.ch/chelsav2/GLOBAL/climatologies/1981-2010/bio/CHELSA_bio1_1981-2010_V.2.1.tif)。

| 检查 | 结果 |
| --- | --- |
| HTTP / 类型 | 200 / image/tiff |
| 完整字节 | 115,490,323 B，匹配 Content-Length |
| 本次传输 | 127.593 秒 |
| MD5 / S3 ETag | 两者实际相同：`799ffe2e872a89a949f045268d721b62` |
| SHA-256 | `f584314e12be7cd63d0e2292f4655173030e3e547ce36a2a97ecbdabc4f87a53` |
| 原网格 | 43,200 × 20,880，单波段 UInt16，EPSG:4326 |
| 存储块 | DEFLATE，strip 1 × 43,200 |
| 全量独立解码 | 902,016,000 像素，28.782 秒 |
| 原数值范围 | 2196–3079 |
| 嵌入 scale / offset | 0.1 / −273.15 |
| 物理 TIFF NoData 声明 | `GDAL_NODATA=-99999`，超出 UInt16 可表达域 |
| GDAL 有效 NoData 视图 | None，读取器没有施加有效 typed mask；这不是没有原始标签 |

独立读取使用 Rasterio 1.3.9 / GDAL 3.6.4，对每个原始 strip 解码与计数，没有把 Range 片段、缩略图、裁剪样例或科研教程预处理产物当成完整原件。

原始边界为 `[-180.00013888885002,-90.00013888884999,179.99985967115003,83.99986041515001]`，步长 `0.0083333333°`。不能四舍五入成整度边界后声称仍是原网格。该文件压缩字节、strip 轴长与网格总量均在现有原生 512 MiB、65,536 strip 轴长及 80 亿像素限制以内；没有提高资源上限。

直接检查 TIFF IFD/GeoKeys 得到 `GTRasterTypeGeoKey=1 (PixelIsArea)`、`GTModelTypeGeoKey=2 (Geographic)`、EPSG:4326、Citation `WGS 84`、角度单位 9102；采用 ModelPixelScale/ModelTiepoint，没有 ModelTransformation。物理 `GDAL_NODATA` 文本确实存在。Rasterio 返回 `nodata=None` 是因为无法把 −99999 表示成 UInt16，并非没有该标签。原生解析的严格整数声明检查应继续拒绝这种错误，不把它改成零或其他掩膜。

原始像素检查：

| 地点 | 列 / 行 | 原值 | NoData | 按嵌入声明换算，仅供说明 |
| --- | --- | ---: | --- | ---: |
| 北京 | 35568 / 5291 | 2861 | false | 12.95 °C |
| 苏黎世 | 22624 / 4395 | 2835 | false | 10.35 °C |
| 纽约 | 12720 / 5195 | 2855 | false | 12.35 °C |
| 0° / 0° 海洋位置 | 21600 / 10079 | 2988 | false | 25.65 °C |

下载与原值检查保持 UInt16 整数。物理说明为 `raw × 0.1 − 273.15`；不能把原值 2861 标成 2861 °C。表中 `NoData=false` 仅是 GDAL 没有施加有效 typed mask 的读取结果；它不能抹去原文件的无效 −99999 声明，也不能证明海洋位置的气候数值科学有效。纬度 85° 超出实际原栅格。

BIO1 保持研究候选，不写入生产连接器。后续改查具有合法 Int16 声明的 CMI mean 或其他作者原件；未经完整下载、解码、软件原生验收不能标可用。其原格网边界若仅有半像元扩展，可以讨论通用显示范围交合法物理世界，原 ImageStatic extent、变换和像素仍保留；这与修复错误 NoData 是两件不同的事。

## 其他具体路径的边界

| 渠道 | 本次实际证据 | 当前处理 |
| --- | --- | --- |
| WorldClim V2.1 | 官方目录为 ZIP；测试裸 `wc2.1_10m_tavg_01.tif` 返回 404，对应 ZIP 返回 200、37,364,656 B | 未下载原件；先处理许可与受限归档提取 |
| EOG VIIRS annual v22 / 2023 | 匿名 HEAD 返回 401；产品页及许可文件可读 | 尚无匿名完整原件验收，不能把免费许可当成匿名接口可用 |
| GEBCO 2026 | 当前官网给 8 个 90° × 90° GeoTIFF 组成的 ZIP，地形包约 4 GB；TID 包约 91.7 MB | 未完整下载；需核每个原 TIFF 大小与 ZIP/裸文件路径，不能以按区域服务输出替代发布原件 |

[WorldClim 当前许可](https://www.worldclim.org/about.html) 允许学术及非商业使用，未经许可不允许商业使用和再分发；[官方文件目录](https://geodata.ucdavis.edu/climate/worldclim/2_1/base/) 是格式证据。不能因第三方镜像自行改许可标签就把该数据加入无限制商用基础源。

[EOG 官方产品说明](https://eogdata.mines.edu/products/vnl/) 与 [具体许可说明](https://eogdata.mines.edu/files/EOG_products_CC_License.pdf) 需要按产品核对。当前仅证实一个年度目录的匿名访问失败，没有声称 EOG 所有产品均不可用。

[GEBCO 当前下载页](https://www.gebco.net/data-products/gridded-bathymetry-data) 和 [通用网格条件](https://www.gebco.net/data-products-gridded-bathymetry-data/gebco2025-grid) 说明可商用并须注明来源。TID 是数据来源类型编码，不是海底深度；也不能把影像底图或渲染图当数值地形原件。此次尚未取得满足原生字节限制的完整地形原文件。
