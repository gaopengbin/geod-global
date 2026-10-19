# NASA 授权原件接入验收 — 2026-10-15

五个既有 Auth 条目完成实际账号原件验收：`nasa-earthdata`（HLS L30）、`nasa-srtm`（SRTMGL1）、`nasa-viirs-npp`、`nasa-viirs-noaa20`、`nasa-viirs-noaa21`。另一个既有 Planned 条目 `srtm-num` 的独立验收见[NUM 记录](srtm-num-original-acceptance-20261015.md)。本批没有新增目录 ID。

## 实际范围

| 产品 | 实际原件 | 完整传输字节 |
| --- | --- | ---: |
| HLS L30 v2.0 | `HLS.L30.T50TMK.2025245T025337.v2.0` 的 B04/B03/B02 三个完整 TIFF | 64,286,315 |
| SRTMGL1 v003 | `N39E116.SRTMGL1.hgt` 完整 ZIP 与其中 3601² HGT | 6,102,958 |
| VIIRS NPP v002 | `VNP09A1.A2025241.h26v05.002.2025253030927` 完整 HDF5 | 30,961,288 |
| VIIRS NOAA-20 v002 | `VJ109A1.A2025241.h26v04.002.2025253025020` 完整 HDF5 | 29,712,360 |
| VIIRS NOAA-21 v002 | `VJ209A1.A2025241.h26v04.002.2025337235206` 完整 HDF5 | 30,187,303 |

七份完整原件全部由软件任务真实下载并 succeeded/settled，没有复制研究文件充当软件下载。三个 VIIRS 原 HDF5 的 M5/M4/M3 共 12,960,000 个 Int16 原值，与独立 GDAL HDF5 解码的完整数组逐件哈希一致；九件软件准备的 GeoTIFF 也对全部 12,960,000 个原值核对通过。保留原始正弦投影、1200² 网格、scale=0.0001、NoData=-28672，不重投影、不应用 QA 掩膜。

HLS 保留原 Int16、30 m UTM 网格、NoData=-9999 与独立 HLS 反射率换算。SRTM 保留有符号大端 HGT、EGM96 与 Point 网格。共 67 组软件全分辨率像元与独立参考匹配，覆盖实际存在的零、负值、范围外值和 NoData；没有把原值或换算结果自动当作质量合格。

普通重启时 60 个既有任务、35 个工程以及上述原件、像元、元数据和预览逐字节不变。其后新增 NUM 的重启检查在其独立报告中记录。实际产品界面分别打开了五个工程、HLS RGB、三类 VIIRS RGB 与 SRTM 高程地图。完整原件和准备结果哈希见[机器可读摘要](nasa-original-acceptance-20261015.json)。

## 修复的真实问题

LP DAAC 当前签名 CDN 路径使用 `/s3-<32 位小写十六进制>`，原适配仅识别 `/get-…`，导致原件请求被拒。现在同时严格识别这两个有限形式，仍核对固定官方 CDN、原始桶路径、文件名、短期签名与到期时间；原账号令牌不会转发到 CDN，签名参数不会写入任务、工程或公开记录。

实际 VIIRS v002 将身份属性放在 HDF5 根节点，并使用 HE5/H5T ODL 名称，旧合成样例只覆盖了另一种布局。新增对真实布局的读取，保留原身份、平台、版本、八天周期、瓦片、类型和定标检查；拒绝两个属性位置同时声明身份或混合来源。首次失败及修复后的同任务重试记录保留，没有以换产品绕过失败。

## 权限与科学边界

Ready 表示已验收此原件链路，不等于每个用户已有 NASA 权限，也不等于全球每个分幅都逐件下载测试。新受保护下载仍要求用户在本地桌面连接自己的 Earthdata 账号，文件访问由原生工作进程再次检查。临时验收存储中的授权已移除；原有账号记录不变。

VIIRS 原 HDF5 的其他科学和质量数据仍完整保留，但本次只解码 RGB 所需 M5/M4/M3；没有宣称全部 QA 解码、云筛选或精度验收完成。HLS 是融合处理的反射率产品，不等同于 Landsat Collection 2。Copernicus SAFE 的实际授权原件仍未通过验收，继续保留 Auth。

来源：[NASA HLS](https://www.earthdata.nasa.gov/data/catalog/lpcloud-hlsl30-2.0)、[SRTMGL1 v003](https://doi.org/10.5067/MEASURES/SRTM/SRTMGL1.003)、[NPP](https://doi.org/10.5067/VIIRS/VNP09A1.002)、[NOAA-20](https://doi.org/10.5067/VIIRS/VJ109A1.002)、[NOAA-21](https://doi.org/10.5067/VIIRS/VJ209A1.002)。
