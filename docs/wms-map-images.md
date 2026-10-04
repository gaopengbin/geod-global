# WMS 地图影像

2026-10-02。桌面原生核心、软件界面和 CLI 共用公开 WMS 服务适配。入口为 **我的数据 → 地图影像 → 获取地图影像**。填入服务名称和不带查询参数的 HTTPS WMS 地址，也可以点击“填入 NASA GIBS”后明确连接。

连接实际读取 GetCapabilities，保存服务目录。搜索并选择图层、明确服务日期、输出尺寸和样式，按当前探索搜索区域获取一张 PNG；多边形另行记录，当前影像覆盖它的外接矩形，**本次不做多边形裁剪**。这不会创建卫星原文件下载工程。请求使用设置中选定的系统、直连或自定义代理。

影像保存在独立应用数据目录，可离线打开地理工作空间并平移、缩放、定位。卡片缩略图从校验过的持久文件懒加载；缩略图填满卡片左侧，来源详情使用弹窗，默认卡片高度一致。工作空间信息面板可以拖动调整宽度，底图使用本地 Natural Earth 参考数据。

## 数据和坐标含义

WMS 返回的是服务端渲染结果，显示颜色不是原始科学波段或经过校准的测量值。输出宽高表示渲染网格，不能标成传感器分辨率。服务日期表示请求的可视化时间，不能直接当成单景卫星拍摄时间。透明像素可能表示缺少影像，但不能仅凭透明像素判定覆盖或服务是否可用。[NASA GIBS 服务说明](https://nasa-gibs.github.io/gibs-api-docs/access-basics/)

已处理 WMS 1.3.0 的 EPSG:4326 纬度 / 经度 BBOX 顺序，CRS:84 及 WMS 1.1.1 的经度 / 纬度顺序；保存和显示的网格统一以 WGS84 经度 / 纬度记录。服务端若返回不同尺寸、损坏 PNG、XML 异常或错误响应类型，不登记影像。

来源保存服务与图层、请求地址 / 日期 / 时间、实际宽高与范围、样式、Capabilities SHA-256、原样 PNG SHA-256、声明的使用限制和署名。使用限制为空或 `none` 只表示服务的元数据声明，不是软件替所有图层授予再利用许可。NASA GIBS 中不同来源的数据仍需遵循各自要求。[NASA 数据使用政策](https://www.earthdata.nasa.gov/engage/open-data-services-software/data-use-policy)

## 导出

桌面使用本机保存对话框导出 ZIP；CLI 使用明确目标路径。不会覆盖已有文件或写入 GeoD 受管存储目录。导出包含：

- `map.png`：服务实际返回的原样 PNG。
- `map.pgw`：经度 / 纬度网格的像元中心坐标与间距。
- `map.prj`：WGS84 投影声明。
- `map.png.aux.xml`：GDAL 可识别的坐标系和像元角点变换。
- `source.json`、说明和逐文件 SHA-256。

包中不包含绝对本机路径。影像、空间范围和来源记录仍然包含用户选择的区域信息。

## 实际验收

公开服务：`https://gibs.earthdata.nasa.gov/wms/epsg4326/best/wms.cgi`。实际读取到 1393 个符合当前适配条件的图层，这个数不是全部 GIBS 产品的下载验收。

实际请求 `MODIS_Terra_CorrectedReflectance_TrueColor`、服务日期 `2025-06-27`、范围 `[-125,30,-110,43]`，返回 RGBA PNG 512 × 444。PNG SHA-256 为 `21247fc9236eb0c8d156dbe5b72a4a065b86cf8fee49c09de1f25e005f782a63`，与独立公开 GetMap 请求收到的文件一致。Pillow 验证图像，GDAL / Rasterio 独立确认导出包的 CRS、范围、变换与四个颜色通道；ZIP 中影像字节、来源和所有校验值一致。

可复验脚本：[verify-wms-public.py](../scripts/verify-wms-public.py)。真实记录与界面 / 重启状态见[验收记录](../prototype/qa/wms-public-verification.json)。界面使用无头 Chromium 和隔离的实际原生运行时，未操作用户桌面；Tauri ACL 和编译 / 测试通过，不把这些检查声称为安装后的 WebView 或本机保存对话框人工验收。

## 当前边界与后续工作

公开 HTTPS、443 端口、无 URL 凭据 / 自定义参数，不跟随重定向；GetMap 链接限定在连接服务同主机同目录。XML 最大 8 MiB，不读取外部 DTD，不允许实体声明；最多 24 个连接、每连接 4096 个兼容图层。当前只支持可选取子范围、无固定宽高限制、WGS84 / CRS:84、PNG、单个图层及可声明的时间维度；其他强制维度不自动填值。

单张影像每边不超过 2048 像素和服务声明限制，PNG 不超过 16 MiB；最多登记 512 张影像。这是当前单次渲染限制，不代表大型区域瓦片下载或拼接已经实现。时间支持明确日期 / RFC3339 枚举、日 / 周 / 月 / 年和小时 / 分钟 / 秒的简单周期；不将不支持的周期、`current` 或缺失日期偷偷替换成最近时间。取图为有界请求，尚未接入持久任务队列、取消、断点续取、WCS、自定义 STAC / COG、WMS 其他投影、GetFeatureInfo、多图层组合或卫星原产品处理。公开 WMTS KVP 的独立网格和瓦片流程已接入同一地图影像入口，范围与证据见[WMTS 说明](wmts-map-images.md)。

## CLI

```sh
geod-runtime map-services connect --request connection.json --data-dir DIR
geod-runtime map-services list --data-dir DIR
geod-runtime map-images get --request map-request.json --data-dir DIR
geod-runtime map-images list --data-dir DIR
geod-runtime map-images inspect --id UUID --data-dir DIR
geod-runtime map-images export --id UUID --out OUTPUT.zip --data-dir DIR
```

服务占用同一存储目录时，连接、获取、列表和检查使用 `--server http://127.0.0.1:PORT`，不能同时直接打开目录。`connection.json` 为 `{"name":"NASA GIBS","url":"https://gibs.earthdata.nasa.gov/wms/epsg4326/best/wms.cgi"}`；`map-request.json` 需要保存的 `serviceId`、`layerName`、`style`、明确 `time`、WGS84 `bounds`、`width`、`height` 和可选 `areaGeometry`。导出路径仅由桌面 chooser 或直接 CLI 提供；HTTP 不接受本机任意路径。
