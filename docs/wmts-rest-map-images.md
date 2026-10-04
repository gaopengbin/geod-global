# WMTS REST 与能力文件地址

2026-10-03。**我的数据 → 地图影像 → 获取地图影像 → 添加地图服务** 支持 WMTS 1.0.0 服务接口（KVP）和服务能力文件（XML）两种地址。选择 WMTS 后可切换地址类型；“填入 NASA GIBS”随选择填入对应示例。连接后使用共用的图层、日期、样式、网格和层级选择，保存到本地并在工作空间查看。刷新连接保留原来的地址类型。

XML 地址示例：

```text
https://gibs.earthdata.nasa.gov/wmts/epsg4326/best/1.0.0/WMTSCapabilities.xml
https://gibs.earthdata.nasa.gov/wmts/epsg3857/best/1.0.0/WMTSCapabilities.xml
```

能力文件地址直接读取 XML，不追加 KVP 参数。新发现的图层优先使用服务声明、能够绑定所选日期、样式和矩阵集的 `ResourceURL`，否则使用已声明的 KVP GetTile。REST-only 能力文件也可连接，其支持由合成协议测试验证；本次真实 NASA 服务同时声明 KVP 与 REST。[NASA GIBS 接口说明](https://nasa-gibs.github.io/gibs-api-docs/access-basics/)，[OGC ResourceURL 字段定义](https://schemas.opengis.net/wmts/1.0/wmtsGetCapabilities_response.xsd)。

## 网格和来源

REST 复用 [WMTS 原生网格处理](wmts-map-images.md)：支持声明为 WGS84 / CRS:84、Web Mercator 的 PNG / JPEG 瓦片，按原生像元窗口保存，不重采样。矩阵标识保持服务原文，不能把任意 WMTS 矩阵当成 XYZ 缩放级别。界面显示“瓦片访问方式”；来源详情保留选中的模板、实际日期、样式、网格、每块瓦片 URL / 字节数 / SHA-256，以及原瓦片归档和输出 PNG 校验值。

支持绝对地址和同源相对路径。模板必须显式包含矩阵、行列号和声明的日期维度；存在多个样式或矩阵集时还必须能够选择对应维度。路径标识按 UTF-8 编码，保留服务定义的大小写、空格和特殊字符。不会隐式使用默认日期模板代替用户所选日期。

服务和瓦片地址限公开 HTTPS、443 端口、同源路径；不跟随重定向，不接收 URL 凭据、自定义查询参数、额外维度、跨源模板或主机占位符。不能使用的 REST 模板会留下兼容性说明；有可用 KVP 时回退，无兼容瓦片地址时排除该图层。既有 WMS 的端点目录限制不变。

既有 KVP 保存记录继续按其原来的请求读取；新增模板和能力文件标记使用兼容的可选字段。导出仍包含 PNG、坐标文件、GDAL 元数据、原瓦片 ZIP、来源和逐文件校验值。本地查看与导出不重新请求远程瓦片。

## 实际验收

通过隔离的原生运行时直接连接 NASA 两个 XML 地址，日期为 `2025-06-27`。使用独立 HTTP、Pillow 和 GDAL / Rasterio 核对所声明模板、矩阵参数、全部输出通道、原瓦片字节、投影及像元变换：

| 操作 | 图层 | CRS / 原瓦片 | 尺寸 | 原瓦片记录数 | 独立像元核对 |
|---|---|---|---|---|---|
| 原生接口 | MODIS Terra 真彩色 | EPSG:3857 / JPEG | 456 × 575 | 9 | RGB 最大差 4、平均约 0.127；alpha 一致 |
| 原生接口 | MODIS Terra 日间地表温度渲染图 | EPSG:3857 / PNG | 456 × 575 | 9 | 所有 RGBA 通道一致 |
| 原生接口 | MODIS Terra 气溶胶渲染图 | EPSG:4326 / PNG | 286 × 285 | 2 | 所有 RGBA 通道一致 |
| 实际界面保存 | MODIS Terra 真彩色 | EPSG:4326 / JPEG | 855 × 741 | 4 | RGB 最大差 4、平均约 0.143；alpha 一致 |
| 实际界面保存 | MODIS Terra 日间地表温度渲染图 | EPSG:3857 / PNG | 683 × 740 | 16 | 所有 RGBA 通道一致 |

5 份输出共 1,744,885 像素、40 条原瓦片记录；包含重叠瓦片，不能理解成 40 个不同空间瓦片。每块原始 PNG / JPEG 都与独立 HTTP 响应字节一致。JPEG 的通道差来自独立解码器；三个 PNG 输出共 3,396,520 个 RGBA 通道值完全一致。每份导出的范围、像元变换、CRS 和全部通道均通过独立检查。

不同请求收到的能力 XML 可能具有不同的属性和命名空间排列。独立请求文件与原生请求的哈希不强制相等，两份响应各自的哈希均保留；验收逐项核对实际使用的声明模板和矩阵，未把独立请求哈希冒充为原生响应哈希。

实际界面检查使用无头 Chromium、真实原生取图和桌面开发 CSP，覆盖 1440 像素英文浅色、1024 像素中文深色：连接、刷新、共用日历、网格选择、等高卡片、来源模板、实际地图和导出均通过，无页面脚本错误或水平溢出；截图已人工查看。未操作用户桌面，未验收安装版 WebView 或本机保存对话框。

更新后的原生进程重启后，2 个连接和 5 张新影像在拒绝连接的代理下仍可本地读取与导出，元数据、影像及整个导出包字节一致。另用独立 CLI 进程重开旧存储，6 张 WMTS KVP、6 张 WMS 和 3 张 XYZ 影像及其来源、坐标包和原瓦片归档均保持一致。

本轮通过 352 项原生核心、3 项 CLI、4 项桌面、184 项前端逻辑和 174 项界面测试；4 项显式启用测试未运行。依赖隔离、接口契约、方案结构、生产前端构建、格式和严格 Clippy 检查通过。具体来源与结果见 [验收记录](../prototype/qa/wmts-rest-verification.json)。复验脚本：[公开取图](../scripts/verify-wmts-rest-public.py)、[重启读取](../scripts/verify-wmts-rest-restart.py)。

## CLI 与限制

连接能力文件时明确发送 `wmtsDocument: true`；省略或为 false 时仍为 KVP 服务接口。此字段不能用于其他协议。

```json
{
  "name": "NASA GIBS",
  "url": "https://gibs.earthdata.nasa.gov/wmts/epsg4326/best/1.0.0/WMTSCapabilities.xml",
  "protocol": "WMTS",
  "wmtsDocument": true
}
```

每次取图仍限 16 块瓦片、输出每边 2048 像素、单瓦片 4 MiB、原瓦片 ZIP 64 MiB、组装 PNG 16 MiB、120 秒总超时。XML 限 8 MiB，不接收 DTD / 实体声明；每个图层最多读取 128 个瓦片模板。任务队列、取消、断点续取、共享瓦片缓存、大范围下载、其他投影 / 维度、认证服务、图层组合和工程处理仍待推进。

这些产品是服务渲染颜色，不是科学原波段或温度数值。NASA Earthdata / Copernicus 的授权入口已提供；真实受保护原文件的正向验收仍需有效账号。此项完成不表示全部数据源已接入，剩余范围见 [接入状态](provider-integration-status.md)。
