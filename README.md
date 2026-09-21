# GeoD Global

独立海外桌面产品仓库。产品目标是围绕同一区域发现、预览、比较、获取、处理和导出空间数据，并保留来源及可重复工作流。

**当前状态：已建立独立仓库和可交互设计原型。尚未接入真实桌面执行核心，模拟任务不能当作数据处理成果。**

## 仓库边界

- 海外版拥有自己的 Git 历史、依赖锁文件、配置、构建和发布流程。
- 国内版是独立项目；本仓库不包含国内版工作树，也不读取它的源码或依赖来启动。
- 以后需要复用核心能力时，使用经过审查、有明确版本的公共包或库；不使用跨仓路径、软链接或本机依赖回退。
- 最终形态为桌面主产品、配套公开网站及可选 Web 协作服务。当前浏览器页面用于设计验收，不代表纯 Web 产品改向。

## 快速启动

Node.js 22.12+、npm 10+；在仓库根目录执行：

```sh
npm ci
npm run dev
```

打开 <http://127.0.0.1:4317/>。生产预览：

```sh
npm run build
npm run preview
```

这几个命令仅需要本仓库，不依赖另一个 GeoD checkout。

## 验证

契约校验另需 Python 3.12（建议使用虚拟环境）：

```sh
python -m pip install -r requirements-dev.txt
npm run verify
```

`verify` 检查四个关键依赖的真实解析路径、七张真实样本的 SHA-256、规格相对链接、拟议契约正反例和 Vite 构建。契约不是当前生产 API。`verification-result.json` 是本地生成文件，运行校验后可查看。

GitHub Actions 已配置 Windows/Linux 检查，但在没有推送并完成运行前，不代表远程 CI 已通过。

## 目录

```text
GeoD-Global-Spec/       完整产品规格、代码参考审计、决策及拟议契约
prototype/             已认可视觉方向的交互原型
  src/                 React 页面与样式
  public/              真实场景快照、缩略图及字体许可
  qa/                  原型视觉验收截图
scripts/               独立依赖与样本检查
.github/workflows/     独立构建检查
```

[规格导航](GeoD-Global-Spec/00-README.md) · [原型使用说明](prototype/README.md) · [仓库隔离决定](GeoD-Global-Spec/08-Repository-Boundary.md)

## 样本、许可和发布状态

原型使用本地保存的七条 Sentinel-2 元数据和提供商 JPEG 缩略图。来源、原始链接及校验值见 `prototype/public/samples/manifest.json`；Inter 字体许可随包保存。开发及预览无需拉取真实大影像或登录云账号。

新产品代码的对外许可和商业包装尚待决定，根包以 `private: true` / `UNLICENSED` 防止被误当作已发布公共软件包。这不改变国内版或第三方资产已有权利。今后引入共享库必须保留其许可通知。

本地仓库初始化和提交不等于创建或发布了 GitHub 仓库。远程地址为空时，内容仍仅保存在本机。
