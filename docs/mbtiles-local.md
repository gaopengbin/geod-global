# 本地 MBTiles 导入与离线地图

2026-10-03。MBTiles 与 [PMTiles](pmtiles-local.md) 共用“我的数据 → 离线瓦片”入口，当前接入独立文件的 gzip MVT（`pbf`）、PNG 和 JPEG（`jpg`）。

## 使用

点击“打开瓦片文件”选择 `.mbtiles`。桌面版通过本机文件选择窗口导入；开发浏览器向本机服务提交文件内容。校验成功后保留完整受管副本，原输入文件可以移动或删除。取消选择或校验失败不新增完成记录。

卡片的三个图标分别打开地图、导出与来源详情。矢量地图可筛选图层、点击要素查看原属性；图片瓦片直接显示已保存的 PNG / JPEG，不套用矢量样式。详情面板支持调整宽度，沿用统一的明暗主题。加载提示等瓦片绘制及淡入结束再消失，动效仍保留。

地图角落的层级采用实际源瓦片网格，支持 256 / 512 像素等不同瓦片大小，不把地图视图的缩放数值误当成源层级。

直接启动本地数据或已保存地图页面时不自动请求在线影像目录；首次进入探索页才开始目录搜索。已由用户发起的搜索仍保留原有行为。

## 文件与坐标

MBTiles 以 SQLite 保存瓦片。读取器支持普通 `metadata` / `tiles` 表，也支持由普通表构成的兼容视图，包括 `map` / `images` 关联结构；元数据 `name` / `value` 的列顺序不受限制。原文件的 TMS 行号换算为地图使用的 XYZ 行号，数据库字节、原元数据与图片压缩内容保持不变。[MBTiles 1.3 规范](https://github.com/mapbox/mbtiles-spec/blob/master/1.3/spec.md)

每个文件最多 128 MiB、512 个瓦片，级别不超过 24；PNG / JPEG 必须为一致大小的正方形瓦片，边长 1–1024 像素。读取还限定单瓦片解压、累计解码量、元数据、查询时间和并发内存占用。数据库采用只读内存连接，不加载扩展或访问外部文件；复杂函数视图、SQLite 虚拟表和依赖外部 WAL 的文件会被拒绝，WAL 文件应先生成独立检查点副本。

受管文件为 `tiles/<UUID>.mbtiles`，登记于 `tiles.json`。记录包含完整 SHA-256、格式、范围、级别和逐瓦片逻辑坐标 / 校验值；MBTiles 不记录伪造的 PMTiles 物理偏移、网络 URL 或 ETag。相同文件名与内容重复导入时核对并复用已有完好副本。已有 PMTiles 来源记录、文件和导出内容保持兼容。

ZIP 导出包含完整的 `tiles.mbtiles`、`source.json`、`README.txt` 和 `checksums.sha256`，可独立读取原数据库。软件不添加用户原输入的绝对路径，生产者原有元数据仍完整保留。MBTiles 容器不授予数据使用权；矢量瓦片仍有量化、简化、缓冲及重复的性质，图片瓦片是渲染地图，不等于科学影像原始波段。矢量预览不下载外部样式、字体或图标素材。

CLI 共用现有命令：

```text
geod-runtime tile-packages open --file region.mbtiles --data-dir ./local-data
geod-runtime tile-packages inspect --id UUID --data-dir ./local-data
geod-runtime tile-packages tile --request tile-request.json --data-dir ./local-data
geod-runtime tile-packages export --id UUID --out region.zip --data-dir ./local-data
```

`tile-request.json` 内容为 `{"id":"UUID","z":7,"x":20,"y":49}`，行号采用 XYZ。桌面命令和开发回环服务共用相同读取器。

## 验收与边界

输入包括一个生产者提供的公开文件和三个独立构造的容器，分别记录来源，不能混称为供应商原始 MBTiles：

| 输入 | 容器来源 | 实际核对 |
|---|---|---|
| FOSS4G-Buenos Aires | [MapTiler workshop](https://github.com/maptiler/foss4g-workshop) 的[完整公开 MBTiles](https://dev.maptiler.download/foss4g/mbtiles/tiles.mbtiles)，7,110,656 字节 | 186 个瓦片、跨瓦片与级别 407,426 个要素记录；普通表关联视图、原元数据及署名 |
| OSM 视图包 | 以此前实际获取的 Protomaps 瓦片原字节，通过独立 Python SQLite 构造 | 3 个瓦片、2,100 个记录；不是供应商交付的 MBTiles 原包 |
| NASA GIBS PNG / JPEG 包 | 以此前实际 WMTS 返回的 PNG / JPEG 原字节，通过独立 Python SQLite 构造 | 各 9 个瓦片、589,824 个解码像素；不是 NASA 科学原文件或 NASA 提供的 MBTiles |

共 207 个瓦片由独立 SQLite、`mapbox-vector-tile` 和 Pillow 读取，核对全部 TMS / XYZ 坐标、原瓦片字节、图层摘要、完整数据库、ZIP 内容及校验清单；要素数是跨级别记录数，不是唯一对象数。移走验收原输入、重启服务并设置拒绝外部访问的代理后，已保存数据与完整 ZIP 保持一致。

1440 像素浅色英文、1024 像素深色中文界面执行真实本机导入、错误文件拒绝、地图显示、道路属性、来源详情和重启恢复。图片截图等待实际绘制完成；离线阶段阻断并检查全部外部请求。开发回环接口另查来源 / 客户端边界、路径拒绝和重复导入。使用无界面浏览器，未操作用户桌面，也未进行安装版 WebView 或本机选文件窗口的人工验收。

边界与拒绝行为还使用明确标注的合成夹具覆盖：列顺序、重复行、非法坐标、错误格式 / 压缩、空 MVT、WAL、复杂视图、带注释的虚拟表声明、损坏副本与资源限制。它们不代表新增生产数据源验收。

验收记录见 [mbtiles-local-verification.json](../prototype/qa/mbtiles-local-verification.json)，可重复独立读取器见 [verify-mbtiles-local.py](../scripts/verify-mbtiles-local.py)；验证用 Python 包不是产品依赖。

WebP、其他压缩、大文件 / 任务队列、精确区域裁剪、样式依赖、工程组合、MCP 和安装版人工验收仍待接入，不能将这个有界文件入口标为全部离线瓦片功能完成。
