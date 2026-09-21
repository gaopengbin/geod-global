# GeoD Global — 原型与本轮验收

**日期：2026-09-21 · 状态：设计原型已构建并检查代表性交互；完整产品尚未实现。**

本轮交付把产品设计落实为可打开的页面、代码复用清单、契约草案和实施依赖。保留六大数据域与商业运营的完整目标，不以当前原型实现范围替代产品终态。

**记录边界**：本文件第1–6节保留独立建仓前的原型交付与检查结果。随后用户认可原型方向，并确认在 `G:\code\geod-global` 建独立仓库。新的仓库/依赖隔离决定与本轮工程检查见[附件08](08-Repository-Boundary.md)，不能用下表历史构建结果代替独立构建验收。

## 1. 交付入口

| 内容 | 位置 | 作用 |
|---|---|---|
| 原型使用说明 | [prototype/README.md](../prototype/README.md) | 启动方式、交互范围与数据出处 |
| 当前预览 | <http://127.0.0.1:4317/> | 本机服务运行期间可打开 |
| 原型源码 | [main.jsx](../prototype/src/main.jsx)、[styles.css](../prototype/src/styles.css) | 与原桌面代码隔离的 React 原型 |
| 代码基线 | [05](05-Code-Baseline-and-Implementation-Map.md) | 真实源码与全部工作包的复用/改造/新增映射 |
| 决策及契约 | [06](06-Decisions-Contracts-and-Release-Gates.md) | 决策归属、接口字段、终态归约、发布门禁 |
| 契约校验结果 | [verification-result.json](contracts/verification-result.json) | 拟议 schema 与 fixture 的通过/拒绝证据 |

桌面截图：[浅色](../prototype/qa/workspace-light-1440.png)、[深色](../prototype/qa/workspace-dark-1440.png)、[导出审阅](../prototype/qa/export-review-1440.png)；[390px窄屏](../prototype/qa/mobile-390.png)。

没有修改现有 `tif-downloader` 应用代码、提交 Git、创建 PR、部署、联系用户、申请商户或启用收费。

## 2. 原型设计选择

主题为本地数据工作台，用户先选择区域与数据，再审阅成果。主视觉由真实影像与观测时间轴构成；采用窄导航、左侧发现、中间预览、右侧详情。面板共享同一份区域与场景选择，不在数据域之间重新建立项目。

颜色沿用总纲蓝色品牌：背景 `#F6F7F9`、表面 `#FFFFFF`、文字 `#17212B`、边界 `#D8DEE7`、强调 `#2563EB`；深色为 `#14181F` / `#1C222B` / `#E8EDF3` / `#394350` / `#7AA7FF`。Inter 本地加载，坐标/ID 使用系统等宽字体。低层状态信息使用较紧凑字号，后续完整可访问性审计仍需覆盖对比度、200%缩放与各系统 WebView。

用户随后反馈“看起来不错”，因此保留这套结构作为后续视觉基线；这不代表完整视觉/可访问性验收，也不代表 OpenLayers 已选型。原型使用浏览器 SVG 容器显示真实 JPEG 概览，未引入或宣称接入专业地图引擎。

## 3. 页面覆盖与剩余深度

| 页面 | 本轮可操作 | 后续仍需实现 |
|---|---|---|
| Explore | 实际场景快照、本地筛选、日期、云量、选景、空/加载/失败状态 | 真实 Provider 查询、分页、认证与实时覆盖判断 |
| Workspace | 统一区域和选中对象、预览、比较、缩放、时间轴 | 独立多图层编辑器、可绘制 AOI、多投影渲染、科学可视化 |
| 六个数据分类 | 导航、固定区域共享、计划能力提示；本地文件名/大小读取代码 | 其余领域真实数据获取、处理和 3D renderer |
| Export | 格式/CRS审阅、JSON计划文本、任务模拟入口 | 真实预算、许可、磁盘检查、原生计算及成果验证 |
| Tasks | 模拟暂停/恢复/失败/新尝试重试、完成报告 | 持久化 Core Job、进程恢复和真实故障注入 |
| My Data | 样例报告、来源缩略图、报告文本 | 真正的成果索引、移动文件检测与独立格式验证 |
| Recipes | 本地保存、刷新保留、固定场景、JSON审阅 | 正式契约迁移、导入验证、批量区域与执行 |
| Sources | 来源目录、快照启用开关、计划状态 | Provider注册、凭据库、能力/授权策略 |
| Cloud | 信息边界与协作价值提案 | 商业研究、账户、组织权限、同步、退出与付款 |
| Settings | 明暗切换、localStorage记录、数据清理入口 | 原生设置、系统主题、迁移和网络配置 |

本轮没有声称已覆盖全量页面的所有异常状态。当前材料足以审阅产品方向和代表性链路，WP-D03 的全部状态设计仍需逐项补齐。

## 4. 真实样本与数据边界

七条 Sentinel-2 L2A 场景来自一次实际 Earth Search 查询；采集日期在2025年6月，获取时间见 manifest。原 JPEG 与 SHA-256 被保留。来源与技术网格详见 [manifest](../prototype/public/samples/manifest.json) 和 [原始响应](../prototype/public/samples/earth-search-response.json)。

参考：[Earth Search 检索](https://earth-search.aws.element84.com/v1/search?collections=sentinel-2-l2a&bbox=-122.55,37.68,-122.32,37.84&datetime=2025-06-01T00:00:00Z/2025-06-30T23:59:59Z&limit=8)、[AWS 数据集登记](https://registry.opendata.aws/sentinel-2-l2a-cogs/)。样本不是全球目录完整结果，数据分类中的未接入项不是搜索成功记录。

343×343 缩略图不包含可导出的原始多光谱科学数值。原型没有生成 COG/GeoTIFF。选择的10米分辨率描述原始RGB资产和目标输出意图，不描述缩略图。

原型计划格式 `design-prototype/v1` 与06的 `0.1.0-proposed.1` 分开。进入真实Core前需要：区域变成可引用且不可变的Area；输入变成包含asset版本的Item引用；转换输出required/outputPolicy、预算和授权策略；处理操作接入真实算子；Job与Artifact采用验证后提交语义。

## 5. 本轮已执行的检查

| 检查 | 结果与边界 |
|---|---|
| Vite生产构建（历史原型阶段） | 通过；当时借用国内版已安装依赖，输出 `prototype/dist/`。该临时方式已被独立仓库决定禁止，后续以08的独立安装/构建记录为准 |
| 现有前端单测 | 34/34通过；由代码审计执行，不代表桌面运行验收 |
| 现有MCP单测 | 6/6通过；本地服务测试，不代表数据源端到端 |
| 拟议契约 | 4条基础对象；6个正例通过、14个负例被拒绝；全为fixture |
| 样本文件 | 七张JPEG的SHA-256与manifest一致 |
| 筛选 | `2025-06-22`得到对应场景；云量设0后为空，Reset filters恢复 |
| 比较 | 分割百分比与视口clip-path一致；缩放后不再使用固定SVG坐标裁剪 |
| 配方持久化 | 保存 `Bay Area review recipe`，刷新Recipes仍可读取 |
| 导出审阅 | 可打开参数窗口，JSON文本可解析且包含design-only与implemented:false |
| 模拟Job | Running→Paused→Running→Failed；Retry创建新尝试；完成后成果库出现Simulation report |
| 来源异常 | Sample states→Source error；Retry sample恢复；未当作真实远端故障 |
| 路由 | Cloud/Settings刷新仍停在对应页面，未回退Explore |
| 布局 | 1440×900、1280×900、1000×800与390×844；无文档级横向溢出；1000px详情浮层可关闭 |
| 字体/主题 | Inter确实加载；明暗主题切换有效 |
| 浏览器日志 | 所检查运行路径未发现error/warn |

浏览器使用Codex内置浏览器。截图既用于视觉检查，也附带DOM和实际点击记录。未执行系统级200%缩放、完整屏幕阅读器审计、三系统安装/升级或真实外部支付测试。

**下载限制**：内置浏览器对blob文件下载的等待事件超时，未确认下载落盘。因此增加完整JSON文本预览，用户可查看/复制，并在普通浏览器尝试保存。不能把代码中存在download调用写成已验证下载成功。

## 6. 修复过的可观察问题

1. 比较图像使用固定1000坐标裁剪，与屏幕分割条错位：改为相同视口叠图及CSS百分比裁剪。
2. 窄屏详情浮层缺少关闭入口：增加详情栏内关闭按钮，小窗口初始收起。
3. Cloud与Settings深链接刷新被误判：补入初始化路由集合。
4. 原型底部操作随详情滚动离开：改为详情栏内固定操作区。
5. 内置浏览器下载没有完成证据：提供可检查的完整JSON文本，不把其显示为下载成功。

## 7. 下一批可执行工作

先按08在 Global 本仓建立独立依赖、锁文件、构建与检查，解除原型对国内 `node_modules` 的借用；05记录的国内MCP/OSM工作树保持原位，不复制、不迁移。随后将06契约落实到 Global Core/前端类型，补Item、Provider、算子图与版本迁移；实现Earth Search只读adapter，接通真实搜索、来源详情与缩略图。并行确定Viewer对照样例和原生科学栅格打包方案。

完整产品后续工作仍包括六数据域、真实执行/恢复、成果验证、配方/CLI/MCP、全状态UI、跨平台发行、研究与推广、待选商业方案；具体依赖见05/06，不能以完成这一轮原型作为产品交付完成。
