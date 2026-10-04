# 提交、构建与发布自动化

本流程仅属于独立 GeoD Global 仓库，不读取国内版工作树、依赖、凭据或发行渠道。

## 三条工作流

| 工作流 | 触发方式 | 结果 |
| --- | --- | --- |
| `Repository checks` | 分支 push、PR；被其他工作流复用 | Ubuntu 24.04 / Windows 2025 的版本、前端、契约、Rust、打包校验测试，格式和 Clippy 检查；actionlint 校验工作流 |
| `Windows evaluation artifacts` | Actions 页面手动运行；发布工作流调用 | 检查全部通过后生成未签名 Windows x64 ZIP、NSIS 安装器、`artifacts.json`、`SHA256SUMS.txt`；Actions artifact 保留 14 天 |
| `Release Windows evaluation` | 推送 `v*` 标签 | 标签与版本一致后执行同一检查和打包流程，创建草稿，下载服务器上的全部资产复验，最后发布为 prerelease |

普通提交不创建 Release。发布不会把评估包设为 latest stable。手动构建文件需要先进入 GitHub 默认分支，才能从 Actions 页面触发。

## 提交和预览构建

本地验证：

```sh
npm ci --no-fund
python -m pip install -r requirements-dev.txt
npm run verify
python scripts/release.py check
python scripts/package-windows.test.py
python scripts/release.test.py
cargo fmt --all --check
cargo test --locked -p geod-runtime
```

提交并推送分支即可运行 CI。需要可下载的完整 Windows 产物时，在 Actions 选择 `Windows evaluation artifacts` → `Run workflow`，或在已配置远程的仓库执行：

```sh
gh workflow run windows-artifacts.yml --ref YOUR_BRANCH
gh run list --workflow windows-artifacts.yml
gh run download RUN_ID --name geod-global-windows-x64-unsigned
```

## 创建版本发布

1. 保持 `package.json`、`package-lock.json` 两处根版本、`src-tauri/tauri.conf.json`、两个 crate 的 `Cargo.toml` 以及 `Cargo.lock` 中本地包版本一致。修改 Cargo 版本后运行 `cargo check --locked -p geod-runtime` 会提示锁文件过期；用 `cargo check -p geod-runtime` 更新并审查锁文件变化。
2. 更新 `docs/releases/RELEASE-NOTES.md`，准确描述该版本能力和未完成验收。提交版本及说明修改并推送。
3. 以下以当前 `0.1.0-rc.2` 为例，在干净提交运行：

```sh
python scripts/release.py check --tag v0.1.0-rc.2 --require-clean
git tag -a v0.1.0-rc.2 -m "GeoD Global 0.1.0-rc.2 Windows release candidate"
git push origin v0.1.0-rc.2
```

版本脚本支持严格 SemVer（包括预发布后缀），且标签必须精确等于 `v` 加清单版本。检查不会自动修改版本或创建标签。由人或正常 Git 凭据推送标签；不要依赖使用工作流自身 `GITHUB_TOKEN` 推标签去触发另一条工作流。

## 发布约束与失败恢复

- 每条 Actions 依赖固定到完整提交 SHA，Dependabot 每月提出更新；Node 24.20.0（附带 npm 11.19.0）、Python 3.12、Rust 1.91.1 和 NSIS 3.11 是显式构建依赖。
- NSIS 从官方 SourceForge ZIP 下载，校验固定 SHA-256 后仅解压到 runner 临时目录，不在系统安装。SPDX 文本固定官方提交，瞬态网络错误最多尝试三次。
- 打包前要求工作树干净。上传前检查外层清单、ZIP 内部 source、build receipt 均指向本次 SHA；交叉检查二进制哈希、逐文件清单和外层校验文件。
- 只暂存四个明确文件，源仓库、任务数据和整个 `.verification` 目录不会进入 Actions artifact。发布 job 从同一次运行下载 artifact 再验证。
- 只有 `publish` job 拥有 `contents: write`，不使用个人 PAT。PR 和构建 job 只有读取权限。Release 使用 GitHub 默认 token，仓库需要启用 Actions。
- 发布先创建草稿；上传后的文件在 Linux 上再次逐文件与 SHA-256 校验，并与上传前文件逐字节比较，然后才公开为 prerelease。仓库本身的私有／公开可见性另由仓库设置控制。
- 重跑不会覆盖同名 Release 或资产。构建失败可以重跑；如果上传后失败留下草稿，先检查失败原因和草稿内容，再人工决定删除未发布草稿后重跑，或保留草稿诊断。不要移动已发行的版本标签。

CI 通过、生成产物、发布 prerelease 是三个不同状态，应以实际 GitHub run / Release 结果为准。签名、原生桌面交互和干净机器安装／升级／卸载需要后续独立验收。
