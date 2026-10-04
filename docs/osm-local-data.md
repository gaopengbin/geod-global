# 本地 OSM XML / PBF 原文件

2026-10-03。桌面“我的数据 → 矢量文件”现可打开 OSM 0.6 XML（`.osm` / `.xml`）及 PBF 快照。默认引用原文件；明确选择“复制到 GeoD 存储”才创建受管副本。开发浏览器的文件上传始终创建副本，不接受本地路径。该入口与[用户配置的 Overpass 在线提取](osm-overpass.md)分开；原文件不会被伪装成 Overpass 查询响应。

地图可以筛选节点、路径和关系，点选查看标签、成员引用和编辑元数据。来源详情记录文件生成工具、原文件格式、对象数、文件声明范围、PBF 必需格式特征及有记录的同步信息。文件未记录数据时间时显示“文件中未记录”，不以下载或打开时间替代。OpenStreetMap 署名和 ODbL 链接保留。

“导出 GeoJSON”保存全部对象的 WGS84 几何和属性，筛选显示不删减导出。另一个带提示的图标“导出原始 OSM 文件”保存与登记时逐字节相同的 XML / PBF。原文件 SHA-256 与转换内容 SHA-256 分开保存。读取、原文件导出和重启后检查均重新核对原文件及转换记录；引用缺失或内容变化会报错，不偷偷改为副本。

## 格式与边界

XML 支持节点、路径、关系、字符串标签、正的安全整数 ID、成员角色、节点引用以及 version / timestamp / changeset / uid / user / visible 元数据。UTF-8、标准 XML 字符转义和 UTF-8 BOM 可读取。只接受 0.6 快照；不执行 DTD / 外部实体，也不猜测编辑器 action、OSMChange、历史版本、删除对象、Overpass 扩展 XML 或未知对象属性。

PBF 支持未压缩 / zlib 数据块、普通 / DenseNodes 节点、差分 ID / 坐标 / 元数据 / 引用、字符串表、非默认坐标和日期粒度 / 偏移，以及声明 `LocationsOnWays` 的内嵌路径坐标。校验必需特征、数组数量、字符串索引、解压后字节数、完整数据块、整数溢出和坐标范围。未知必需特征（含 HistoricalInformation）整份拒绝。未知可选 protobuf 字段保留在原文件，不进入转换属性；未知扩展文件块记录为未绘制数据块。重复单值字段、混合类型 PrimitiveGroup、changeset 数据块、非支持的 wire type、lzma / bzip2 / lz4 / zstd 压缩等不接受。

新文件复用既有 OSM 几何转换：完整节点引用、分段 / 反向 outer 环、inner 孔洞及嵌套普通关系；道路闭环依标签保持线或转面。每个原始对象都保留，包括无标签依赖节点。缺失引用明确指出拥有者和缺失对象，不填补、跳过或自动修复边界。一般关系不是完整 OSM 路由 / 网络分析引擎。转换不会调整测量基准或验证所有 OGC 面拓扑。

当前为有界本地工作流：原文件及 PBF 全部已解压数据块合计分别不超过 20 MiB；转换 JSON 不超过 20 MiB；50,000 个对象；500,000 个坐标 / 引用；单关系最多 1,024 个成员；1,024 个文件块；XML 最多 1,000,000 个解析节点；来源元数据不超过 64 KiB。PBF 数组在分配时有数量上限，zlib 在读取时有解压上限。不是国家级 / 全球 PBF、增量合并、gzip XML、流式索引、任意压缩、复杂不完整提取或自动下载服务。

## 验收证据

[机器记录](../prototype/qa/osm-local-verification.json)区分独立控制、可追溯地理子集、公开原文件拒绝和网络未验收。

1. `b-r-u/osmpbf` 固定提交的原始 XML、raw / zlib、dense / non-dense、LocationsOnWays 和历史 PBF 控制均逐字节留存，来源与 MIT 通知在[控制目录](../crates/geod-runtime/fixtures/osm/SOURCE.json)。原始 upstream fixture 是合成控制，不冒充实时提供商数据。
2. [自有独立控制](../crates/geod-runtime/fixtures/osm/INDEPENDENT-SOURCE.json)由 Pyosmium 4.3.1 写出 XML / PBF，含 UTF-8、反向分段外环、孔洞、嵌套关系及编辑元数据。原生解析后由独立 Pyosmium 再读取原件，核对全部 ID、标签、成员、元数据、坐标和环。生成器[不覆盖已冻结文件](../scripts/generate-osm-control.py)。
3. 下载到的公开 Seatac 地理 PBF 原件（181,036 B）SHA-256 `c1f8b9d2b25f6dcef2482355d4aaeea48d897a16c1e8e8908725cfccecc23092` 被拒绝：relation/2317217 超过 1,024 成员。独立读取确认原包有 13,217 节点、1,938 路径、77 关系和 5,676 个缺失路径引用。另一个 OSM-binary 公开原样本（9,653 B）被拒绝：relation/21855 缺少 way/156255507；独立读取确认 232 个缺失路径引用。拒绝不是成功的区域原文件验收。
4. 从公开 Seatac 原件明确生成[完整地理子集](../crates/geod-runtime/fixtures/osm/SEATAC-SOURCE.json)：保留 263 节点及 10 个原建筑路径 ID、全部标签和元数据；Pyosmium 独立写出 XML / PBF。该子集不是未修改的提供商原文件，也不证明当前 OSM API / Geofabrik 请求成功。其全部 273 对象通过原生导入、独立属性 / 几何比对和原文件完整导出。
5. 共 18 组 reference / managed 原生导入和独立比对通过。实际停止、重启服务并将测试输入移出登记位置后，30 个受管副本全部再次独立核对，24 个缺失引用正确报错。隔离运行时使用不可用代理 `127.0.0.1:9`，恢复不需要外网。更多副本来自可重试测试和界面上传，计数不代表独立数据集数量。
6. 实际原生 API 支撑的 headless 界面在 1440 浅色英文和 1024 深色中文下验证 XML 上传、历史文件拒绝、对象筛选、孔洞 / 建筑地图、点击属性、原文件与完整 GeoJSON 下载及来源详情；无页面异常、横向溢出或外网请求。截图已检查。没有操作用户桌面；原生 WebView 人工验收仍待完成。

直连当前 OSM API 与 Geofabrik 的请求超时，未计作下载成功。大文件 / 不完整区域导入、工程内矢量处理、栅格叠加、认证服务以及完整发布验收仍在[全量清单](provider-integration-status.md)中。

技术依据：[OSM XML](https://wiki.openstreetmap.org/wiki/OSM_XML)、[PBF 格式](https://wiki.openstreetmap.org/wiki/PBF_Format)、[Pyosmium](https://docs.osmcode.org/pyosmium/latest/)、[数据署名及许可](https://www.openstreetmap.org/copyright)。应用内解析器为本仓库实现，未加入其他 PBF 运行时依赖；Pyosmium 只用于独立开发验证。
