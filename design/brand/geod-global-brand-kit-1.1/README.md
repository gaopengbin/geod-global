# GeoD Global 正式品牌套件 1.1

采用已确认的 **蓝 → 青 → 绿渐变、立体折带 G 与轨道地球**。本套件直接使用选定 v5 图形的原图，未再次生成或修改它的造型和配色。

## 快速选文件

| 场景 | 文件 |
| --- | --- |
| 查看整套素材 | `overview.png` |
| 浅色网页 / 软件横向 Logo | `logos/horizontal-on-light.svg` 或 `.png` |
| 深色网页 / 软件横向 Logo | `logos/horizontal-on-dark.svg` 或 `.png` |
| 竖向 Logo | `logos/stacked-on-light` / `stacked-on-dark` |
| 只有品牌文字 | `logos/wordmark-on-light` / `wordmark-on-dark` |
| 单色文字标识 | `logos/wordmark-solid-ink` / `wordmark-solid-white` |
| 原始透明图形 | `source/approved-symbol.png` |
| Windows 应用图标 | `icons/windows/geod-global-app-dark.ico` |
| Windows 透明图形图标 | `icons/windows/geod-global-symbol.ico` |
| macOS 应用图标 | `icons/macos/geod-global-app-dark.icns` |
| 网页 Favicon | `icons/favicon.ico` / `icons/favicon-32.png` |
| Apple Touch Icon | `icons/apple-touch-icon-180.png` |
| PWA / App PNG | `icons/png/app-dark-192.png` / `app-dark-512.png` |
| 社交头像 | `social/avatar-dark.png` / `avatar-light.png` |
| 圆形头像透明外沿 | `social/avatar-circle-dark.png` / `avatar-circle-light.png` |
| 链接分享封面 | `covers/open-graph-1200x630.png` |
| GitHub 仓库社交封面 | `covers/github-social-1280x640.png` |
| README 顶部横幅 | `covers/readme-header-1600x520.webp` |
| 官网 Hero 装饰 | `covers/website-hero-1920x1080.webp` |
| 社交横屏封面 | `covers/social-landscape-1600x900.png` |
| 无文字太空背景 | `backgrounds/clean-space-original.webp` |
| 无文字浅色背景 | `backgrounds/clean-light-1920x1080.webp` |
| 桌面壁纸 | `backgrounds/desktop-dark-1920x1080` / `desktop-light-1920x1080` |
| 配色变量 | `guidelines/brand-tokens.css` / `brand-tokens.json` |

Logo 的 `on-light` / `on-dark` 指适用背景，文件本身保持透明。横向 Logo 提供 1x、2x、4x PNG；竖向和文字版提供 1x、2x。头像为 1024 px。图标为 16、24、32、48、64、96、128、180、192、256、512、1024 px，包含透明、浅底和深底三组。ICO 包含 16–256 px 的七个尺寸；ICNS 使用 256/512/1024 px PNG 图块。

## 品牌规范

- 对外产品名为 **GeoD Global**，`GeoD` 作为完整名称单元加粗。技术项目名保持原样。
- 主渐变：蓝 `#087BEA` → 青 `#00BBDF` → 绿 `#21C885`。字标中的 Global 从左到右过渡，图形保持源图已有的立体渐变分布。
- 深色底 `#071423`，浅色底 `#F3FAFF`，正文深色 `#102A43`。渐变用于品牌文字和装饰，正文、操作提示用实色以保证可读性。
- 字体为 Segoe UI，回退 Arial / sans-serif。标题可用 600–700，正文使用 400。GeoD 字重 700，Global 字重 400。
- 符号外围留白至少为画布宽度的 12%。小尺寸优先使用符号，横向字标显示高度建议不少于 32 px；24 px 以下使用已导出的图标并检查实际显示效果。
- 不拉伸、不旋转、不单独改变地球位置、不添加大轨道、不把图形裁切到圆形头像边缘、不重新给源图染色。只允许等比缩放与背景选择。
- 复杂影像背景上加深色遮罩并留出文字区。浅底使用 `on-light`，深底使用 `on-dark`。禁用时通过控件状态表达，不改变官方 Logo 配色。
- 单色印刷可使用本套件的**纯文字版**；它不包含 G 图形，不能宣称为原图的单色矢量版本。

## 文件与使用边界

Logo 图形是原始高清 PNG。SVG 字标容器内嵌该 PNG，并保留可编辑文字；它们不是图形的纯矢量描摹。SVG 独立可用，无外链图片或字体。导出 PNG 固化了字体；印刷时优先用足够尺寸的 PNG，或由设计软件处理文字转曲。

太空背景源图为 1672 × 941 的装饰性生成图像。1920 px 封面和壁纸由该背景结合品牌图形排版而成，不代表原背景新增了真实细节，也不是地理测绘数据。

套件只生成可用素材，没有替换当前软件或发布到官网。使用前按需要选择对应文件，避免把预览总览和全部设计源文件一起打入软件安装包。

## 来源和复现

- 已确认图形与背景的生成提示词分别保存在 `source/approved-generation.json` 和 `source/background-generation.json`，使用内置 image_gen。
- 本次套件无新增模型调用，图形原图哈希和导出记录在 `source/PROVENANCE.json`；SHA-256 文件清单在 `MANIFEST.json`。
- SVG 排版源和原 PNG 一并提供。导出工具为仓库的 `scripts/build-approved-brand-kit.mjs`，渲染依赖仅用于素材导出，不加入软件运行依赖。
- 原素材许可见 `LICENSE`。

在仓库根目录重新导出：

```powershell
npm install --prefix .verification/brand-tools --no-save --package-lock=false @resvg/resvg-js sharp
node scripts/build-approved-brand-kit.mjs
```


## 1.1 revision

The user requested removal of the small globe and its orbit. All symbol-based exports now use the simplified G. The previous 1.0 kit remains available for comparison. The original half-Earth background and brand palette are unchanged. The edit prompt and provenance are saved in source/approved-generation.json.
