# GeoD Global — Research Sources & Evidence Boundaries
## 研究来源、事实边界与待验证事项

**资料核对日期：2026-09-20 · 版本：1.0**

本索引对应总纲、执行清单和英文模板中的 `[Rxx]`、`[Sxx]`、`[Pxx]`、`[Gxx]`。公开网页通过网页检索/正文读取核对；用户仓库通过已连接的 GitHub 工具读取。URL 以代码文本列出，便于复制和存档。

本次研究**没有**完成海外用户访谈、付费转化实验、数据源端到端联调、软件运行测试、支付商户审批或真实结算。因此，文档不把这些事情写成已验证的成果。没有使用第三方市场规模报告推导收入，也没有把搜索到的插件数量当作付费市场规模。

第三方规则会变化。公开上线、调用大规模数据、签约或收费前，应重新核对有关页面、实际接口以及适用于申请主体/用户账户的合同。

---

## A. 仓库与已讨论规划

### [R01] GeoD README — 当前产品声明

- 来源：`gaopengbin/geo-downloader`，默认分支 `README.md`。
- 地址：`https://github.com/gaopengbin/geo-downloader/blob/main/README.md`
- 本次读取的 blob SHA：`12dbebc37ec72958314df5e7c7457bdf2d832c94`。
- 可支持：README 所述免费开源、跨平台、OSM 矢量、授权瓦片导出、MVT、DEM、Wayback、3D Tiles、任务恢复及技术结构。
- 不可支持：所有功能在所有平台均经过实际测试；文档里列出的第三方服务允许所有下载行为；Global 目标设计已经实现。
- 应用：总纲的现状基线、复用范围、现有用户体验保护。

### [R02] GeoD LICENSE — MIT 许可文本

- 地址：`https://github.com/gaopengbin/geo-downloader/blob/main/LICENSE`
- blob SHA：`9ae1f58935aa5e2403fdd637659511ada9654c6e`。
- 可支持：该文件授予在保留相关声明等条件下使用、复制、修改、分发及出售副本等权限；当前代码的许可为 MIT。
- 不可支持：第三方影像、商标、所有依赖都使用 MIT；未来任何新增组件都已经决定采用某种许可证。
- 应用：不能把已经提供的 MIT 代码权利重新说成必须付费才有；收费应与确切的软件组件或额外服务对应。新增许可方案与贡献者权利仍需单独审查。

### [R03] 前端依赖 — Leaflet / MapLibre 现状

- 地址：`https://github.com/gaopengbin/geo-downloader/blob/main/frontend/package.json`
- blob SHA：`daac991804d77dd223a125e09cfda7ac07d393d6`。
- 可支持：声明了 Leaflet、MapLibre GL、二者桥接、React、Tauri API 等依赖；该清单没有列出 `ol`。
- 不可支持：每个地图页面具体运行路径、渲染性能、所有动态加载库；依赖存在也不等于某个功能已经完整交付。
- 应用：纠正此前把“当前二维引擎就是 OpenLayers”当现状的说法。OpenLayers 在本设计中是待验证的 Global 技术选型。

### [R04] Rust / Tauri / GDAL 依赖

- 地址：`https://github.com/gaopengbin/geo-downloader/blob/main/src-tauri/Cargo.toml`
- blob SHA：`9a3e47a4f695dcbb251a9b4dbdc4f0889cd09110`。
- 可支持：读取时包版本声明为 `3.6.10`；Tauri/Rust、SQLite、keyring 等依赖；GDAL 为 optional，默认 features 为空，`geotiff` feature 关联 GDAL。
- 不可支持：实际发布二进制开启了哪些 feature；已经具备完整栅格分析、重投影或所有平台 GDAL 分发能力。
- 应用：功能抽取、原生 worker 与依赖打包需实测，不能把可选依赖当完成的分析引擎。

### [R05] Issue #85 — Satellite Explorer 与统一工作台

- 地址：`https://github.com/gaopengbin/geo-downloader/issues/85`
- 标题：`feat(satellite): Satellite Explorer 与统一空间数据获取工作台规划`。
- 依据：此前本次对话中，已连接 GitHub 工具成功创建的返回记录；创建日期为 2026-09-15。
- 可支持：此前确实已把卫星模块与统一工作台的规划记录到仓库。
- 边界：本次文档制作没有重新读取该 Issue 的最新状态，也没有修改它或提交新评论。本文件不主张其当前状态、标签、进展与创建时完全相同。
- 应用：保留设计沿革；本次完整海外产品设计扩展其商业和终态范围，不自动替换既有 Issue。

---

## B. 数据来源与访问条件

### [S01] Element 84 Earth Search

- 产品页：`https://element84.com/earth-search/`
- 官方仓库说明：`https://github.com/Element84/earth-search/blob/main/README.md`
- 可支持：提供公开 STAC 检索入口，覆盖多种地理数据集合；其资料区分公开资产与需要 AWS 凭据的 Requester Pays 资产，并提醒公共接口没有保证服务级别。
- 不可支持：所有检索结果可匿名免费整景下载；所有资产永不收费；公共目录有付费级 SLA。
- 设计影响：Catalog、Asset、费用和授权独立建模；根据实际 collection、asset URL 和存储元数据选择访问路径。

### [S02] AWS Open Data — Sentinel-2 COG

- 地址：`https://registry.opendata.aws/sentinel-2-l2a-cogs/`
- 可支持：该公开数据条目描述 Sentinel-2 JPEG 2000 转换为 COG、STAC 元数据，以及部分公开 bucket 的匿名访问方式。
- 不可支持：Sentinel 全部级别/全部平台均以相同格式或认证方式提供；仅凭模型名推断当前可用 collection ID。
- 设计影响：可以评估 COG 按需读取和选择波段，但仍需检测 HTTP Range、格式、投影、数据版本与响应行为。

### [S03] USGS — Landsat 商业云访问

- 地址：`https://www.usgs.gov/landsat-missions/landsat-commercial-cloud-data-access`
- 可支持：USGS Landsat 的 AWS `usgs-landsat` bucket 采用 Requester Pays；相关 STAC 资产访问可能需要承担请求/传输费用。USGS 同时保留其他熟悉的免费数据下载方式。
- 不可支持：Landsat 数据许可收费；每种 USGS 下载路径都收费；目录查询免费就等于资产获取免费。
- 设计影响：UI 明确显示数据权利与基础设施费用是不同维度。不能替用户静默选择可能扣其云账户费用的路径。

### [S04] Copernicus Data Space — APIs

- 地址：`https://documentation.dataspace.copernicus.eu/APIs.html`
- 可支持：该平台提供多个 API/访问体系，包括 STAC、OData、S3、Sentinel Hub 和 openEO 等。
- 不可支持：所有 API 共用认证、全部免费无限、每种数据均可使用所有处理能力。
- 设计影响：Search、Preview、Download、Process、认证和配额分开声明；正式接入前按具体 endpoint 联调。

### 数据源仍需补充的验证

Microsoft Planetary Computer、NASA CMR、国家/地方开放数据、商业影像、更多 DEM 和三维服务是**完整设计的候选范围**，不因列入功能矩阵就视为已核验接口或已获得授权。对每个候选源补充：官方文档、collection/asset 样例、认证、网络、配额、费用、许可、传输、试验结果与负责人。

文档将 NAIP 归类为航空影像，避免把“影像数据目录”都叫卫星产品。不同高程产品需按官方定义区分 DSM/DTM、水平和垂直参考，不能统一假定为裸地高程。

---

## C. 现有替代产品与需求证据边界

### [S05] Copernicus Browser 官方文档

- 地址：`https://documentation.dataspace.copernicus.eu/Applications/Browser.html`
- 可支持：已有按地点、时间、数据等检索、元数据查看、可视化、比较、原始产品/部分文件下载与分析输出等能力。
- 不可支持：没有人需要独立桌面工具；所有用户觉得其工作流简单或复杂；GeoD 在任何维度已经优于它。
- 设计影响：单一“搜 Sentinel → 导出 TIFF”不是没有竞争的市场；需验证跨源、重复任务和本地交付差异。

### [S06] Planet — EO Browser / Dashboard 界面退役公告

- 地址：`https://community.planet.com/product-updates/important-update-sunset-of-eo-browser-and-sentinel-hub-dashboard-6426`
- 公告日期：2026-02-05；公告列出原界面于 2026-03-20 停止可访问。
- 可支持：Planet 引导用户使用 Planet Insights Platform 的 Browser；公共 Sentinel 工作流可转向 Copernicus Browser。公告/官方回复区分 Web UI 变更与 API。
- 不可支持：Sentinel Hub 的全部 API 已停用；可以继续把旧 EO Browser 作为当前独立入口而不解释迁移。
- 设计影响：竞争研究、SEO教程和迁移文案使用当前入口，不反复引用过时 UI。

### [S07] Planet — Browser 影像分析文档

- 地址：`https://docs.planet.com/platform/get-started/analyze-data/analyze-imagery-in-browser/`
- 页面列出的更新日期：2026-05-05。
- 可支持：Browser 的可视化、对比、下载、分析性导出和指数统计等功能。
- 不可支持：这些功能对所有用户和数据免费；任意商业影像权限自动随平台账号获得。
- 设计影响：竞争矩阵基于真实功能，不以产品名称差异制造不存在的能力缺口。

### [S08] QGIS Plugin — Sentinel 2 Image Downloader

- 地址：`https://plugins.qgis.org/plugins/sentinel_downloader/`
- 可支持：官方插件目录中的作者描述，包含 footprints、产品/波段获取、quicklook 和部分指数工作流。
- 不可支持：插件始终可用或我们已测试；下载计数等于活跃用户、付费用户或独立人数。
- 设计影响：免费替代已覆盖大量基础获取需求；明确差异与用户为何不用原有插件。

### [S09] QGIS Plugin — Optical Downloader

- 地址：`https://plugins.qgis.org/plugins/optical_downloader/`
- 可支持：目录描述从 Planetary Computer 获取 Sentinel/Landsat、区域与质量条件、裁剪和带掩膜 GeoTIFF 的工作流；页面也有实验版本提示。
- 不可支持：所有后端、数据和环境均无需认证且永远可用；GeoD 已完成同样的实现。
- 设计影响：仅靠“画区域导出遥感数据”不足以证明独特付费价值。

### [S10] QGIS Plugin — Sentinel Download

- 地址：`https://plugins.qgis.org/plugins/sentinel_download/`
- 可支持：作者列出的批量、AOI、选景策略、组合/指数产品、重试与恢复等能力。
- 不可支持：其功能和质量已由本次研究独立验证；存在供给就能直接推导付费需求规模。
- 设计影响：批量也已有免费替代，付费方案需更具体地验证托管协作、组织保障或服务交付。

### 这组来源到底证明了什么

它们是**现有供给与竞争行为的证据**，可以证明问题域已有工具和解决方案，不能代替一手需求访谈。文档中的目标客群、付费动机、获客效率和留存仍是待验证假设。主文档安排的 18 人访谈样本是研究计划，不是已经访问了 18 人。

---

## D. 数据使用与工程规范

### [S11] OpenStreetMap Foundation — Tile Usage Policy

- 地址：`https://operations.osmfoundation.org/policies/tiles/`
- 可支持：OSM 公共标准瓦片服务的使用限制，包括不用于批量预取、离线下载等；客户端标识、缓存与署名也有要求。
- 不可支持：OSM 原始数据不能下载；所有基于 OSM 数据的自建或商业服务都采用相同限制。
- 设计影响：公共标准底图预览与获授权离线数据源分开；前后端共同执行相应下载门禁。

### [S12] Google Maps Platform — Map Tiles API Policies

- 地址：`https://developers.google.com/maps/documentation/tile/policies`
- 可支持：瓦片服务具有独立的使用、缓存、提取和署名限制；不能将 API Key 或技术可访问性当作离线打包、再分发授权。
- 不可支持：任意特定客户合同的完整权利；可以忽略合同例外或适用服务条款作统一法律结论。
- 设计影响：不把 Google Photorealistic 3D Tiles 默认离线下载作为海外主推案例；只有明确许可支持的操作才能开放。

### [S13] Protomaps — PMTiles Documentation

- 地址：`https://docs.protomaps.com/pmtiles/`
- 可支持：PMTiles 是地图瓦片归档/分发格式及相关工具体系，可用于不同数据和托管场景。
- 不可支持：PMTiles 文件内的所有数据都是免费或可再分发；矢量归档可无损恢复原始测绘几何。
- 设计影响：格式能力、数据许可与提取/转换能力分别建模。

### [S14] OpenLayers — Cloud Optimized GeoTIFF 示例

- 地址：`https://openlayers.org/en/latest/examples/cog.html`
- 可支持：OpenLayers 的 GeoTIFF/COG 源与显示示例，可作为二维预览设计的技术参考。
- 不可支持：当前 GeoD 已使用 OpenLayers；所有遥感文件无需预处理即可流畅显示；前端预览等于生产级导出。
- 设计影响：以真实 COG、多源叠加、投影与内存样例验证 Viewer adapter。

### [S15] OpenLayers — NDVI / Band Math 示例

- 地址：`https://openlayers.org/en/latest/examples/cog-math.html`
- 可支持：在前端展示由波段表达式计算的可视化结果的实现方向。
- 不可支持：所有传感器可以忽略 scale/offset、掩膜和像元对齐使用同一公式；屏幕颜色等于科学数值。
- 设计影响：预览样式与分析输出分离，并对参考结果做数值回归。

### [S16] GDAL — COG Driver

- 地址：`https://gdal.org/en/stable/drivers/raster/cog.html`
- 可支持：GDAL 的 COG 输出驱动、布局、概览等选项与约束。
- 不可支持：任何 TIFF 改扩展名就是 COG；当前默认 GeoD 安装包已经包含这些能力。
- 设计影响：将格式合规与随机范围访问作为可验收项；打包 feature、原生库和实现需单独确认。

### [S17] Tauri — Updater Plugin

- 地址：`https://v2.tauri.app/plugin/updater/`
- 可支持：Tauri 更新分发和签名校验的文档接口及要求。
- 不可支持：Updater 签名自动代替 Windows 代码签名或 Apple Developer ID/notarization。
- 设计影响：更新包验证与操作系统发行信任分开验收；密钥不得进入仓库或渲染层。

### [S18] W3C — WCAG 2.2

- 地址：`https://www.w3.org/TR/WCAG22/`
- 可支持：无障碍设计与检查的规范参考，例如对比度、键盘、焦点、目标尺寸及交互可理解性。
- 不可支持：采用某组字体/颜色后自动符合 WCAG；本次已经完成无障碍审计或取得认证。
- 设计影响：把无障碍转化为测试用例，不以“海外审美”作为小字号和低对比的理由。

---

## E. 海外支付、审核与收款

### [P01] Paddle — Supported Countries

- 地址：`https://www.paddle.com/help/legal/sanctions/which-countries-are-supported-by-paddle`
- 可支持：Paddle 对软件卖家地区与受限制地区的公开说明。此次读取的排除列表未列中国大陆。
- 不可支持：任何中国个人/公司都一定获批；银行、商品、风险或文件审核不再必要。
- 设计影响：列入条件性候选，先确认实际主体、商品形态和结算账户。

### [P02] Paddle — Identity Verification / Business Identification

- 地址：`https://www.paddle.com/help/start/account-verification/what-is-identity-verification`
- 补充：`https://www.paddle.com/help/start/account-verification/what-is-business-verification`
- 可支持：个人/sole trader 与公司申请者在相应流程中有不同的身份/业务核验要求，可能要求身份证明、地址证明等。
- 不可支持：国籍、实际居住地、经营主体和收款账户任意组合都可通过；使用虚构海外资料属于允许方案。
- 设计影响：只按真实身份与业务申请；在安全的官方流程中提交材料。

### [P03] Paddle — Acceptable Use Policy

- 地址：`https://www.paddle.com/help/start/intro-to-paddle/what-am-i-not-allowed-to-sell-on-paddle`
- 可支持：平台以软件公司/软件产品为中心；主要是人工服务的业务不适合；与软件无关的纯咨询等，以及缺少真实软件或服务商品的赞助、捐赠等列有约束。
- 不可支持：所有软件相关支持都绝对禁止；凡写成“软件套餐”的咨询就自动被允许。
- 设计影响：真实云软件订阅、软件相关服务、独立人工咨询、赞助分开描述和审核，不能共用一个虚构 SaaS 商品来收款。
- 与本项目特别相关：AUP 还限制侵害第三方版权/使用条款及未经授权数据访问。审核应覆盖实际软件功能和公开宣传，不只是付款页上的商品名称。

### [P04] Paddle — Pricing

- 地址：`https://www.paddle.com/pricing`
- 可支持：公开标准方案列出 `5% + $0.50` 交易收费，具体方案或条件可能不同。
- 不可支持：这等于所有卖家的最终总成本；无需考虑税基、汇兑、银行、退款或其他合同条件。
- 设计影响：单位经济模型仅按明确假设计算，并在真实账单出现后替换。

### [P05] Paddle — Local Currency Payout

- 地址：`https://www.paddle.com/help/manage/get-paid/can-i-be-paid-in-my-local-currency`
- 可支持：结算货币资料包括人民币选项，并涉及可能的转换费用等事项。
- 不可支持：支持人民币结算等于已接受中国个人；任意银行或个人账户均受支持；可以据此省略 KYC。
- 设计影响：币种、银行路径、账户名称、申请主体、最小结算额、手续费和到账周期分别确认。

### [P06] Paddle — VAT / Merchant of Record

- 地址：`https://www.paddle.com/help/sell/tax/how-paddle-handles-vat-on-your-behalf`
- 可支持：Paddle 作为 merchant of record 在其交易范围内处理相关买家间接税事项。
- 不可支持：卖家不需要记账、申报收入、承担本地税务义务或遵守隐私/数据许可规则。
- 设计影响：结算单、收入、费用、退款和税务资料留档，实际处理由适格专业人士结合主体所在地确认。

### [P07] Paddle — Webhook Signature Verification

- 地址：`https://developer.paddle.com/webhooks/about/signature-verification/`
- 可支持：服务端验证 webhook 签名及原始请求体等技术要求。
- 不可支持：浏览器成功跳转是可信支付证明；验签本身解决重复、乱序、退款和所有对账问题。
- 设计影响：验签只是入口；事件去重、状态机、权限同步、退款、人工补偿和账本对账是本方案另行设计的职责。

### [P08] Lemon Squeezy — Supported Countries

- 地址：`https://docs.lemonsqueezy.com/help/getting-started/supported-countries`
- 可支持：其银行结算国家列表和 PayPal 结算说明；此次读取的银行列表未见中国大陆，但包含香港等地区。
- 不可支持：PayPal 覆盖某国家就等于该申请人的商户和提现资格已通过；买家付款支持等于卖家准入。
- 设计影响：作为备选，先用真实主体书面确认可行结算路径，而不是先接入再发现无法提现。

### [P09] Lemon Squeezy — Fees

- 地址：`https://docs.lemonsqueezy.com/help/getting-started/fees`
- 可支持：基础费率之外，支付方式、跨境、订阅和部分结算场景可能另有费用。
- 不可支持：只比较 `5% + $0.50` 就能代表两家平台最终净收入相同。
- 设计影响：比较费用需按实际商品、买家分布、支付方式、退款和 payout 路径计算。

### [P10] Lemon Squeezy — Getting Paid

- 地址：`https://docs.lemonsqueezy.com/help/getting-started/getting-paid`
- 可支持：银行/PayPal 结算设置、验证和结算节奏等公开说明。
- 不可支持：所有申请者拥有同样到账币种/周期；测试支付成功等于真实结算已完成。
- 设计影响：把“商户审批通过、真实订单、退款、首笔到账”列为不同门禁。

### [P11] Stripe — Global Availability

- 地址：`https://stripe.com/global`
- 可支持：Stripe Payments 商户支持地区列表，此次读取中中国大陆不在直接支持列表；其他业务产品的可用性与 Payments 不可混为一谈。
- 不可支持：人在某个国家或能打开网页就拥有当地商户资格；Atlas 自动免除实际经营、银行、报税与持续维护义务。
- 设计影响：当前不把 Stripe 作为尚未确认主体条件下的唯一依赖；若未来有真实合适主体，再评估。

### 支付候选的统一边界

这份设计没有断言用户已经拥有公司、海外公司、特定银行、PayPal 收款权限或已获商户批准。Paddle 是对**符合其软件商品定义、且实际主体能通过审核**的条件性候选，不是保证可收款的结论。

未列入某条名单、可以注册、可以开 sandbox、可以创建订单、可以买家付款、可以提现，是不同状态。主文档用门禁将其逐项验证。实际经营和税务处理不以本设计代替正式专业意见。

---

## F. 推广与社区规范

### [G01] Hacker News — Show HN Guidelines

- 地址：`https://news.ycombinator.com/showhn.html`
- 可支持：Show HN 面向能实际试用的作品；不适合只有 landing page 或候补名单的展示；不能组织投票请求等。
- 不可支持：一定进入首页、一定带来高价值用户；可用实验性概念稿冒充已发布软件。
- 设计影响：发布前提供真实可试版本、源码/下载、具体示例、限制与作者身份。

### [G02] Reddit — Spam Policy

- 地址：`https://support.reddithelp.com/hc/en-us/articles/360043504051-Spam`
- 可支持：通用反垃圾规范；重复、未经请求或操纵性内容存在风险。
- 不可支持：满足通用规则就可以在所有 subreddit 自我推广；其他社区可忽略其自身规则。
- 设计影响：逐个社区核对，先提供有用技术内容、披露作者关系，不批量贴相同文案或购买投票。

---

## G. 仍未验证、不能变成事实的数字与判断

| 项目 | 本文件的处理 |
|---|---|
| 海外付费用户数量 / TAM / 市场份额 | 未调查，不给估算事实 |
| 搜索词月流量、难度与 SEO 收益 | 仅提供意图选题，不伪造流量 |
| 官网转化率、激活率、付费率、留存 | 定义测量方式；目标只是内部假设 |
| $99 / $399 等价格 | 研究与包装提案，不是已确认成交价格 |
| 商业云服务存在与支持容量 | 目标设计，尚未开通或实测 |
| Paddle 等审核与银行到账 | 尚未申请/确认；应完成真实审核及结算验证 |
| GeoD 所有现有能力是否可靠 | 仓库声明与依赖已读取，运行表现未测 |
| OpenLayers/GDAL 性能指标 | 待建立测试基准，不从示例页外推 |
| Provider 免费/许可/商业使用范围 | 按资产、端点和真实条款分别确认 |
| 用户都偏爱某种“海外审美” | 设计假设，需实际任务与可用性验证 |
| 所谓社区需求已经足以支持商业化 | 无此结论；免费替代存在，需验证额外价值 |

## H. 后续补证据的统一模板

```text
Evidence ID:
Claim to verify:
Source owner and URL:
Retrieved / tested at:
Product / API / plan version:
Relevant applicant / account / geography:
Observed result:
What this evidence does NOT prove:
Raw sample / redacted log / screenshot location:
License / quotation / privacy restrictions:
Decision affected:
Next review trigger:
```

**资料使用原则：来源证明到哪里，结论就写到哪里。设计可以完整，事实不能用推测补齐。**
