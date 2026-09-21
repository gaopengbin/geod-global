# GeoD Global — English Copy & Outreach Kit
## 官网、需求访谈、社区发布、合作与支付审核英文素材

**版本：1.0 · 日期：2026-09-20 · 用途：内部审稿与后续执行**

本文件不是已经发布的文案，也不代表已经联系用户、申请商户或确认产品能力。方括号内为填写项；涉及未实现能力的段落只有在功能验收通过后才能公开使用。真实联系方式、主体名称、网址、价格与条款必须由负责人确认。

特别注意：**GeoD 当前仓库已有能力、GeoD Global 的目标设计、拟议中的付费云服务，是三种不同状态。** 不用同一段现在时营销文案将其混为一谈。技术、支付和竞争背景的证据见 `04-Research-Sources.md`。

---

## 1. 英文品牌与用词规范

### 1.1 产品名称

对外主名称建议保留 **GeoD**。`GeoD Global` 可作为设计和发行配置名称，不必让用户理解国内版/海外版的内部关系。商标、域名和软件商店名称在投入购买或推广前另行查验。

**Category**

> A local-first geospatial data workspace.

**Target positioning — proposed, not a current feature claim**

> Find the data. Keep the workflow.

**Supporting sentence**

> Discover, preview and prepare geospatial data for your next project, with reusable workflows and files you control.

避免声称“the first”“the only”“unlimited”“all satellite data”“100% private”“zero cloud”或“commercial use always allowed”。公开数据检索仍会向相应服务发送区域和查询条件；不同资产也可能需要账号、配额或支付。[S01–S04][S11][S12]

### 1.2 稳定的界面词汇

| 用途 | 默认英文 | 解释/约束 |
|---|---|---|
| 研究或下载范围 | Area | 专业详情可标 Area of interest (AOI) |
| 一类数据集合 | Dataset | 不把一个临时地图图层都叫 Dataset |
| 一次观测场景 | Scene | 只用于对应数据模型，不用于所有底图 |
| 当前显示对象 | Layer | 和可下载产品分开 |
| 可重复操作描述 | Recipe | 不自动含执行权限、账号凭据或数据文件 |
| 异步工作 | Task | 详情可显示稳定 Job ID |
| 输出结果 | Output | 资产库中的成果可称 Saved dataset |
| 接入方 | Source / Provider | Source 面向用户，Provider 用在开发文档 |
| 下载真实文件 | Download data | 不能用地图截图冒充 |
| 导出显示效果 | Export visualization | 明确与原始值不同 |
| 导出分析数值 | Export analytical data | 保留波段/指数、掩膜、CRS等信息 |
| 历史影像日期 | Acquired / Published | 采集与服务发布日期必须分开 |

---

## 2. 官网文案：区分现在与目标终态

### 2.1 在 Global 尚未公开时可用的诚实介绍

以下段落以当前 README 所述能力为基础，仍应按实际发行包验证平台与格式支持。[R01]

> **GeoD is a free, open-source desktop tool for working with geospatial data.**
>
> Browse maps, download OpenStreetMap features, and export imagery from tile services you are authorized to use. GeoD also includes workflows for elevation data, historical imagery and 3D Tiles.
>
> We are designing a dedicated global experience around open data, repeatable workflows and local files. The new experience is in development; features shown in design previews may not yet be available.

**Calls to action**

- `Download the current release`
- `Read the source code`
- `Help shape GeoD Global`

第三个入口应是明确同意参与的研究招募，不是把未实现产品包装成可用版本。当前版本下载页必须保持当前版本的真实名称和能力表。

### 2.2 完整目标产品通过验收后使用的首页结构

**Hero**

> **Geospatial data, ready for your project.**
>
> Find imagery, elevation and vector data for an area. Preview the results, prepare the files and save a workflow you can run again.
>
> A local-first desktop workspace. Free and open source at its core.

**Primary CTA**: `Download GeoD`

**Secondary CTA**: `See a real workflow`

**Trust strip**

> Local files · Open formats · Reusable recipes · Optional cloud services

不要放未经授权的合作方 Logo、虚构使用人数、未测量性能倍数或“官方认证”标记。平台图标只显示已发布且通过签名/安装验收的平台。

**Section: Start with an area**

> Draw a boundary, open a polygon or find a place. Keep the same area as you explore imagery, terrain and vector data.

**Section: Preview before you download**

> Inspect coverage, dates and available assets before committing to a large job. See where the data comes from and which conditions apply.

**Section: Prepare files you can use**

> Clip, align and export data using the options supported by each workflow. Keep the source information and processing settings alongside the result.

**Section: Repeat the work, not the setup**

> Save a recipe for the next date, area or project. Run it from the desktop or command line, and inspect the same task history.

**Section: Built to work with your tools**

> Export to documented, interoperable formats and continue in your preferred GIS, analysis or visualization software.

格式 Logo、兼容软件名称和“一键打开”按钮以真实测试清单为准。能写出 `.tif` 不等于所有应用都能正确打开，能读取某种格式不等于能够写出。

**Optional cloud section — only when the service actually exists**

> **Your local workspace does not require a subscription.**
>
> Add optional cloud services for shared recipes, team configuration and remote task coordination. Raw imagery stays at the source or in storage you choose unless you explicitly upload it.

最后一句只有在真实网络路径符合时使用；禁止在服务端已经转发原始影像时仍声称完全不经过 GeoD。

### 2.3 首页展示案例的文字模板

每个案例必须使用可再分发的真实样例、明确数据日期，并让访客看到结果文件说明。

| 案例 | 标题 | 证据 |
|---|---|---|
| 跨源项目数据包 | `One area. Imagery, terrain and buildings.` | 下载产物、来源、边界、CRS、处理参数 |
| 重复遥感任务 | `Update a monthly imagery workflow without starting over.` | Recipe、输入时间、选景规则、实际任务日志 |
| 本地文件处理 | `Turn a folder of rasters into a documented deliverable.` | 输入说明、拼接/裁剪结果、校验与 provenance |

不把“每月最少云的一景”说成“每月无云覆盖整个区域”。结果受真实观测、范围覆盖和质量掩膜影响。

---

## 3. Pricing 页与 FAQ

### 3.1 公开定价前的前置检查

价格必须和实际交付、服务限制、支付平台批准的商品描述相匹配。主文档的 $99/$399 年费等仅是研究假设；本模板故意用 `[approved price]`，避免直接误发上线。

**Page introduction**

> **Start locally. Add services when you need them.**
>
> GeoD’s open-source local workflows remain available without a cloud subscription. Optional services cover shared infrastructure, coordination and support—not ownership of third-party data.

| Plan | Description | CTA |
|---|---|---|
| Desktop | `Free local workflows, open-source code and files you control.` | `Download free` |
| Cloud Workspace | `Optional hosted recipe storage, shared settings and task coordination for individual workflows.` | `View service details` |
| Teams | `Organization-level access, shared configurations and team coordination.` | `Explore Teams` |
| Professional services | `Clearly scoped setup, workflow review or integration work.` | `Request a scoped quote` |

服务尚未上线时，不展示可付款 CTA；可以说明研究中的服务方向，但不可给人已经可购买的印象。纯人工服务与软件订阅按各自合规的收款路径处理。[P03]

### 3.2 FAQ 草稿

**Is the desktop application free?**

> GeoD’s existing open-source components are distributed under the MIT License. The global product design keeps local data workflows separate from optional paid services. Check the release page and license files for the exact scope of each component.

公开发布时用最终许可与发行结构替换“product design”；不能用含糊的“open source at its core”掩盖实际闭源部分。[R02]

**Am I paying for Sentinel or OpenStreetMap data?**

> No. A GeoD service fee is for the software service described in your plan. Data providers set their own access, attribution and usage terms. Some endpoints may charge for transfer, processing or commercial imagery even when a related dataset is openly licensed.

**Can I use the data commercially?**

> It depends on the specific dataset, service and intended use. GeoD shows the source and available license information, but does not grant rights on behalf of the data owner. Review the applicable terms before distribution or commercial use.

**Does “local-first” mean nothing leaves my computer?**

> No. Searching an online catalog sends query information to that service, and downloading data makes requests to its servers. Local files are not uploaded to GeoD services unless a feature explicitly requires it and you choose to proceed. Optional diagnostics and cloud features are explained separately.

**Do I need an AI account?**

> No. AI is optional. Standard data discovery, preparation and export workflows do not require a language model. Any AI integration should show which provider receives your request and what information is included.

**What happens if I cancel a cloud subscription?**

> Your local files and open-source local workflows remain available. Cloud access follows the cancellation and retention terms for your plan. You can export your recipes and metadata before the applicable retention period ends.

必须在最终条款中写明实际宽限期、删除范围和账户删除流程，不让此文案成为未兑现承诺。

**Are taxes included?**

> The checkout shows the final amount, applicable taxes and billing details before you confirm the purchase. Your receipt identifies the merchant of record where one is used.

不写“we handle all taxes for everyone”，MoR 并不替代卖家自己的全部税务义务。[P06]

**Do you offer refunds?**

> Please review the refund policy for the specific product or service before purchase. Contact `[billing contact]` with your order reference if you need help. Statutory rights are not limited by our policy.

这是占位说明，不替代由实际经营主体、商品形态和销售地区审查后的退款条款。

---

## 4. 海外需求访谈：招募信与问题

### 4.1 GIS / 遥感工作者招募

**Subject:** `How do you prepare geospatial data before a project?`

> Hi `[name]`,
>
> I’m Gao, the developer of GeoD, an open-source desktop geospatial tool. I’m researching how people find and prepare data before it reaches their main GIS or analysis workflow.
>
> I’m especially interested in the last real task you completed: which sources you used, where the workflow became inconvenient, and how you checked the output.
>
> Would you be open to a 20-minute conversation? This is product research, not a sales call. There is no need to share confidential data; a description or redacted example is enough.
>
> `[If applicable: compensation and terms]`
>
> Thanks,
> Gao
> `[verified project page]`

**使用规则**：只向有合理业务关联并允许接收此类联系的人发送；不批量抓取邮件、不隐藏商业身份、不连续催促。不得把公开 GitHub email 当无限营销许可。

### 4.2 开发者 / ML 用户变体

**Subject:** `Research: repeatable imagery and terrain data workflows`

> Hi `[name]`,
>
> I’m studying how developers turn catalog results and cloud-hosted assets into repeatable local datasets. I maintain GeoD and am designing a dedicated global experience.
>
> I’d like to understand a concrete workflow rather than ask whether a new app sounds interesting. For example: what you needed, how you selected the data, which scripts or tools you used, and what you would change next time.
>
> A short written reply is also welcome. Please do not send private endpoints, credentials or project data.
>
> Thanks,
> Gao

### 4.3 不带诱导的访谈顺序

1. `Tell me about the most recent project for which you had to obtain geospatial data.`
2. `What output did you need, and which tool or person consumed it next?`
3. `Please walk me through the actual steps you took.`
4. `Where did you wait, repeat work, fix errors or need someone else’s help?`
5. `How did you check that the data, dates, coordinate system and values were correct?`
6. `How often do you repeat a similar task, and what do you reuse today?`
7. `What have you already tried? What would make you keep the current approach?`
8. `Are there restrictions on installing desktop software, querying external services or uploading areas and files?`
9. `Who decides which tools can be purchased, and what has been approved for similar work?`
10. `Could we test a redacted version of this task with you later? What would count as success?`

先问实际任务，再展示产品。不要问“你是不是觉得 QGIS 很复杂”“你愿不愿意付 $99”，这些问题容易收集礼貌赞同，而不是决策证据。

### 4.4 价格研究模板

先说明免费本地边界，再单独展示一个真实额外服务方案。

> The local application is not the subscription. The optional service in this concept provides `[specific hosted capability]` with `[specific limits]`.
>
> What, if anything, would this replace for you? Who would use it? What budget would it come from? At `[test price]`, what would prevent approval?

记录是否有实际替代支出、购买权限、可接受合同/发票形式和使用频率。不得将“价格看起来可以”计为购买意向已验证。真实付费测试只在能交付、能退款、主体与收款合规时进行。

### 4.5 研究记录模板

```text
Research ID:
Consent and allowed use:
Role / organization type / region:
Recent real task:
Existing tools and cost:
Frequency:
Required output:
Where time or reliability is lost:
Security / licensing constraints:
Buyer and approval process:
Alternative they would otherwise choose:
Observed behavior:
Hypothesis supported / contradicted:
Follow-up permission:
```

公开案例须另行征得姓名、机构、截图和引述的使用同意。研究同意不等于营销同意。

---

## 5. Show HN 与社区发布模板

### 5.1 Show HN：只有产品能实际试用时使用

HN 的 Show HN 指南强调可试用的实际作品，不适合只有营销页或候补名单的发布。[G01]

**Title candidate**

> Show HN: GeoD — a local-first workspace for geospatial data

**Body**

> Hi HN, I’m Gao, the maker of GeoD.
>
> I built this for the part of a geospatial project that happens before analysis: finding data, checking coverage, preparing files and repeating the same work for another area or date.
>
> The current release can `[list only verified capabilities]`. Here is a reproducible example: `[sample workflow]`. The example includes the input area, source information, output settings and a downloadable result description.
>
> GeoD is not intended to replace QGIS or official satellite browsers. The distinction I’m testing is whether a dedicated local workspace makes cross-source and repeatable data preparation less cumbersome.
>
> Source: `[repository]`
> Download: `[release]`
> Known limitations: `[limitations page]`
>
> The local application is `[exact license and free scope]`. `[Describe optional paid services only if available; otherwise omit.]`
>
> I’d appreciate feedback on a real workflow you would use—or a reason you would stay with your current tools.

**禁用**：未经证实的“10× faster”、虚构用户评价、暗示比 QGIS 全面、请求点赞、安排账号互相投票。

### 5.2 Reddit / 专业社区：先给解法

先核对目标社区当前规则；通用反垃圾政策不能代替每个 subreddit 的具体要求。[G02]

**Title**

> A reproducible way to prepare `[dataset]` for `[specific task]`

**Post**

> I recently worked through `[real, documented task]`. The result needed to be `[format, grid and quality requirements]`.
>
> The steps were:
> 1. `[data selection and source]`
> 2. `[coverage/quality checks]`
> 3. `[preparation]`
> 4. `[validation and export]`
>
> The details that mattered were `[actual pitfall, such as scene cloud cover versus area validity]`.
>
> You can perform this workflow using `[honest existing alternative]`. I also implemented it in GeoD, which I develop, and documented the same inputs and output checks here: `[link, only if community rules allow]`.
>
> Limitations: `[specific limitations]`.
>
> How do you handle `[specific unresolved question]` in your own workflow?

不把没有真实操作记录的模板当做“我刚做完”的案例发布。

### 5.3 一次允许的研究跟进

> Hi `[name]`, just following up once on my research note about geospatial data preparation. No worries if it is not relevant. I won’t follow up again unless you reply.

对方拒绝或退订后停止；是否可发送首次/跟进邮件还应按实际营销合规要求审查。

---

## 6. 教育与专业伙伴外联

### 6.1 教师 / 培训机构

**Subject:** `A reproducible data-preparation exercise for your GIS course`

> Hi `[name]`,
>
> I maintain GeoD, an open-source geospatial desktop tool. I’m preparing a teaching exercise on obtaining data for a defined area and checking the result before analysis.
>
> The exercise includes `[verified sample inputs]`, source and license notes, the intended output, and checks for coordinate reference, coverage and pixel values. Students can compare the result with their usual tools.
>
> Would this be useful for your course? I would value feedback on the workflow and explanation rather than an endorsement. I would not use your institution’s name or logo without permission.
>
> Best,
> Gao

### 6.2 GIS 顾问 / 小团队

**Subject:** `Research on repeatable data handoffs for GIS projects`

> Hi `[name]`,
>
> I’m exploring how small GIS teams hand off prepared data across projects: source references, coordinate systems, processing settings and repeatable recipes.
>
> Are these steps currently handled with scripts, shared folders or a standard checklist in your team? I’d be interested in one anonymized example where the handoff was difficult.
>
> I’m the developer of GeoD. This is a research request; there is no expectation to purchase anything or disclose client data.
>
> Thanks,
> Gao

### 6.3 Provider 互操作与许可确认

**Subject:** `Client integration and usage-permission questions for [service]`

> Hello `[provider team]`,
>
> We are evaluating an integration of your service into GeoD, a local-first geospatial desktop application. Users would authenticate using their own accounts where required.
>
> Could you confirm which of the following are permitted under the applicable plan or agreement?
>
> - Interactive preview and necessary temporary caching.
> - User-initiated area downloads or offline packages.
> - Clipping, reprojection and format conversion.
> - Retention of outputs on a user’s device.
> - Commercial use and redistribution of original or derived outputs.
> - Attribution requirements and any logo-placement rules.
> - Rate limits, concurrency guidance and paid transfer/processing charges.
>
> We will not redistribute credentials or describe the integration as an official partnership without agreement. Please also point us to the current API documentation and the applicable terms.
>
> Regards,
> `[real legal name / project identity]`

---

## 7. 支付平台预审核邮件

### 7.1 Paddle：必须先确认商品形态与主体

Paddle 的官方资料允许个人/sole trader 进入相应身份审核流程，但这不保证特定中国主体、产品和银行账户获批；其 AUP 以软件为中心，不能将纯咨询或赞助一概按 SaaS 收款。[P01–P05]

**Subject:** `Pre-onboarding eligibility review: China-based developer and optional cloud software for an open-source desktop app`

> Hello Paddle team,
>
> I’m evaluating Paddle for an optional cloud software service associated with GeoD, a local-first geospatial desktop application.
>
> **Applicant**
> - Legal name: `[actual legal name]`
> - Applicant type: `[individual / sole trader / registered company — use the actual status]`
> - Country of residence or incorporation: `[actual country]`
> - Product website: `[owned website]`
> - Bank or payout account country and account-holder name: `[actual details, sent only through an approved secure process]`
>
> **Product**
> The existing desktop application is open source under the MIT License. Its local capabilities remain free. The proposed paid product is a hosted software service for `[exact implemented or planned capabilities: recipe storage, team configuration, access controls and task coordination]`.
>
> It does not sell third-party satellite imagery, promise unlimited access to external data, or resell an AI provider’s subscription. Raw data would normally transfer between the user and the source; please see `[architecture/privacy page]` for the precise flow.
>
> Proposed billing: `[monthly/annual prices and limits]`. Current status: `[in development / private testing / available — be accurate]`.
>
> Could you confirm:
>
> 1. Whether this applicant type and country can be considered for onboarding for this product.
> 2. Which identity, business and domain documents are required.
> 3. Whether the proposed software subscription fits your Acceptable Use Policy.
> 4. Whether software-related onboarding or support may be bundled, and what must instead be sold separately. We understand that pure consulting, donations and sponsorship may not qualify.
> 5. Which payout methods, account requirements, supported currencies, conversion costs, minimums and schedules apply to this applicant.
> 6. What invoice, merchant-of-record disclosure, refund and customer-support information must appear before launch.
> 7. Whether sandbox approval differs from permission to accept live payments, and what validation you recommend before launch.
>
> We will not start charging customers until product eligibility, account verification and payout requirements are confirmed.
>
> Thank you,
> `[real name]`
> `[business contact]`

不要在普通邮件中发送身份证影像、银行卡完整信息、完整住址证明或 API secret；按平台正式安全流程提交。不要伪造海外地址、主体或收款人。

### 7.2 Lemon Squeezy：重点问可用收款路径

**Subject:** `Seller and payout eligibility for a China-based software developer`

> Hello,
>
> I’m reviewing eligibility for `[same accurate software-service description]`. The applicant is `[actual type and jurisdiction]`.
>
> Your supported-country documentation distinguishes bank payouts from PayPal payouts. Could you confirm whether this applicant can be onboarded and, if so, which verified payout route is actually available?
>
> Please clarify product review, account-holder requirements, payout currency, transaction and payout fees, minimums, settlement timing, reserves and refund handling. I would also appreciate confirmation of whether the attached software/support packaging is acceptable.
>
> We have not assumed that buyer payment support or the availability of PayPal in a country automatically establishes seller eligibility.
>
> Regards,
> `[real name]`

### 7.3 未通过审核的回复与内部记录

**External reply**

> Thank you for reviewing the application. Could you clarify whether the decision relates to the applicant jurisdiction, product category, payout route or additional documentation? We will not attempt to use inaccurate registration details. Please let us know whether a legitimate change or a later reapplication would be eligible for review.

**Internal record**

```text
Provider / applicant / product:
Application date:
Decision and exact written reason:
Missing requirements:
Permitted next action:
Whether live sales are authorized:
Whether payouts are verified:
Owner / next review:
```

---

## 8. 交易与服务邮件模板

仅在服务端已验证相应状态后发送。浏览器跳转成功不能触发“付款成功”或付费权益邮件。[P07]

### 8.1 云服务已激活

**Subject:** `Your GeoD [plan] is ready`

> Hi `[name]`,
>
> Your `[plan]` is now active for `[account or organization]`.
>
> Billing period: `[start]` to `[end]`
> Plan details: `[link]`
> Manage billing: `[authenticated billing portal]`
> Get started: `[guide]`
>
> This subscription covers `[specific service]`. It does not change ownership of your local files or the terms of third-party data sources.
>
> `[Merchant-of-record and receipt information appropriate to the actual provider]`
>
> Need help? Contact `[support]` with reference `[non-secret order reference]`.

### 8.2 付款已收到，权益正在同步

**Subject:** `We received your payment — your account is being updated`

> We have confirmed payment for `[order reference]`, but the service entitlement is still being synchronized. Please do not purchase again.
>
> You can check the status at `[authenticated status page]`. If the update has not completed within `[real support threshold]`, contact `[support]` with the reference above.
>
> Your local application and files are unaffected.

### 8.3 续费未成功

**Subject:** `Action needed for your GeoD [plan] renewal`

> We could not complete the renewal for `[plan]`. Please review your payment method in the billing portal.
>
> Current service access: `[actual state]`
> Next relevant date: `[date and timezone]`
> Manage billing: `[link]`
>
> The local application remains available. Cloud access and retention follow the published plan terms; your local files will not be deleted because a renewal failed.

### 8.4 取消确认

**Subject:** `Your GeoD subscription cancellation is confirmed`

> Your `[plan]` will not renew. Current access continues until `[actual paid-through date]`, subject to the applicable terms.
>
> Export your recipes and metadata: `[guide]`
> Cloud retention details: `[policy]`
>
> Your local files and free local workflows remain available. If this change was unexpected, contact `[support]`.

### 8.5 退款确认

**Subject:** `Refund update for GeoD order [reference]`

> A refund of `[amount and currency]` has been `[approved / submitted / completed — use the actual state]` for order `[reference]`.
>
> The time before it appears in your account depends on the payment provider and your financial institution. Your updated service status is `[actual entitlement state]`.
>
> Contact `[billing contact]` if you need help. Your local files are unaffected.

不写无法保证的“24小时到账”；退款审批、平台退款成功、银行入账是不同状态。

---

## 9. 支持与错误文案

### 9.1 请求诊断信息

> Please share the GeoD version, operating system, provider name, workflow and the error reference shown in the task details.
>
> A small reproducible example is helpful, but please remove API keys, signed URLs, personal information and confidential areas or files. You can review the diagnostic package before sending it. We do not need your full project by default.

### 9.2 产品内错误示例

| 情况 | 英文文案 |
|---|---|
| 场景无结果 | `No scenes match this area and time range. Try a wider date range or review the filters.` |
| 搜索成功、预览不支持 | `Metadata is available, but this source does not provide a supported preview.` |
| 服务禁止离线下载 | `This service permits previewing but does not grant offline-download rights under the configured terms.` |
| 授权未知 | `Download permission has not been verified for this source. Review its terms or connect an authorized service.` |
| Requester Pays | `This asset may charge your cloud account for requests or data transfer. Review the estimate and account before continuing.` |
| 签名过期 | `The download link has expired. GeoD will request a new link from the source where supported.` |
| 成果部分缺失 | `Completed with missing data. Review the coverage and failure report before using this output.` |
| 暂停不支持 | `This processing step cannot pause safely. You can cancel it and restart from the last available checkpoint.` |
| 文件被移动 | `The saved file is no longer at this location. Locate the file to reconnect it; GeoD will not download it again automatically.` |
| 单位/CRS不完整 | `The coordinate reference or units are incomplete. Confirm them before running measurements or exporting.` |
| 空间不足 | `There is not enough free space for the estimated output and temporary files. Choose another location or reduce the job.` |
| AI建议待批 | `Review the area, data sources, output and estimated cost before running this plan.` |

### 9.3 状态披露

> **Service incident: [provider or GeoD component]**
>
> Observed impact: `[verified behavior]`
> First observed: `[timestamp and timezone]`
> Affected operations: `[specific operations]`
> Workaround: `[tested alternative or “None confirmed”]`
> Next update: `[real staffed commitment, or omit]`
>
> We have not confirmed the root cause yet.

不要把供应商不可用说成用户网络错误，不在没有证据时归咎第三方。

---

## 10. 发布前的文案验收

| 检查项 | 必须通过 |
|---|---|
| 能力 | 每个现在时功能承诺有对应发行版本和验收记录 |
| 数字 | 用户数、下载量、性能、价格、节省时间都有定义与证据 |
| 图片 | 概念图明确标注；真实截图不伪造输出或处理完成状态 |
| 竞品 | 使用当前产品名称与实际能力，不贬低或制造不存在的空白 |
| 开源 | 现有 MIT 权利、付费服务和新增组件许可写清楚 |
| 收费 | 商品/主体/支付通道已获准；不把纯咨询塞进不适用的 MoR 商品 |
| 数据 | 数据许可、署名和服务条件明确，不宣传无限抓取 |
| 隐私 | 实际发送内容与文案一致；诊断、研究与营销同意分开 |
| 交易 | 退款、续费、取消、保留期和商户身份前后一致 |
| 外联 | 发布符合社区规则，发送符合适用规则，无虚构客户或背书 |
| 链接 | 方括号占位符已替换；下载、支持、条款、账单入口真实可用 |

**整套英文文案的原则：让用户知道你真正做了什么、没有做什么，以及付的钱具体买到了什么。**
