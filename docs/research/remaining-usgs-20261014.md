# USGS Landsat 直连：本轮实时复核

2026-10-14 以匿名 HTTP 请求复核原待办 `usgs-landsat`。本轮没有创建凭据、使用旧会话中透露的令牌或请求新增账号权限，也没有把其他镜像的下载结果算作 USGS 直连完成。

官方 [STAC 说明](https://www.usgs.gov/landsat-missions/spatiotemporal-asset-catalog-stac) 指向 LandsatLook 目录。实际检索 `landsat-c2l2-sr`、区域 `[116.3,39.8,116.5,40]`、2025-09 UTC 月、limit=1 返回 HTTP 200 和真实 Item `LC09_L2SP_123032_20250926_20250927_02_T1_SR`。采集时间为 `2025-09-26T02:53:47.863845Z`；文件名中的下一日是处理日期，不能当成观测时间。

从该 Item 的真实 `qa_radsat` asset href 发起有界 256 字节匿名 Range 请求，原站返回 HTTP 302，目标主机为 `ers.cr.usgs.gov`。没有跟随认证跳转，也没有保存重定向查询参数。该结果证明公开目录可检索，不能证明匿名数值文件可下载；没有收到 TIFF 数据、完整文件或原像元，不填写文件 SHA 或成功下载回执。

官方 [M2M 教程](https://code.usgs.gov/eros-user-services/machine_to_machine/m2m_landsat_9_search_download/-/tree/main) 将 ERS 账号、M2M 接口访问申请与应用令牌分别说明。因此既有账号或已创建令牌，不能直接推断用户当前已有 M2M 下载权限。后续须在用户已授权账号下取得真实接口访问结果，并完成原件下载、校验及软件读取。当前保留 Planned，不以网页登录、STAC 查询或已接 Landsat 镜像清账。

私有证据保存在 `.verification/remaining-backlog-20261014/usgs/real-query.json` 和 `anonymous-asset.json`。前者保留原请求与完整官方响应；后者只记录原 asset URL、状态、响应体长度和重定向主机，不收集凭据或 OAuth 查询串。本次为新的实时探测，旧失败回执仍保留。
