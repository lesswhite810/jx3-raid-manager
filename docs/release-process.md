# Release Process

## 目标

发布流程需要同时满足两点：

1. 安装包版本号必须和 tag 完全一致。
2. Release Notes 必须是正常的 UTF-8 中文，并且站在用户角度书写。

## 中文防乱码规则

以后修改中文内容时，遵循下面规则：

1. 不要用 PowerShell 直接写中文文件内容。
2. 不要把中文 Release Notes 直接作为命令行参数传给 `gh release edit`。
3. 写中文文件时，统一使用 Node 脚本并显式指定 `utf8`。
4. 更新 GitHub Release Notes 时，统一使用 `npm run release:notes -- <tag> <notes-file>`。
5. 修改后用 Node 重新读取文件确认内容，不要只看 PowerShell 输出。

推荐校验命令：

```bash
node -e "console.log(require('fs').readFileSync('release-notes/v2.1.6.md', 'utf8'))"
```

## 发版前检查

发版前至少检查以下版本号一致：

- git tag
- `package.json`
- `src-tauri/tauri.conf.json`
- `src-tauri/Cargo.toml`

GitHub Actions 已经会在发布前自动校验这 4 个版本。

自动更新相关发布还要额外检查：

- GitHub Actions 生成了 updater 产物
- Release 中包含 `latest.json` 和对应签名文件
- 安装版资产命名保持稳定，避免更新清单指向错误资源
- 仓库 Secrets 同时配置了 `TAURI_PRIVATE_KEY`、`TAURI_PRIVATE_KEY_PASSWORD`、`TAURI_PUBLIC_KEY`
- 如果启用了 Gitee 回退源，还要同步配置 `GITEE_PUSH_URL`，可选配置 `GITEE_REPO`、`GITEE_ASSETS_BRANCH`
- `latest.json` 中的下载地址、版本号、签名与本次 release 资产一致
- Release 页面中，`latest.json` 标签应显示为“自动更新元数据”，`.sig` 标签应显示为“自动更新签名文件”
- Windows 安装器定制统一维护在 `src-tauri/nsis/hooks.nsh` 与 `src-tauri/nsis/SimpChinese.nsh`，不要再单独维护整份 `installer.nsi` 模板
- 安装目录选择页追加应用名称依赖 Tauri 默认 NSIS 模板行为，如需调整，先验证生成的 `target/release/nsis/x64/installer.nsi`

## Release Notes 存放位置

正式发布说明存放在仓库内：

- `release-notes/v2.1.5.md`
- `release-notes/v2.1.6.md`

以后新增版本时，按同样方式新增对应文件。

临时文件不要进入仓库：

- 临时文件放在 `tmp/`
- `tmp/` 已加入 `.gitignore`

## 更新 Release Notes

使用仓库脚本更新：

```bash
npm run release:notes -- v2.1.6 release-notes/v2.1.6.md
npm run version:prepare -- --next-patch-from 2.1.19
```

这个脚本会：

1. 先通过 tag 查到对应 release id
2. 再用 UTF-8 JSON 请求更新 GitHub Release 正文
3. 避免 PowerShell 中文参数链路导致的乱码

## Release Notes 写法

Release Notes 站在用户角度写，不写“做了哪些代码改动”，而写“用户能得到什么变化”。

推荐结构：

```md
## 新增需求

- 只写用户能直接感知到的新功能或新变化

## 修复 bug

- 只写用户能直接感知到的问题修复
```

没有对应内容的栏目就不要写，不必为了凑结构保留空段落。

## 写作要求

1. 只保留“新增需求”和“修复 bug”两类信息，没有的类别不要写。
2. 每条说明尽量能回答“这对用户有什么影响”。
3. 不要只列内部实现词汇，例如“重构”“拆包”“校验脚本”。
4. 错误发版要明确写“不要下载使用，请改用哪个版本”。

## 推荐流程

1. 先完成版本号同步。
2. 本地执行 `npm run build` 或 `npm run tauri build`。
3. `npm run tauri build` 本地默认只生成可执行文件，并启用快速本地 release 配置（incremental、较高 `codegen-units`、`rust-lld`）以缩短日常构建时间；如需本地验证安装包，使用 `npm run tauri:bundle`。
4. 如需在本地复现 GitHub Release 的完整 release 配置，可临时设置 `JX3_TAURI_FULL_LOCAL_BUILD=1` 后再执行 `npm run tauri build`。
5. GitHub Actions 发版时会通过 `TAURI_BUNDLE=1 npm run tauri build -- --config ...` 生成安装包与 updater 产物，不会使用本地快速构建配置。
6. 在 `release-notes/` 新增或更新对应版本说明。
7. GitHub Actions 发版时会自动把 `release-notes/<tag>.md` 同步到 GitHub Release 正文；如需补写或重写，再手动执行 `npm run release:notes -- <tag> <notes-file>`。
8. 如果更新了 updater 签名密钥，同步更新仓库 Secrets：`TAURI_PRIVATE_KEY`、`TAURI_PRIVATE_KEY_PASSWORD`、`TAURI_PUBLIC_KEY`。
9. 如果使用 Gitee 回退源，确认仓库 Secrets 已配置 `GITEE_PUSH_URL`，并确认 Gitee 仓库存在 `master` 代码分支和 `updater-assets` 资产分支。
10. 确认 `scripts/build-updater-manifest.mjs` 生成的 GitHub 与 Gitee 两份 `latest.json` 都指向正确资产地址。
11. GitHub Actions 在 release 成功后会自动把仓库版本推进到下一个补丁版本，并生成新的空白 `release-notes/v<next>.md` 模板，便于后续继续开发。
12. 最后用 GitHub API 或网页再次确认正文没有乱码，并确认 GitHub release 里已上传安装包、便携版、`latest.json` 与签名文件；同时确认 Gitee `updater-assets` 分支已同步 `updater/latest.json`、安装包和 `.sig` 文件。
