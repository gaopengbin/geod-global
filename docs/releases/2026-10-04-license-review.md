# RC.1 随包依赖文本核对

本记录仅说明 0.1.0-rc.1 打包过程中补齐的依赖文本，不改变第一方权利或数据源使用条件。
发布版 npm tarball 缺少以下四个依赖的独立许可文件。已从 npm 官方的精确版本记录获取
`gitHead`，按固定提交读取官方上游内容，并在打包流程中固定 SHA-256；不跟随主分支。

| npm 包 | 精确版本 | 上游提交 / 内容 |
| --- | --- | --- |
| @cesium/wasm-splats | 0.1.0-alpha.2 | [Cesium 官方 LICENSE.md](https://github.com/CesiumGS/cesium-wasm-utils/blob/96a2fbae7ab1d117dd533fe558f0e061bed6762b/LICENSE.md)，Apache-2.0 |
| draco3d | 1.5.7 | [Google Draco LICENSE](https://github.com/google/draco/blob/8786740086a9f4d83f44aa83badfbea4dce7a1b5/LICENSE)，保留完整文件及所附通知 |
| bitmap-sdf | 1.0.4 | [精确提交 README](https://github.com/dfcreative/bitmap-sdf/blob/78de3569d32404a7009f62bea3befca55838118a/readme.md) 中的版权和 MIT 声明，另附固定 SPDX 的完整 MIT 条款 |
| mersenne-twister | 1.1.0 | [精确提交源文件](https://github.com/boo1ean/mersenne-twister/blob/83844a282375c657473edbe11aa2e2be85fe1746/src/mersenne-twister.js) 的 MT19937 版权 / BSD 条件完整保留；npm 声明的 MIT 条款另外附上 |

bitmap README 与已安装 npm 文件的规范化行文本一致，仅换行形式不同；
Mersenne 源文件与 npm 文件逐字节一致。README 中的简短许可名称没有被当成完整许可正文。
完整条件缺失、版本改变、上游内容或缓存校验不符都会阻止打包。
包内 `THIRD-PARTY/inventory.json` 记录实际文件、来源与校验值。
