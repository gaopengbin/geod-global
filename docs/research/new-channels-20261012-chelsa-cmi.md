# CHELSA CMI 常年气候：完整原件与有效 NoData 验证

2026-10-12。BIO1 气温原件的 UInt16 / −99999 声明不合法，保留原生严格拒绝规则。此次改验的是作者**另一种真实产品变量**：CMI mean，固定 1981–2010 气候常年期；它没有被改名或假冒为 BIO1 气温。[完整证据与原始标签](new-channels-20261012-chelsa-cmi.json)。

**独立完整原件验收通过；生产模块已准备，仍须软件原生生命周期验收后才能标 ready。**

## 固定公开原件

[CHELSA_cmi_mean_1981-2010_V.2.1.tif](https://os.zhdk.cloud.switch.ch/chelsav2/GLOBAL/climatologies/1981-2010/bio/CHELSA_cmi_mean_1981-2010_V.2.1.tif)，WSL 的 SWITCH 对象存储，匿名 HTTPS，直接 GeoTIFF，无 ZIP 解包或像素重写。

| 检查 | 实际结果 |
| --- | --- |
| HTTP / 类型 | 200 / image/tiff |
| 完整大小 | 244,959,851 B，匹配 Content-Length |
| 完整传输 | 288.453 秒 |
| SHA-256 | `5aa9f5f1aa14f8671807a73181c13aa55217ab1a653081111e691fa89c6cd283` |
| 原网格 | 43,200 × 20,880，单波段 Int16，EPSG:4326 |
| TIFF | DEFLATE，strip 1 × 43,200，PixelIsArea |
| NoData 原标签 | 32767，合法可表达的 Int16 sentinel |
| 原始 scale / offset | 0.1 / 0 |
| 完整解码 | 902,016,000 像素，7.234 秒 |
| 按原声明有效 / 缺失像素 | 224,368,589 / 677,647,411 |
| 原始有效值范围 | −3349–17393 |

原件独立 MD5 匹配对象的 `x-amz-meta-md5chksum`。这里的 ETag 是 multipart ETag，**没有把它误当单文件 MD5**。实际大小、axis、网格及 chunk 都在现有资源限制内，没有提高限额。

独立解码使用 Rasterio 1.3.9 / GDAL 3.6.4，对全部原 strip 计数。此前 Range 只用于候选标签排查，全部验收统计来自这个完整原件。

## 原始像素及科学含义

| 地点 | 列 / 行 | 原值 | 原 NoData | 按嵌入 scale 换算，仅供说明 |
| --- | --- | ---: | --- | ---: |
| 北京 | 35568 / 5291 | −323 | false | −32.3 kg m⁻² month⁻¹ |
| 苏黎世 | 22624 / 4395 | 260 | false | 26.0 kg m⁻² month⁻¹ |
| 纽约 | 12720 / 5195 | 69 | false | 6.9 kg m⁻² month⁻¹ |
| 0° / 0° 海洋位置 | 21600 / 10079 | 32767 | true | 不转换缺失值 |

[作者数据论文第 2.2.4 节](https://essd.copernicus.org/articles/14/5573/2022/#section2_2_4) 定义 CMI 为**降水减潜在蒸散**，单位 kg m⁻² month⁻¹；这个常年均值不表示最新天气、某单年观测、降水总量、气温或实际土壤含水量。下载及原值检查保留原 Int16，换算说明为 `raw × 0.1`。负值不能因不符合气温或降水直觉而截成零。

[作者当前 climatologies 页面](https://www.chelsa-climate.org/datasets/chelsa_climatologies) 将 CMI 简介写成 ratio，而单位和原作者论文采用上述差值定义。这一说明冲突已记录；不据此把原数据解释成无量纲比例。该作者页当前声明 CC0，完整页面已保存 SHA-256 回执；不是给原 TIFF 添加虚构许可字段。

## 格网与接入边界

实际 GeoKeys 为 Geographic / PixelIsArea / EPSG:4326，Citation `WGS 84`；ModelPixelScale 与 ModelTiepoint 保留原数值，没有 ModelTransformation。

原始外边界为 `[-180.00013888885002,-90.00013888884999,179.99985967115003,83.99986041515001]`；这是作者原格网的细小边缘扩展。所有原像元中心仍在物理合法经纬度内。客户端查询/显示范围可以与合法世界取交集 `[-180,-90,179.99985967115003,83.99986041515001]`，但不能把这个交集冒充原 TIFF 的 extent：原文件、变换、ImageStatic 定位及点击像元映射不重写。

独立产品 ID `chelsa-climate`，唯一图层 `cmi-mean`，不提供伪造的 year/month/country/region/season 筛选；源中显式保存 1981-01-01 至 2010-12-31 常年期。完全位于 85°N 的请求拒绝。范围相交不等于有效像元或科学准确性保证。

BIO1、BIO12、monthly TAS/PR 的负 UInt16 sentinel 继续拒绝；其中 BIO12 完整原件大小还超过 512 MiB。SWB 与 monthly CMI 目前只有元数据排查，不因 CMI mean 通过而标为可用。
