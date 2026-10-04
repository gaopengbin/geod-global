# WCS 区域栅格

WCS 入口用于连接用户指定的公开覆盖服务，读取数据集定义，再按工程区域请求 GeoTIFF。文件是服务端按范围生成的栅格，不能标成整景原文件、原始调查资料或已完成科学校准的数据。

## 使用流程

从探索页或设置中的 WCS 入口添加服务，选择覆盖数据集并读取定义。界面展示服务声明的坐标系、网格、字段、单位、空值和元数据链接。选择区域后，本地运行时根据原生网格计算实际范围及行列数；保存到新工程或已有工程，再开始下载。

范围以 WGS84 输入，输出保留服务原生网格；边缘按原生像元向外对齐，超出数据覆盖的部分取交集。UTM 服务使用加密边界转换后的外接矩形，属于矩形截取，不是精确多边形裁剪。未调用缩放、插值或输出重投影扩展。

下载后可以打开本地灰度地图、查看实际文件信息和原始像元。显示拉伸只影响预览。来源声明与文件实际标签分别保留，不因为字段叫 `Depth` 就把数值标成米，也不把没有声明的 0 当作空值。保存的连接、定义、请求、任务、文件和缩略图可以在重新启动后恢复。

## 支持范围

- WCS 2.0.1 的公开 HTTPS KVP 请求；GetCapabilities、DescribeCoverage 和 GetCoverage。
- 二维 RectifiedGridCoverage，独立的两个空间轴，规则且无旋转或剪切的网格。
- EPSG:4326 / CRS:84、EPSG:3857 和 WGS84 UTM 南北半球。EPSG:4326 正式纬度、经度轴序与普通界面输入的经度、纬度顺序分别处理。
- 服务声明的 GeoTIFF 输出，下载完成前逐块核对全部主图样本、尺寸、通道、数据类型、坐标及空值。声明与结果不匹配时任务失败，不自动重采样来掩盖问题。

当前不包括 WCS 1.x、POST / XML、认证与内网服务、时间或高程等额外维度、非规则网格、范围字段选择、multipart、NetCDF / HDF、任意坐标转换、断点续传及通用科学处理。其他产品和服务需要独立验收；本实现不等于 WCS 规范认证。

单文件沿用 512 MiB 上限。请求子集每边最多 65,536 个像元，主图累计最多 536,870,912 个通道样本；逐块解码使用 128 MiB 缓冲上限和校验和与像元验证共用的 60 秒预算。这些是本地资源预算，不代表所有达到预算的数据布局都可以显示。拒绝超过预算的请求，不擅自降低分辨率。验证在读取、解码前后和样本循环中检查取消；单次有界块解压本身仍为同步执行。

## 来源、工程与恢复

连接记录和原始能力文档、覆盖定义、请求计划保存在独立工作空间中，原始文档及选择使用内容哈希关联。此处 XML 指 HTTP 内容解码后的 XML；支持编码前后均不超过 8 MiB 的 gzip，GeoTIFF 响应要求 identity 内容编码。工程的 `wcsItems` 与既有场景、自定义 STAC 资产可以共存，合计最多 32 个选项。界面提交原生保存的计划引用，不能在下载请求中替换 URL。

下载前重新读取能力和覆盖定义，核对结构性声明；发生变化时要求重新准备请求。相同声明和范围仅改变获取时间时会复用工程已有选择。复用完成文件前重新检查受管路径、字节数和完整 SHA-256；文件丢失或内容改变会重新排队，并保留旧任务的失败原因。排队、取消、重试与退出时的任务恢复沿用共享任务系统。

请求遵循软件的代理设置，不转发 NASA / Copernicus 凭据。连接限制在公开 HTTPS，拒绝凭据和临时签名参数，不自动跟随重定向。服务声明的操作地址保持同源；仅允许把与已连接 HTTPS 地址完全相同主机、无显式端口且同一路径的 HTTP 声明提升为 HTTPS。

费用或访问约束写为 `NONE` 不能证明特定数据许可。软件保留服务声明和元数据链接；未解析或未成功读取的许可证不得显示为已确认。

## CLI 与本地接口

```sh
geod-runtime wcs connect --request connection.json --data-dir workspace
geod-runtime wcs describe --request describe.json --data-dir workspace
geod-runtime wcs plan --request area.json --data-dir workspace
geod-runtime wcs project --request project.json --data-dir workspace
geod-runtime wcs download --request download.json --data-dir workspace
geod-runtime wcs inspect --id JOB_UUID --data-dir workspace
geod-runtime wcs pixel --id JOB_UUID --column 10 --row 10 --data-dir workspace
```

请求分别为 `{name,url}`、`{connectionId,coverageId}`、`{descriptionId,bounds}`、`{name,bounds,selections:[{planId}]}` 和 `{projectId}`。追加已有工程使用 `projectId`，不重复传入工程名。可用 `list` 列出连接、`description --id SHA256` 读取保存定义、`snapshot --id SHA256` 读取保存计划。

桌面或服务已占用工作空间时使用 `--server http://127.0.0.1:4318`，避免第二个进程同时写入目录。下载命令等待任务结束，未完成时退出非零。桌面命令、本地 HTTP 与 `geod_wcs_*` MCP 工具使用同一原生实现；MCP 默认只读取本地连接、目录、定义、计划、工程和文件，连接、发现、准备、保存及下载需启动时启用写操作，具体流程见 [MCP 说明](mcp.md#wcs-agent-workflow)。

## 验收记录

真实样本和每个验收阶段的状态见 [WCS 验收记录](../prototype/qa/wcs-public-verification.json)。应用已从 EMODnet `emodnet__mean` 实际下载经度 2–2.05、纬度 53–53.05 的 48 × 48 单通道 Float32 GeoTIFF，共 9,600 字节。文件 SHA-256、全部 2,304 个原始样本、网格和坐标标签与独立公开请求及 rasterio 读取一致；没有自动重试。

最终运行时重新启动后，六次独立 CLI 读取恢复了相同的连接、请求、工程、任务、文件和原值，全程没有外部网络请求。1440 像素浅色和 1024 像素深色界面通过无桌面操作的浏览器检查，覆盖实际工程、来源计划、缩略图、地图和点击取值；点击像元由独立 rasterio 再次核对。缩略图磁盘缓存的内容和 PNG 哈希在重启后保持一致，命中时仅更新供缓存淘汰使用的访问时间。安装版 Windows WebView 尚未进行人工验收。

共享原生回归通过 337 项、忽略 4 项；WCS 核心专项通过 15 项，另有 6 项 MCP 适配回归。前端通过 172 项普通测试和 169 项界面测试，原生命令权限清单覆盖 77 项命令；最终布局调整另通过 18 项相关界面回归。最终原生构建、静态检查及前端构建均通过。这些验证针对当前支持范围，不表示全部数据源或发布验收已完成。

此前 MCP 验收通过五次标准输入输出会话，覆盖独立目录、已有本地服务和重连恢复；读取了完整的八项服务目录，工程追加和已完成文件复用均成功。25 次原始像元取值与独立 Rasterio 一致，拒绝外部访问的本地代理记录为零请求。这份历史记录复用已验收文件，见 [MCP 文件复用记录](../prototype/qa/wcs-mcp-verification.json)。

新增验收使用全新隔离目录，由真实 MCP 完成公开连接、分页目录、覆盖定义、网格计划、工程保存及新的 GetCoverage 下载。八项目录与服务 XML 一致；新的 48 × 48 文件共 9,600 字节，与第二次独立公开请求逐字节一致，全部 2,304 个 Float32 样本、网格和坐标经独立 Rasterio 核对。MCP 在线读取五个原始像元，随后关闭运行时，在拒绝外部请求的代理下完成五次独立 / 本地服务会话及 25 次原值读取，登记文件字节保持不变，外部请求为零。验收中发现并修复 Windows 独立 MCP 启动时的栈溢出；修复后从新目录重新下载及离线恢复通过。见 [MCP 新下载记录](../prototype/qa/wcs-mcp-public-verification.json)。此流程未操作用户桌面。

该服务把 `Depth` 字段单位声明为 `W.m-2.Sr-1`，与水深产品描述存在疑点；返回 TIFF 没有单位和 NoData 标签，2,304 个样本全部有限，而覆盖定义的 nil 值为 `NaN`。这些声明分别保存，不替换成猜测的单位；如果后续响应出现未通过 TIFF 标签声明的 nil 样本，验证会失败。外部元数据链接一次连接超时，未将其特定数据许可标为已核实。

协议参考：[OGC WCS](https://www.ogc.org/standards/wcs/)、[KVP 扩展](https://docs.ogc.org/is/09-147r3/09-147r3.pdf)、[GeoTIFF 扩展](https://docs.ogc.org/is/12-100r1/12-100r1.pdf)。服务参考：[EMODnet 水深](https://emodnet.ec.europa.eu/en/bathymetry)、[官方 WCS 客户端](https://github.com/EMODnet/emodnet.wcs)。
