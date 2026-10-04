# ArcGIS 地图与影像服务

2026-10-03。本页记录公开 ArcGIS REST `MapServer` / `ImageServer` 的渲染影像接入；矢量查询另见 [ArcGIS 要素服务](arcgis-features.md)。

在 **我的数据 → 地图影像 → 获取地图影像** 中新增连接，服务类型选择 **ArcGIS MapServer / ImageServer**，填写公开 HTTPS 服务根地址。MapServer 可搜索并选择独立图层；分组图层及暂不支持的维度会显示兼容原因。ImageServer 使用服务默认渲染。示例按钮只填写 USGS NAIP 地址，连接和保存仍由用户执行。

区域沿用探索页当前 WGS84 搜索范围，输出长边可选 512 / 1024 / 2048 像素；尺寸还受服务声明限制。声明时间维度的图层必须选择范围内的明确 UTC 时间，支持日期或 RFC3339 时间，精度到毫秒。时间是服务可视化时间，不推断采集日期。

服务会调整范围来匹配图片比例，因此软件先获取 JSON 导出响应，再下载其中的 PNG，按**实际返回范围**配准。请求范围、明确时间、原始服务 / 图层元数据、原始导出响应、图片 SHA-256 和署名都持久保存。每次获取使用独立请求标识，避免缓存的 JSON 引用过期临时图片地址。请求沿用软件代理设置；图片链接必须保持同一公开 HTTPS 来源，并位于受支持的服务生成目录。错误、未知投影或返回尺寸不一致不会登记为成功。

已保存图片和缩略图从本地 PNG 读取，重启后无需远端临时链接。地图查看使用实际返回的 EPSG:4326 像元角点范围。ZIP 导出含精确 PNG、像元中心 PGW、PRJ、GDAL PAM 配准、来源 JSON、原始服务 / 图层及导出响应和全部文件校验和。服务声明的访问限制和版权会保留；公开访问本身不表示所有重用权利。

当前范围是单图层、服务默认样式的有界渲染图，每边最多 2048 像素、PNG 最大 16 MiB，元数据响应每份最多 2 MiB，导出响应 64 KiB；受管记录总量还受 32 MiB 注册表限制。不是原始多光谱波段、校准测量值、传感器分辨率或精确多边形裁剪。暂不支持认证 / 内网、编码服务路径、跨主机输出链接、自定义渲染 / 镶嵌规则、波段选择、其他输出投影、组合图层、维度切片、任务队列及工程处理。原始 NAIP 四通道 COG 工作流见 [NAIP 原文件](naip-inspection.md)。

## 实际验收

验收采用隔离运行时和真实公共请求，没有改动用户工程或操作桌面：

- Esri USA MapServer 的 States 独立图层：512 × 256，2,320 字节。请求 `[-123,37,-122,38]`，实际返回 `[-123.5,37,-121.5,38]`，验证扩大范围后的准确配准。
- USGS NAIP ImageServer：256 × 256，160,739 字节。服务调整纬度范围，来源保留请求与实际范围；独立请求返回临时 PNG 的全部字节相同。
- 界面实际选择 USGS 服务、图层、512 像素长边并保存当前搜索区域：512 × 356；独立核对导出和配准。

三个实际输出共 **378,880 个像素 / 1,515,520 个 RGBA 通道值**，Pillow 与 GDAL 全部解码结果相同，GDAL 的 CRS 和实际范围、PGW 中心坐标、来源及原始响应逐项核对。这里验证的是实际 PNG 传输、解码和配准，不评估供应商原始数据精度。

1440 中文浅色、1024 英文深色的真实本地图像、缩略图、连接图层选择和来源详情均通过无桌面操作的界面检查。实际进程重启后再次检查三张图像、原始元数据和导出；拒绝网络连接的代理下离线读取仍通过。安装版 Windows WebView 的人工验收未完成。

验收入口：`scripts/verify-arcgis-map-public.py`、`scripts/verify-arcgis-map-restart.py`；可复核记录见 [QA 记录](../prototype/qa/arcgis-map-verification.json)。前端界面检查脚本及图片在本地忽略的 `.verification/arcgis-map-public/`，不会作为产品数据发布。

官方协议依据：[Export Map](https://developers.arcgis.com/rest/services-reference/enterprise/export-map/)、[Export Image](https://developers.arcgis.com/rest/services-reference/enterprise/export-image/)。
