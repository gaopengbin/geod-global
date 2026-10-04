# 公开 ArcGIS 矢量服务

“我的数据 → 矢量文件 → 获取矢量数据”现可选择 **ArcGIS Feature Service**，填写公开 `FeatureServer` 地址，选择图层并按当前探索区域提取。连接可刷新或移除；移除连接保留已有文件。结果进入本地矢量列表，可离线显示、点击查看原始属性和导出 GeoJSON。中英文界面沿用 OGC 矢量入口和共享组件。

接入对象是公开服务提供的二维 GeoJSON 要素，**不是原始地理数据库下载，也不是整套 ArcGIS 平台适配**。NASA Earthdata / Copernicus 账号入口及其验收边界不受本次影响。

## 查询与完整性

按 [Esri 官方查询接口](https://developers.arcgis.com/rest/services-reference/enterprise/query-feature-service-layer/)执行 WGS84 外接矩形相交查询。先独立获取数量与对象 ID，再按明确 ID 分批请求全部字段和完整几何，最后重新核对数量、ID 集合及图层定义。不是基于偏移量翻页；不发送简化、量化或精度截断参数。多边形选区另存为来源，结果不隐式裁剪。

批次必须精确覆盖请求 ID，每个要素的 ID、字段名、几何类型与二维坐标必须一致。顶层或 GeoJSON `properties` 内的截断标志、HTTP 成功但含 ArcGIS 错误对象、重复或遗漏 ID、字段定义变化都会使提取失败，不登记部分文件。独立计数为零时支持服务真实返回的 `objectIds: null`；缺失该键不视为空结果。

图层发现要求可查询、声明 GeoJSON、二维几何和一个数值对象 ID 字段。Z / M、明确声明曲线的图层、哈希对象 ID 及不支持字段类型会列出排除原因。支持普通点、多点、线和面；不承诺 GeoJSON 保留服务背后的原始曲线。`supportsTrueCurve` 只代表服务能力，不能证明数据是否含曲线。

查询前后核对字段名称 / 别名 / 类型、对象 ID 字段、几何类型、源坐标系、批次上限及署名声明。数量和 ID 不变不代表属性在提取期间没有更新，**不是事务一致快照**。在服务返回 GeoJSON 前发生的投影、曲线线性化或数值格式转换属于服务输出语义。

上限为 5,000 个要素、25 个批次、每批最多 200 个且不超过服务声明上限、20 MiB 网络响应总量、120 秒网络提取；元数据发现另有 8 MiB 上限。GeoJSON 文件继续受现有 20 MiB / 50 万坐标上限约束。接收安全非负整数 ID；超出 JavaScript 安全整数范围的数据会明确拒绝。完成网络提取和验证后才原子保存文件。

## 文件与来源

保存服务 / 图层身份、查询区域与提取时间、字段名称 / 别名 / 类型、源坐标系、元数据 SHA-256，以及每批准确 POST 参数、响应字节数 / SHA-256 / 要素数。前后数量与 ID 请求各保留两份回执，并记录完整排序 ID 列表。源坐标系与输出 WGS84 分开记录；日期和属性值不擅自格式化或转换为展示用值。

保存及导出的 `geodSource` 跟随受管 GeoJSON；打开和导出均核对文件 SHA-256 与来源。连接与文件登记分别保存在 `feature-services.json` 和 `vectors.json`，文件内容位于 `vectors/`。导出的是保存的服务 GeoJSON，不是原始 HTTP 响应逐字节归档。不同 JSON 小数字面量可能对应同一 Double 数值。

版权署名保留服务声明，不自动解释成数据许可；需查看数据提供方的授权。当前不复制字段域、子类型、关系表、附件或完整图层定义，不把保存的字段摘要称作完整地理数据库模式。

网络遵循软件的跟随系统 / 直连 / 自定义代理设置。仅公开 HTTPS，同服务限定、地址检查和禁止重定向复用现有矢量网络层。受保护服务明确返回需要授权；本次没有实现 ArcGIS OAuth / API Key，也没有绕过受保护服务。

## 实际验收

[验收记录](../prototype/qa/arcgis-features-verification.json)来自运行中的 Rust 核心和独立公开 HTTP 请求：

| Esri 公开样例 | 实际提取 | 独立核对 |
|---|---|---|
| Earthquakes Since 1970 / 0 | 6 个点，3 个批次 | 28 个字段、168 个属性值、12 个坐标值；另验证真实零结果区域 |
| Wildfire / 1 | 1 条线，1 个批次 | 9 个属性值、16 个坐标值；保留查询框外完整几何 |
| Wildfire / 2 | 10 个面，3 个批次 | 90 个属性值、184 个坐标值；保留查询框外完整几何 |

逐要素比较独立 GeoJSON 的全部属性与完整几何结构，并额外比较 Esri JSON 的全部属性和坐标位置。线和面源坐标系为 Web Mercator，由服务返回 EPSG:4326；这不代表软件实现了通用矢量重投影。公开样例可随时变化，不作为生产数据权威性或稳定性保证。

英文浅色 1440 像素和中文深色 1024 像素界面均完成真实连接、刷新、区域提取、地图选点、原始属性展示、来源展开及浏览器导出一致性检查。停止服务后，6 份已保存结果（含空结果、线、面和两个界面查询）在新原生进程中离线恢复、校验并完整导出；3 份既有 OGC 文件亦恢复通过。

上述界面验收使用无界面 Chromium，没有操作用户桌面；Windows 桌面命令与权限编译测试通过，安装版 WebView 未人工操作。本次不打发布包。

## 复验入口及剩余范围

CLI 沿用 `feature-services connect|query` 与 `vectors inspect|export`，连接参数示例：

```json
{"name":"Esri 历史地震样例","url":"https://sampleserver6.arcgisonline.com/arcgis/rest/services/Earthquakes_Since1970/FeatureServer","protocol":"ArcGIS"}
```

`protocol` 省略时仍按原 OGC API Features 处理，旧连接和旧文件无需迁移。查询中的 `collectionId` 为图层编号字符串；可选 `pageSize` 为 1–200。

实际复验脚本为 [点与空结果](../scripts/verify-arcgis-public.py)、[线和面](../scripts/verify-arcgis-shapes.py)、[离线恢复与导出](../scripts/verify-arcgis-restart.py)。在独立数据目录运行；脚本的 `--direct` 仅选择独立核对请求的网络路径，不修改系统或软件代理设置。

仍待接入：MapServer / ImageServer、ArcGIS 账号授权、非空间表、附件 / 关系 / 字段域、Z / M 与原始曲线、时间 / 属性查询、事务历史时刻、超限大规模提取、任务队列 / 取消 / 续取、精确几何裁剪及工程内矢量处理。其他平台及完整产品目标继续以[接入状态](provider-integration-status.md)为准。
