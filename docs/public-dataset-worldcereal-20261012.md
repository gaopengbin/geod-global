# ESA WorldCereal 原生完整原件验收 · 2026-10-12

**通过：七份 classification 和一份独立年度 confidence，共 45,586,101 字节。** 七分类均有真实的 100 正类别；完成原始文件下载、SHA-256、独立全格网解码、原生预览、57 个原生像元值与嵌入 NoData 核对，以及八个工程、来源快照、任务的重新打开。没有使用旧 AEZ9029 空样本作证明，也没有使用模拟下载、地图渲染图片或之前批次的文件。

## 冻结版本和证据

- 独立二进制 SHA-256：`32a0af3544b9ad7451ba267613f2bbb6990a6292d42a821f10252e9e449ef04d`。
- 原生入口：`/datasets/resolve` → `/datasets/connect` → `/stac/project` → `/stac/downloads` → `/stac/jobs/<id>/inspect` 与 `/pixel` → 服务重开。固定发行索引限定 **AEZ 9026 / Mauritius、Réunion，参考年 2021**，七主题是一个产品家族，不是七个卫星平台，也不代表全球原件都已可用。
- 实际 native 回执：[JSON](./public-dataset-worldcereal-20261012.json)；独立逐 asset 格网核对：[格网回执](./public-dataset-worldcereal-asset-grids-20261012.json)；拒绝不支持参数：[边界回执](./public-dataset-worldcereal-boundaries-20261012.json)。
- 八张已保存原生 PNG 的唯一文件名、关联任务/原件、尺寸与 SHA-256：[预览清单](./public-dataset-worldcereal-previews-20261012.json)。本轮 classification 与 confidence 预览已使用不同名字，没有覆盖；此次清单仅检查本地已保存 PNG，不重新调用服务、下载或解码整幅栅格，原执行 JSON 未改写。后续 harness 文件名使用 case 序号、主题和真实 asset key。
- 私有原件、完整 publisher JSON、独立 reviewed receipt、预览和日志：`.verification/public-datasets-worldcereal-native-20261012`。该独立验收仅使用自有端口 4665，任务结束已停止，没有改写此前批次的报告。其后软件浏览器验收使用同一冻结二进制的 4318 预览，另行产生两份实际年度原件，详见下方独立回执。
- 本轮重新连接真实 standalone STAC item，原 properties、geometry、bbox、asset metadata 和原 S3 href 都与存档 JSON 一致；原文及 separate snapshot receipt hash 分别校验。`reviewedDataset` 是本地固定发行绑定；`assetTransports` 只把精确受控公开地址映射到 HTTPS，并不改写 publisher JSON。原件使用实际 HTTPS 传输，不读取 AWS 凭据。
- reviewed 索引 SHA-256：`e9ca8ef16918b72c30cdfa41fe48f27c9aac22bc7b57303665ec77d7947435b6`。真实 item 没有 collection link，collection 声明未取得；CC BY 4.0 是独立发行文档和原 TIFF 标签许可，未构造不存在的 collection 许可证明。

## 软件内实际操作

[浏览器与原生回执](../prototype/qa/public-dataset-worldcereal-ui-verification.json)记录原有探索下拉入口、实际条目与文件声明、显式选择/保存/下载、我的数据成果与刷新恢复。下拉仍有 30 个选项：15 个影像提供方入口（含 6 个仅目录）和 15 个公开产品。切换主题会匹配真实季节并清除旧条目，保留输入区域；中国区域会明确拒绝，不替换成演示地区。

浏览器工程 `b9f382cd-0c38-47f2-8ff1-56a7fc825395` 下载年度 classification 6,310,865 字节和 confidence 2,038,416 字节；两任务均为 fresh、各一次尝试。刷新后原件和本地预览仍在，原始栅格窗口显示各自尺寸与 EPSG:4326：分类 `(45065,12882)=100`，置信度 `(9388,2683)=54`，与独立 Rasterio 和原生像元 API 相符。元数据读取、保存和最终检查不重复启动下载；原有五任务加此两任务共七任务，均已结束。

置信度 101–253 超出 TIFF 图例 0–100 的说明紧邻文件选择项；软件保留原值及各自网格。768×800 窄屏检查无水平溢出，原值输入和操作按钮可用；临时尺寸已恢复。五张实际浏览器截图以独立哈希记录在回执中。本轮前端 367、Agent 149、界面 448、原生 664、桌面 56 个测试通过；格式、Clippy 和生产前端构建通过，选择性外部/桌面测试仍按原规则忽略，不作为实测证明。

## 完整传输和解码

|主题 / 资产|原始字节|原格网宽×高|native 下载秒|原值100数量|
|---|---:|---|---:|---:|
|activecropland / classification|6,498,381|73,566×44,563|11.860|1,413,007|
|maize-active / classification|6,424,508|73,566×44,563|9.703|1,358,513|
|maize-irrigation / classification|5,745,232|73,566×44,563|10.594|7,828|
|maize-main / classification|6,400,120|73,566×44,563|11.078|629,397|
|temporarycrops / classification|6,310,865|73,567×44,563|9.953|2,264,771|
|wintercereals / classification|6,391,243|73,566×44,563|6.984|126,652|
|wintercereals-irrigation / classification|5,777,336|73,566×44,563|18.453|49,674|
|temporarycrops / confidence|2,038,416|15,326×9,284|6.985|319|

全部是 EPSG:4326、UInt8 的 tiled 原件。每份都保持轴≤131,072、总格网≤8B像元、文件≤512MiB；保留 native chunk、preview、review 时间预算。独立 GDAL/Rasterio 逐块解码共 **23,090,582,753 原格网像元**，包括 NoData，没有把辅助统计或处理输出重复计入。该全解码是验收工具的独立工作，软件只在预算内生成原值预览、读取选中像元。

表内是此次 fresh native 任务从排队至成功 settled 的观察用时，不是 Python 外部研究样本传输，也不是性能保证。所有原件为本轮 native 新下载，没有缓存转移复验。

## 科学含义和实际限制

嵌入 NoData 为 255。季节类别 254 为有效 no crop，native 像元接口也维持 NoData=false；年度分类 0=other land cover/use、100=temporary crops。活跃耕地和灌溉季节是对应 AEZ 的季节窗口，不自动等同于 maize-only 或 winter-cereals-only。原生预览是数值缩略图，不是科学精度、QA 处理、作物产量或整个 AOI 的有效覆盖证明。

年度 confidence 原件独立格网为 15,326×9,284、0.0004°；分类原件约 0.000083333°。八份实际格网均对各自 asset 的 proj 字段独立核验，confidence 没有继承 classification 的 shape、transform 或分辨率。TIFF confidence legend 宣称 0–100 与 254=no crop；本文件实际有 **48,300 个 101–253 原值像元**，已写入软件来源警示，保留原值，不截断、转换成校准概率或推断产生原因。其余四份 confidence 仅元数据可见，尚未逐份完整文件验收。

年度分类 item bbox south 与 TIFF 实际 south 有 0.2 分类网格的差异，保留原文；实际 source 和 file 范围分别记录。小 AOI `[57.61593284149066, -20.059293161668283, 57.623932841490664, -20.051293161668283]` 内，column=45065、row=12882 原件值100、native NoData=false，中心 `(57.619974508157334, -20.05533482833495)`；独立原件附近32×32块都是100。该点可用于真实界面检查，不等同整个 AOI 或地图精度验证。

纯 resolve 对中国 AOI、未审阅 AEZ9029、2020、错误季节及 month 参数都实际返回400，未增加任务。八次 connect 完成元数据也未启动下载，下载由后续明确项目操作开始。这是有限 reviewed index 的元数据完成，不是全洲 exhaustive discovery。

[DE Africa 官方发行说明](https://docs.digitalearthafrica.org/en/latest/data_specs/ESA_World_Cereal_specs.html) 提供七主题、公开区域 S3 和 CC BY 4.0；[WorldCereal 官方产品定义](https://esa-worldcereal.org/en/products/global-maps) 解释 AEZ 季节与分类范围。实际日期以每份原 item/TIFF 标签为准，2021 参考年并不意味着自然年。

软件本轮支持原件保存、预览和原值检查，未声称增加任意裁剪、分类面积统计、校准、时序分析、RGB合成或源 ZIP打包。native 窗口与浏览器视觉验收由父任务另做，此报告不把 API 测试说成界面验收。

## 原件哈希

|主题 / 资产|SHA-256|
|---|---|
|activecropland / classification|`ac1dba900b0cf064d549b3fabcf83bcd7ab22794f579a68e5d55dcc773680885`|
|maize-active / classification|`a91e50019c1f6db0fdfe3d038fd6c5270314a953df811707704a1128f6cf136e`|
|maize-irrigation / classification|`fb116af6e1de40c76d7419cbedf18f80d1ec6ddaf7d26be050f24d41cb83388b`|
|maize-main / classification|`4750f787da7013d604aacd500814f9ec4f91c021b3b0318dae7db244e4917c8e`|
|temporarycrops / classification|`a4c3735e77d6784e1f85312a8b3e7ccdf9f650fd1ee541708ca9266969ca65dd`|
|wintercereals / classification|`212c9ccb82dcf3963afc111ed322ee3b4facf9bc3fab252d2368d3fc39593738`|
|wintercereals-irrigation / classification|`3bba7e02561aed775dba1cfdc306ca0b625d5b9c90ea73b12ea7e188e694a3b5`|
|temporarycrops / confidence|`418ac67a78a94c33824519b2cb9c55976b8bb62f6f47f5eb69309cfea9d162df`|
