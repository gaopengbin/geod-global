# 本地 Shapefile 文件

2026-10-03。「我的数据 → 矢量文件」现在支持本地 Shapefile 的引用、明确复制、多图层地图、原属性检查、GeoJSON 导出与原始文件组导出。与 [GeoPackage](geopackage-local.md) 共用本地登记和地图，不需要设置在线服务或账号。

## 打开与保存

桌面文件选择器可以选择 `.shp` 或 `.zip`，默认引用原文件，也可明确选择复制到 GeoD 数据目录。打开 `.shp` 时读取同目录、同名的 `.shp/.shx/.dbf/.prj`，以及存在的 `.cpg/.qix/.sbn/.sbx/.fix/.shp.xml`；不会收集文件夹内其他文件。缺少必需伴随文件会报告具体图层和扩展名。浏览器开发适配只接受 ZIP 文件内容，不接收任意本机路径；浏览器上传是受管复制。

两种原文件导出有不同含义：

- 打开 ZIP：完整归档原样保存、导出，不改写压缩包；内部 README、版本说明等普通成员也保留。
- 打开 SHP：各个已收集伴随文件的字节保持不变，以可重复生成的 ZIP 组织导出。这个 ZIP 是 GeoD 的文件组容器，不冒充供应商原包。

每个原始成员和完整归档均记录 SHA-256；登记同时保存转换版本、每图层坐标方法和属性编码。检查与导出前重新读取、核对原文件和转换结果。引用文件移走或任何成员变化会报错，不能把旧地图缓存当成可用原件。受管副本在输入被移走后仍可离线使用。移除登记不删除原件，导出目标不能覆盖任何已登记的引用成员。

## 几何、属性与坐标

支持 Point、PolyLine、Polygon、MultiPoint 的 XY、Z 和 M 共 12 种类型（1/3/5/8、11/13/15/18、21/23/25/28），以及这些图层中的 Null Shape 记录。SHX 偏移、长度、编号和 DBF 数量须与 SHP 一致。MultiPatch、头部类型为 Null 的整层，以及其他专用类型尚不支持。[Esri 格式说明](https://www.esri.com/library/whitepapers/pdfs/shapefile.pdf)

外环按源 XY 的顺时针方向识别，孔洞归入最小包含外环，环的存储顺序可以不同。源顶点顺序、重复相邻 XY 所附的 Z/M 均保留；转换的 GeoJSON 也保留该顺序，当前不执行 RFC 7946 环方向规范化。自交、自触、交叠、未闭合和不能归属的孔洞会拒绝，错误定位到图层和从 1 起算的记录。为避免未验证的内部连通性，孔洞边界与其他环相接的情形明确列为不支持；不自动修复拓扑。

普通 dBASE III 的 C/N/F/L/D 字段可读取；memo、二进制和不明确的字段布局会拒绝。C 字段保留前导空格、去掉尾部填充；L 是布尔值或 null，D 是 ISO 日期或 null。**所有 N/F 字段以原文十进制字符串输出**，保留超大整数、精确小数及指数写法；不在浏览器中先转换为浮点数。空白 / 全星号数值输出 null，原 DBF 字节仍保留。

CPG 优先于 DBF language driver。明确 CPG 支持 UTF-8、GBK 和所用解码库提供的兼容单字节 / 多字节编码；UTF-16 和替代字符解码不接受。无 CPG 时只采用已实现的 LDID 映射（03/57/C9/7A/78/7B/79）；未知映射只能读取 ASCII，非 ASCII 字符需要来源提供明确 CPG。0x57 的 ISO-8859-1 解释遵循 GDAL，来源有其他编码时应明确提供 CPG。[GDAL Shapefile 编码说明](https://gdal.org/en/stable/drivers/vector/shapefile.html)

删除标记记录保留属性和几何，并添加 `geodDeleted: true`，地图排除这些记录；Null Shape 保留 `geometry: null`。要素编号是各源图层从 0 起算的记录位置，因此删除 / 空几何不会使后面的编号改变。M 保存为独立 `geodMeasures`，未提供的整块 M 和小于 `-1e38` 的 NoData 使用 null，Z 原值保留。

必须提供可识别的 PRJ WKT，不根据数值猜坐标系。地图和转换导出使用 EPSG:4326；坐标方法与 [GeoPackage 的水平转换](geopackage-local.md) 相同：每图层固定一个实际记录的基准操作，显示声明精度和适用范围，报告超出范围的坐标。近似未知基准转换、自动下载格网和垂直转换不执行；Z 不能据此宣称为 WGS84 高程。世界边界外仅 `1e-9` 度以内的数值舍入可夹回并计数，其他越界报错。

## 范围限制

压缩输入、解压后成员总和与转换结果各限 20 MiB；每包最多 32 层、50,000 条记录、500,000 个坐标、256 个 ZIP 成员，每层 128 字段。记录上限包含删除和空几何。多边形每要素至多 4,096 环；每包拓扑工作预算为 50,000,000，按各多边形两倍顶点数量平方累计，在进入环内与环间检查前拒绝超额工作。这个限制可能先于坐标上限触发。

ZIP 只读取 stored / deflate 的普通成员，检查解压长度和 CRC，不解包到输入目录。路径越界、链接、加密、不支持的压缩和大小写歧义拒绝。同目录伴随文件扫描有界，超过 10,000 项的目录应先将数据移至独立文件夹。

该增量不包含编辑、空间分析、工程内处理、MCP、PBF/XML、投影格网、自动拓扑修复或无限量读取。安装版原生选择器与 WebView 人工验收仍是独立步骤。

## 实际验收

使用 Natural Earth 公开的 [110m Lakes v5.0.0](https://www.naturalearthdata.com/downloads/110m-physical-vectors/110mlakes-reservoirs/) 原 ZIP，23,622 字节，SHA-256 `f2eed3c738a93010770acb0ba44273ea6a83b053641588bc902d9d6fd1cdafcb`。其 24 个要素、465 个坐标、888 项属性、全部 7 个原始成员经独立 PyShp / GDAL / PROJ 核对；930 个数值坐标分量差异为 0，原 ZIP 导出逐字节相同。数据采用 [Natural Earth 公共领域条款](https://www.naturalearthdata.com/about/terms-of-use/)，实际文件仅留在隔离验收目录，不随应用分发。

另外保留 [独立控制文件及来源记录](../crates/geod-runtime/fixtures/shapefile/independent-pyshp.SOURCE.json)，由 PyShp 3.1.6 和 GDAL 3.9.2 生成，不是供应商数据。12 个图层覆盖 WGS84 / ESRI Web Mercator / UTM 33N、所有支持几何类型、中文 UTF-8 / GBK、超大整数、精确小数、删除、空几何、Z/M 与乱序孔洞。全部 14 条记录、84 项属性、225 个数值分量核对，最大水平坐标差 `2.67e-14` 度；原始成员全部相同。

公开的 `ne_110m_land` v4.0.0 原包另作为**失败案例**：69,700 字节，SHA-256 `1926c621afd6ac67c3f36639bb1236134a48d82226dc675d3e3df53d02d2a3de`。独立 GDAL 确认记录 79 有自相交（位置约 -132.71000788443121, 54.040009315423447），软件明确拒绝并定位该记录；没有修复或替换样本后宣称原包验收通过。

真实隔离运行时完成 12 组 ZIP / 伴随文件、引用 / 受管复制、转换与原件导出的检查。输入移走、代理指向拒绝端口、实际停止并重启后，9 个受管副本完整核对通过，6 个失效引用拒绝。1440 英文浅色与 1024 中文深色的无头浏览器检查了真实地图、属性与 M 值、图层切换、面板拖动、两种完整导出和错误定位；没有操作用户桌面，也不据此宣称原生 WebView 人工验收完成。机器记录见 [Shapefile 验收](../prototype/qa/shapefile-local-verification.json)。

独立重跑：

```powershell
python -X utf8 scripts/verify-shapefile-local.py --source SOURCE.zip --converted EXPORTED.geojson --original ORIGINAL.zip --registry DATA/vectors.json --id UUID --report REPORT.json
python -X utf8 scripts/generate-shapefile-fixture.py --out NEW-CONTROL.zip
```

`--source` 也可传同目录有完整伴随文件的 SHP。生成器拒绝覆盖已有 ZIP；QA 的 PyShp / GDAL 不进入应用运行依赖。CLI 使用现有 `vectors open --file FILE --data-dir DIR`，显式 `--mode managed` 复制；导出加 `--format original` 保存 ZIP，默认导出转换 GeoJSON。
