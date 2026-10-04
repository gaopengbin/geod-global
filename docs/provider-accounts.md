# 数据源授权入口

桌面版的「设置 → 数据源账号」提供 NASA Earthdata 与 Copernicus Data Space 的授权入口，探索页和下载任务中的「管理授权」直接打开对应表单。公开目录和缩略图无需账号。[受保护原文件下载适配](providers.md)已接入工程与任务，授权成功下载及新格式的本地处理尚未完成全流程验收。

## 授权方式

- NASA：HLS L30、SRTMGL1 v003 和 VIIRS 三个平台的 09A1 v002 共用同一 Earthdata 账号入口。用户在 [Earthdata Login 官网](https://urs.earthdata.nasa.gov/profile)生成用户令牌，在 GeoD 输入后验证。运行时将 bearer 令牌交给官方 CMR 验证接口；单纯解析 JWT 的过期时间不会建立成功状态。验证只确认 EDL 令牌有效，具体产品仍可能需要接受 LP DAAC 应用条款。SRTM / VIIRS 实时目录和浏览预览无需账号；HGT 原包与原始 HDF5 需要授权，详见[VIIRS 接入边界](viirs-integration.md)。
- Copernicus：账号密码仅用于一次登录，支持可选六位 TOTP。按照 [CDSE 官方认证文档](https://documentation.dataspace.copernicus.eu/APIs/Token.html)，使用 `cdse-public` 交换令牌，再访问官方 OIDC userinfo 确认身份。身份验证通过后只持久化刷新令牌，短期 access token 留在运行时内存；恢复会话时用刷新令牌取新授权。

表单密码、令牌和验证码均使用遮蔽输入，提交后清空，只保留非敏感用户名用于重试。未授权、待验证、已验证、过期和安全存储不可用分别显示。连接失败不覆盖此前有效的本地凭据。移除授权只删除 GeoD 在此设备保存的授权，不删除供应商账号，也不声称撤销官方服务端令牌。

## 存储及边界

使用固定版本 `keyring 3.6.3` 的 Windows Credential Manager 原生存储。凭据按独立工作空间的路径摘要区分，不保存路径文本。密码、TOTP、bearer、刷新令牌不进入 localStorage、工程、任务、来源链接、下载 sidecar 或诊断导出。

授权写操作只注册到 Tauri 主窗口的本地 ACL；浏览器开发适配器仅暴露 `GET /accounts` 的脱敏状态，不提供凭据提交、验证或删除的 HTTP 路由。官方网页在系统浏览器打开。账号请求共用用户设置的原文件下载代理，关闭自动重定向，并限制响应大小、连接时间和总请求时间。错误不回显提供方响应正文或原始异常，以通用错误和本次排查编号显示。

## 2026-10-01 验收边界

- Windows 凭据管理器实际写入、以新实例读回、删除及隔离测试通过，使用随机命名的合成凭据，测试后删除。
- 本地身份服务器测试覆盖 NASA 服务端拒绝、Copernicus 身份验证、刷新轮换、敏感信息不进入状态和凭据中的密码/TOTP、重启恢复、重定向拒绝及浏览器无凭据写路由。这些是协议测试，不冒充真实账号授权。
- 实时 CMR 对故意无效的测试令牌返回 401；官方 CDSE OIDC discovery 端点验证 token / userinfo 地址。没有使用真实用户账号进行成功登录或下载。
- 明暗主题 1440 宽、紧凑 1024 宽的浏览器检查通过，检查遮蔽字段、提交后清除、失败提示、重试、无凭据 localStorage、无页面异常与横向溢出。交互测试使用模拟原生 IPC，和上述系统安全存储与真实网络检查分别记录。
- [授权入口验收](../prototype/qa/provider-accounts-verification.json)与[授权下载适配验收](../prototype/qa/protected-download-verification.json)注明各证据的范围。原生 worker 已加入授权读取、重启恢复和 CDSE 刷新令牌轮换；浏览器不能取得令牌。[Landsat / HLS 本地原始波段读取](reflectance-inspection.md)已接入，HLS 使用夹具验证；原始反射率波段工程处理和 [SAFE JP2 本地准备](safe-processing.md)已接入。NASA / CDSE 真实账号原文件及完整生产 SAFE 的正向验收仍待完成，不能据此声明五个平台全流程验收通过。

2026-10-02：[SRTM 入口与验收](srtm-inspection.md)复用上述原生授权，不在浏览器开放凭据写入。真实隔离运行时的未连接 SRTM 任务以设置页提示停止，字节为 0、输出路径为空。本地 HGT 读取和缓存证据来自明确标注的合成文件，不等于实际账号原包下载。

2026-10-04：按用户确认的范围，先完成软件内授权入口，继续验收无需账号的公开来源。当前没有用于正向验收的 Earthdata / Copernicus 账号；保持成功登录、受保护生产原文件下载及其实际处理为待验收，不要求在聊天里提交凭据。后续用户可直接在桌面版「设置 → 数据源账号」授权。
