# DE Africa Fractional Cover 年度覆盖候选 · 2026-10-12

**已找到可完整读取的真实数值原件，尚未原生接入。** 推荐后续采用 `fc_ls_summary_annual`、v1.0.0、2025、内陆 tile `x218y075` 的 `pv_pc_50`，同时保留其他分量和独立百分位的科学含义。

原 item：[STAC 文件](https://deafrica-services.s3.af-south-1.amazonaws.com/fc_ls_summary_annual/1-0-0/x218/y075/2025--P1Y/fc_ls_summary_annual_x218y075_2025--P1Y.stac-item.json)；原数值：[pv_pc_50 TIFF](https://deafrica-services.s3.af-south-1.amazonaws.com/fc_ls_summary_annual/1-0-0/x218/y075/2025--P1Y/fc_ls_summary_annual_x218y075_2025--P1Y_pv_pc_50.tif)。

## 本轮实际完整文件

- 内陆 TIFF：**8,340,611 字节**，SHA-256 `ff7f3a266f4a216abf36546a179b628579e01bf65a1ecb0275539fd42b4a63ae`，与公开伴随 `.sha1` 的文件摘要独立匹配。3200×3200、UInt8、EPSG:6933、30 m 原格网，完整逐块解码 10,240,000 像元，10,102,789 个嵌入 mask 有效，原值 0–96，NoData=255。
- 原 item WGS84 bbox `[36.8135240654279, -1.5051857382785072, 37.808484175304336, -0.7525296804300243]`。内罗毕附近代表点 `(36.85005775696243, -1.2900704763300752)`，column=117、row=2285、原值2、NoData=false。这只是模型原始数值检查，不是实地植被精度验证。
- 首个边缘 tile `x154y096` 还完整读取了262,067字节原件，独立匹配 companion SHA-1；其数值不被当成陆地覆盖准确性证据。两份是新下载的独立研究文件，没有复用前轮原生文件。
- 详细原文、全部真实响应、原件 SHA、完整解码和契约差异：[内陆回执](./deafrica-fractional-cover-inland-candidate-20261012.json)、[边缘回执](./deafrica-fractional-cover-candidate-20261012.json)。只完整验证这两份 `pv_pc_50`；每个 item 的其他10资产仅元数据可见，不能称全部已测。

## 适配前需处理的真实契约

1. 原 asset 是相对文件名；self link 是 S3 下 `.json`，实际公开文件是 `.stac-item.json`。保留原 JSON，连接固定 HTTPS item，再限定同一审核目录的相对 asset。不能复用 WorldCereal adapter 把任意 S3 bucket 变成可信来源。
2. 年度区间由 `dtr:start_datetime`/`dtr:end_datetime` 声明，standard `datetime` 只有年初。应保留真实年度区间，不能把一年产品说成1月1日单景观测。
3. 原 TIFF 是 EPSG:6933 投影格网，item footprint 是 WGS84。需要实际验收 native 原图解码、预览、原值、工程恢复；该研究报告不证明软件已支持此新产品或处理转换。
4. 公开 companion SHA-1 已独立核验，本轮 native 没有读取或强制验证它；不能把外部核验当成产品内置完整性能力。

[官方数据说明](https://docs.digitalearthafrica.org/en/latest/data_specs/Fractional_Cover_specs.html) 将该服务定义为模型估计的绿植被、非绿植被和裸土分量，年度产品对每个分量独立估计10/50/90百分位，并声明 CC BY 4.0、非洲精度评估仍为 provisional。原数值可能超出标称范围，分量百分位也不代表同时观测，不能自动相加成100%或当成已校准的覆盖率。当前本轮内陆样本没有超100，但不能据此推广所有产品。
