# WorldCereal 实际正分类原件研究 · 2026-10-12

本报告是本轮新下载的独立文件验证，原生软件验收另有报告。公开目录中有 135 份 classification、97 份 confidence；已审阅的固定发行索引限定 AEZ 9026、参考年 2021 的七种主题，不代表所有地区已经接入。原始 JSON、原始 S3 href 和实际影像保留。

## 完整原件

|原件|字节|原件 SHA-256|完整解码观察|
|---|---:|---|---|
|年度 temporarycrops classification|6,310,865|`a4c3735e77d6784e1f85312a8b3e7ccdf9f650fd1ee541708ca9266969ca65dd`|73,567×44,563，0 和 100；100 有 2,264,771 像元|
|main-maize-season activecropland classification|6,424,508|`a91e50019c1f6db0fdfe3d038fd6c5270314a953df811707704a1128f6cf136e`|73,566×44,563，0、100、254；100 有 1,358,513 像元|
|年度 temporarycrops confidence|2,038,416|`418ac67a78a94c33824519b2cb9c55976b8bb62f6f47f5eb69309cfea9d162df`|15,326×9,284，原图例与实际值差异见下文|

三份合计 14,773,789 字节，全部独立解码；EPSG:4326、UInt8、tiled Deflate，嵌入 NoData 为 255。原件和独立解码回执在私有 `.verification/worldcereal-positive-research-20261012`，公开详细回执见 [JSON](./worldcereal-positive-originals-20261012.json)，七主题索引见 [发行索引](./worldcereal-deafrica-9026-index-20261012.json)。

年度分类完整下载后首次读文件脚本调用出错，修复后复用该完整原件；首次下载用时未保留，后续 HEAD 只用于元数据，不能计为完整下载。另一分类、confidence 的本轮独立完整传输分别为 4.313 秒、5.281 秒。这些用时不是软件原生任务的下载用时。

## 保留科学含义和发布差异

年度图例为 0=other land cover/use、100=temporary crops；季节 activecropland 为 0=not active、100=active、254=No crop。254 是有效类别，255 才是这两份原件的嵌入 NoData。作物编码的存在并不验证分类精度、整个 AOI 的作物覆盖，或其他季节的正确性。

Confidence 的 TIFF 图例写“0 到 100，254=no crop”，实际完整文件还有 48,300 个 101–253 原值像元。没有截断、重分类或概率校准；254 的 native NoData 不能标为 true。该资产使用独立 0.0004° 网格，不能继承分类图约 0.000083333° 的格网。其余 confidence 本轮仅元数据可见，未宣称全部原件已验收。

年度分类真实 south=-22.695376495001614，publisher item bbox south=-22.69539316166828，差 0.2 个分类网格像元。实际 transform 与 asset 声明一致；保存原 bbox，文件检查使用真实 TIFF 地理信息。

[WorldCereal 官方定义](https://esa-worldcereal.org/en/products/global-maps) 区分年度临时耕地、作物类型和季节活动：activecropland 不限于对应季节名称中的作物，季节时间由 AEZ 确定，不能把参考年理解为自然年。[DE Africa 发行说明](https://docs.digitalearthafrica.org/en/latest/data_specs/ESA_World_Cereal_specs.html) 声明 CC BY 4.0 和公开 Cape Town S3 分发；独立 TIFF license 标签也为 CC BY 4.0。真实 standalone item 没有 collection link，不构造不存在的集合许可文件。

## 浏览器正像元探针

年度 temporarycrops 的小 AOI：`[57.61593284149066, -20.059293161668283, 57.623932841490664, -20.051293161668283]`。

原件 column=45065、row=12882，像元中心 `(57.619974508157334, -20.05533482833495)`，原值 100，嵌入 NoData=false；附近 32×32 块的 1,024 个原像元都为 100。这只是代表性原图验证位置，不是产品新生成的处理成果，也不作为区域科学精度或全面覆盖证明。
