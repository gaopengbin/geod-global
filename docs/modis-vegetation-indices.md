# MODIS NDVI / EVI 接入与实际文件验收

2026-10-04。桌面开发版新增 **MODIS NDVI / EVI · Planetary Computer**，接入 Terra MOD13Q1 与 Aqua MYD13Q1 v061 的两个植被指数。实时目录、完整 COG 下载、工程恢复、本地地图和原值检查、持久化缩略图、同网格拼接与裁剪已用实际公开文件验证。机器记录见[植被指数验收](../prototype/qa/modis-vegetation-verification.json)。

这份初始验收覆盖该产品十二个科学图层中的 NDVI、EVI 两层，记录保留原值的处理模式。后续十个辅助图层及逐像元观测日期见[辅助科学层](modis-science-layers.md)，新的[联合质量筛选](modis-vegetation-quality.md)另有实际文件验收；原有记录和快照保持独立。下载的是 Planetary Computer 转换 COG，不是完整 NASA 原始 HDF。[公开集合定义](https://planetarycomputer.microsoft.com/api/stac/v1/collections/modis-13Q1-061)

## 软件内操作

探索页选择上述数据源，按区域和日期检索，选择影像后创建工程，或追加到当前工程。默认同时下载 NDVI 和 EVI，也可选择单独一个指数。工程详情提供各指数的下载、拼接和裁剪；回到探索页会恢复来源和已选景，并可随时返回同一工程。

列表显示完整 16 天合成时段及 Terra / Aqua 平台，不提供光学整景云量筛选。探索地图可直接在线预览当前景，顶部切换 NDVI / EVI，左下角图例显示合成时段及固定 −0.2 至 1.0 范围。预览使用 Planetary Computer Data API 按所选条目和科学波段生成的 Web Mercator PNG 瓦片，以最近邻重投影，NoData −3000 透明，未应用质量掩膜；不需要创建下载任务或先下载整景。网络失败会显示重试入口，切换指数、日期或数据源会取消旧请求。下载后的指数在工作空间按原始正弦投影网格显示，支持原值检查。在线 RdYlGn 配色与本地 modis-vi-v1 配色各自独立，超出产品范围的显示色会截断；预览颜色不用于精确读值或判定质量。[官方 Data API](https://planetarycomputer.microsoft.com/api/data/v1/docs)

列表缩略图也使用固定范围的 NDVI 配色。供应商默认浏览图将原始 DN 区间调色板与 0–255 重标度组合，在实际条目上几乎只显示一种颜色；这里分别指定连续 RdYlGn 调色板及 DN −2000 至 10000 范围，避免这种显示错误。

在“保留原值”模式下，NDVI、EVI 分别保存与处理。重叠区使用较新的有效值，较新文件的 NoData 可以由较旧的有效值补齐；该模式不对两层联合选择同一观测，也不应用质量或云掩膜。新的质量模式使用每景四份原文件联合选择，详见[质量筛选](modis-vegetation-quality.md)。同一幅合成影像内，不同像元可能来自不同观测日期。[MOD13 v6.1 用户指南](https://lpdaac.usgs.gov/documents/621/MOD13_User_Guide_V61.pdf)

## 原始值与显示

| 项目 | 已实现及验证的含义 |
|---|---|
| 原文件 | 单波段 Int16，实际三个条目的六个指数 COG 均为 4800 × 4800 |
| 指数换算 | 原始 DN × 0.0001；例如实际 EVI 样本 DN = −367，对应 −0.0367 |
| NoData | −3000；显示透明，原值读取明确返回 NoData，不返回指数值 |
| 产品有效范围 | DN −2000 至 10000；超出范围的原值保留，并与显示范围区分 |
| 显示 | 固定 −0.2 至 1.0 颜色范围；整数插值采用明确的四舍五入规则，不改变科学值 |
| 网格 | MODIS 正弦投影、PixelIsArea；标称 250 米，实际像元间距约 231.656358264 米 |
| 工程输出 | 保留 Int16、比例系数、NoData、原始网格及逐文件来源 / SHA-256；不重投影或重采样 |

原值检查独立于预览分辨率；地图点击从完整本地 TIFF 读取 DN 和指数值。原文件及工程结果的界面说明不混用反射率、RGB 或 SCL 的解释。比例、时段、网格、通道或文件校验不匹配时拒绝处理或复用。

## 实际证据

六份完整公开 COG 共 **251,259,645 字节**，覆盖 Terra、Aqua 和相邻 h08v05 / h09v05 瓦片。下载由原生任务执行，使用公开目录验证身份和 Planetary Computer 的临时签名访问。后续验收复用这些原件，不把重复检查、浏览图或副本计为新下载。

独立 GDAL / Rasterio、NumPy、投影与几何工具核对了 138,240,000 个原始 DN、八份成果的全部 **110,592 个 DN**、3,854,876 个预览 / 缩略图 RGBA 像素，以及 92 个原值点。处理包括单瓦片、相邻 / 重叠多景、带孔多边形和真实较旧景补齐：补齐范围中 NDVI 有 71 个像元来自较旧 Aqua，EVI 有 104 个；两层结果独立，不能推定同一观测来源。

实际离线重启恢复十四份原件 / 成果及其缓存；另外移除全部六份原件，只保留八份成果，冷启动与再次重启仍能读取地图、原值和持久化缩略图。缓存损坏后重建得到相同图像；修改原文件或成果文件后拒绝复用旧缓存；恢复文件后结果一致。

直接与本地桥接两种只读 MCP 共核对 184 次原值读取，并检查写入拒绝、断连拒绝和协议输出。生产界面以 1440、1024、900 像素视口和中英文 / 深浅主题验证四个实际处理按钮、完整缩略图、统一卡片高度、栅格检查及地图点击；实际鼠标点击结果与独立 GDAL 读取一致。

探索流程另向公开服务实时获取六条目录记录和六张 PNG 浏览图，验证十五个来源入口、工程恢复、NDVI/EVI 下载选项及返回同一工程。该测试只打开下载入口，未重复创建下载任务。

前端逻辑 255 项、完整界面回归 224 项通过；原生运行时库 473 项通过、4 项明确忽略，格式、严格 Clippy、隔离 / 契约 / 配方检查通过。最后调整下载检查文案后，相关六项界面测试与检查再次通过。

生产资源与带本地资源的 Windows 桌面开发程序构建成功。开发程序为 **80,978,432 字节**，SHA-256 和完整 663 个资源文件登记在机器记录中；十一项汇总反例检查通过，原有运行程序和历史 Landsat 记录保持原哈希。

## 复核与剩余范围

可重复脚本包括[实际原文件下载](../scripts/verify-vegetation-sources.mjs)、[独立全值处理核对](../scripts/verify-vegetation.py)、[两种 MCP](../scripts/verify-vegetation-mcp.py)、[缓存恢复及损坏控制](../scripts/verify-vegetation-cache.py)、[实际界面处理](../scripts/verify-vegetation-ui.mjs)、[实时探索流程](../scripts/verify-vegetation-explore.mjs)和[证据汇总](../scripts/verify-vegetation-closeout.py)。私有原件、失败过程及通过快照分别保留。

在线地图另外使用[实时浏览器验收](../scripts/verify-vegetation-preview.mjs)检查直接网络 / CSP / CORS、两个指数、日期切换、请求取消及错误重试；[独立像元核对](../scripts/verify-vegetation-preview-pixels.py)将真实瓦片像元中心坐标转换回已下载、SHA-256 匹配的原始 COG，比较最近邻 DN、NoData 和显示颜色。两种语言 / 主题 / 窗口配置的九项流程及 242 个独立像元样本通过，[机器记录](../prototype/qa/modis-vegetation-preview-verification.json)绑定源代码及实际响应哈希。Python 3.12 的可选科学验收依赖见[锁定版本](../requirements-raster-qa.txt)。这些验收不创建下载任务，也不替代原生窗口验收。

界面验收使用生产资源、桌面 CSP 和实际原生接口，在隐藏浏览器中运行，未操作用户桌面。它不等同于原生 WebView / 安装版窗口验收；本轮未制作安装包或发布。

NASA / Copernicus 的软件内授权入口继续保留，成功授权和受保护生产文件仍待账号验收。辅助科学 / QA 图层见[十二图层接入](modis-science-layers.md)，指数质量模式见[联合质量筛选](modis-vegetation-quality.md)；完整 HDF、其余科学分析、通用重投影与其他来源的剩余能力见[接入状态](provider-integration-status.md)。公开访问不代替具体来源许可，软件保留 LP DAAC 与 Planetary Computer 的来源说明。
