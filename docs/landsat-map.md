# Landsat 探索页真彩色地图

2026-10-01，Landsat 8/9 Collection 2 Level-2 已接入探索页的地理配准真彩色显示、多景加载、同网格日期对比及工程选景恢复。地图读取原始 B4 / B3 / B2 COG，不把平台缩略图放到地图上冒充原始栅格。

## 显示与原文件

- 三个通道都要求 UInt16、NoData=0、30 米、同一 UTM CRS、尺寸与原始格网。缺失波段、错误通道、其他产品路径、已签名的持久化 URL 或不匹配元数据会被拒绝。
- 显示先按 `DN × 0.0000275 - 0.2` 转换反射率，再使用 0–0.3 范围与 gamma 2.2。任一通道为 NoData 时透明；不把云自动当作 NoData，也没有 QA 云掩膜。
- 拉伸只影响在线地图。下载与本地单波段检查、裁剪、拼接继续保留原 DN、NoData、scale / offset，见[反射率检查](reflectance-inspection.md)与[工程处理](reflectance-processing.md)。另有[本地原始三波段 RGB 组合](local-rgb.md)用于工作空间显示和原值检查，使用采样 2–98 百分位拉伸；尚未提供三波段 RGB 合成文件导出。
- PixelIsPoint 的原始 tiepoint 是像元中心，地图换算为像元外边界；实际样本的外边界与 STAC / GDAL 一致，没有 15 米偏移。

## 访问和重试

签名通过官方 `/api/sas/v1/token/landsateuwest/landsat-c2` 容器接口获取。与 [Planetary Computer 官方 SDK](https://github.com/microsoft/planetary-computer-sdk-for-python/blob/main/planetary_computer/sas.py) 的容器缓存方式一致，同一轮选景共用一次临时只读签名；并发请求合并，快过期时刷新，地图重试强制更新。令牌只驻留内存，不进入目录对象、工程、任务、浏览器持久化存储或验收记录。访问仍只允许已审核的原始 B4/B3/B2 地址。

开发验收曾在按文件申请签名时收到真实 HTTP 429；改为共享容器签名后，两景六波段只需一次请求。共享缓存减少签名调用，不保证提供方永不限流。429 显示独立提示。元数据、瓦片和对比影像均有加载反馈、错误与重试入口。

OpenLayers 版本固定为 10.10.0。PixelIsPoint 适配使用其 `configure_` 内部钩子；[适配回归测试](../prototype/src/explore-imagery-source.integration.test.jsx)运行实际 OpenLayers 格网、变换方向及 view Promise，并替换远端传输。升级依赖时必须重验这些测试与真实 COG，不只检查编译。

## 验收证据与边界

[验收记录](../prototype/qa/landsat-map-verification.json)区分真实网络、真实本地原文件和 UI 隔离夹具：

- 真实公开目录查询成功；在线栅格均使用真实签名 HTTPS Range，三通道返回 206。
- `LC09_L2SP_044034_20250628_02_T1` 的 33 个浏览器原始 DN 样本与之前实际下载原文件经独立 Rasterio / GDAL 读取的值全部一致。其地图外边界为 `[462285,4030485,692415,4264215]`，与 GDAL 一致。
- 两景真实目录项在浅色 1440、深色 1440、浅色 1024 下完成多景显示、工程选景恢复和滑动对比，未出现页面异常或水平溢出。第二景是 `LC09_L2SP_044034_20251205_02_T1`。UI 的工程上下文为隔离夹具，目录项重放固定场景；签名、元数据、像素范围请求均真实。
- 故意错配尺寸时，真实 COG 元数据使 view Promise 拒绝并进入错误态。另一次 UI 测试注入 B4 HTTP 503；重试后恢复真实地图，重新申请一次容器签名。注入失败不代表提供方真实故障。
- 本轮没有下载第二景完整文件，没有验收 NASA / Copernicus 的账号授权成功或受保护原文件，也没有生成安装包、发布版本或操作用户桌面。
