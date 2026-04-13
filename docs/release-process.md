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
- 如果使用 `workflow_dispatch` 重发已存在版本，workflow 会复用现有 tag，不再删除重建
- 如果目标 Release 已经存在，workflow 会强制刷新正文、解除 draft / prerelease 状态，并重新上传资产
- `workflow_dispatch` 手动重发时，构建内容以当前分支/当前提交为准，不再强依赖旧 tag checkout，避免修好的发布说明和流程脚本被旧 tag 覆盖
- 如果历史 Release 中残留了错误命名的旧资产（例如旧版便携包名），workflow 会在重发时先清理再上传新资产

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
7. GitHub Actions 发版时会使用 `gh release create` / `gh release upload` 创建 Release 并上传资产，不再依赖 `softprops/action-gh-release` 这类 Node 20 action。
8. `workflow_dispatch` 模式下，如果仓库里已经存在对应 `release-notes/v<version>.md`，workflow 会直接复用，不会再用空模板覆盖；只有显式传入 `release_notes` 输入时才会重写。
9. GitHub Actions 发版时会自动把 `release-notes/<tag>.md` 同步到 GitHub Release 正文；如需补写或重写，再手动执行 `npm run release:notes -- <tag> <notes-file>`。
10. 如果更新了 updater 签名密钥，同步更新仓库 Secrets：`TAURI_PRIVATE_KEY`、`TAURI_PRIVATE_KEY_PASSWORD`、`TAURI_PUBLIC_KEY`。
11. 如果使用 Gitee 回退源，确认仓库 Secrets 已配置 `GITEE_PUSH_URL`，并确认 Gitee 仓库存在 `master` 代码分支和 `updater-assets` 资产分支。
12. 确认 `scripts/build-updater-manifest.mjs` 生成的 GitHub 与 Gitee 两份 `latest.json` 都指向正确资产地址。
13. 便携版发布资产命名保持为 `JX3RaidManager_v<version>.exe`，安装版保持为 `JX3RaidManager_<version>_x64-setup.exe`。
14. GitHub Actions 在 release 成功后会自动把仓库版本推进到下一个补丁版本，并生成新的空白 `release-notes/v<next>.md` 模板，便于后续继续开发。
15. 最后用 GitHub API 或网页再次确认正文没有乱码，并确认 GitHub release 里已上传安装包、便携版、`latest.json` 与签名文件；同时确认 Gitee `updater-assets` 分支已同步 `updater/latest.json`、安装包和 `.sig` 文件。

---

## 常见问题

### Q: `gh run rerun` 后还是失败？

A: `gh run rerun` 使用的是触发时的代码版本。如果之后提交了修复，需要重新推送 tag。

### Q: latest.json 一直是旧内容？

A: 需要手动重新生成并上传：
```bash
# 1. 下载签名文件
gh release download <tag> --pattern "*.sig" --clobber

# 2. 复制到正确位置
cp JX3RaidManager_<version>_x64-setup.exe.sig src-tauri/target/release/bundle/nsis/

# 3. 重新生成 latest.json
node scripts/build-updater-manifest.mjs <tag>

# 4. 上传到 GitHub Release
gh release upload <tag> src-tauri/target/release/bundle/latest.json --clobber
```

### Q: Gitee 同步失败，提示 clobber？

A: 在 workflow 的 `Mirror code to Gitee` 步骤中，强制推送前先删除远程 tag：
```yaml
- name: Mirror code to Gitee
  shell: bash
  run: |
    set -euo pipefail
    # ... existing setup ...
    # 删除远程 tag（如果存在），避免 clobber 错误
    git push gitee :refs/tags/${GITHUB_REF_NAME} 2>/dev/null || true
    git push gitee refs/remotes/origin/master:refs/heads/master --force
    git push gitee --tags --force
```

### Q: 可以跳过某些 workflow 步骤吗？

A: 不可以。workflow 的步骤是串联的，跳过会导致后续步骤失败。但如果某个步骤失败（如 Gitee 同步），不影响 Release 本身的功能。

---

## v2.1.24 问题总结

本次发布过程中遇到的问题，确保后续不再犯：

### 1. Release Notes 描述不用户友好

**问题**：首次发布的 release notes 包含技术实现细节（"移除 game_directory.rs 中的硬编码..."），用户无法理解。

**后续措施**：
- release notes 文件中的描述应以用户价值为导向，避免技术实现细节
- 提交发布前检查 `release-notes/v*.md` 文件内容

### 2. Gitee 同步 Tag Clobber 错误

**问题**：`git push gitee --tags --force` 被拒绝，错误信息 `would clobber existing tag`。

**后续措施**：
- workflow 中添加 `git push gitee :refs/tags/${GITHUB_REF_NAME} 2>/dev/null || true` 在强制推送前先删除远程 tag
- 避免手动推送 tag，尽量让 workflow 统一处理

### 3. latest.json 生成时机错误

**问题**：更新 GitHub Release notes 后，`latest.json` 中的 notes 仍为旧内容。

**原因**：`Build updater manifest` 步骤从 tag 对应的代码生成 `latest.json`，而非从 GitHub Release API 获取。

**后续措施**：
- 更新 release notes 后，必须手动重新生成并上传 `latest.json`

### 4. Workflow Rerun 不使用最新代码

**问题**：`gh run rerun` 使用触发时的代码版本，不会使用后续提交的修复。

**后续措施**：
- 提交 workflow 修复后，需要重新推送 tag 才能使用新 workflow
- 不要依赖 rerun 来应用代码修复

### 5. 并发 Workflow 冲突

**问题**：`Prepare next patch version` 步骤因本地文件被远程覆盖而失败。

**后续措施**：
- 避免短时间内多次 rerun workflow
