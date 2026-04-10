# 发布流程指南

本文档记录 v2.1.24 发布过程中遇到的问题，确保后续发布不再犯同样的错误。

## 发布前检查清单

在创建 GitHub Release 之前，必须确认以下事项：

- [ ] **版本号一致**：tag、package.json、src-tauri/tauri.conf.json、src-tauri/Cargo.toml 版本号必须完全一致
- [ ] **Release Notes 用户友好**：描述应以用户价值为导向，避免技术实现细节
- [ ] **无未提交的相关代码**：确认没有与发布相关的未提交更改
- [ ] **Gitee 网络连通性**：如果 Gitee 同步有问题，确保网络可以访问 gitee.com

---

## Release Notes 编写规范

### ✅ 正确示例

```markdown
# 剑网三副本管家 v2.1.24

## 问题修复

- **修复部分职业无法解析心法与装分的问题** - 统一门派心法数据源，提升数据解析的准确性

## 新增功能

- **新增角色排序功能** - 支持按装分、心法等方式排序角色列表
```

### ❌ 错误示例

```markdown
# 剑网三副本管家 v2.1.24

## 代码重构

- **统一门派心法数据源** - 移除 game_directory.rs 中的硬编码门派/心法映射，改为调用统一的 kungfu_data 模块，修复多处硬编码问题
```

**原因**：用户不关心技术实现，只关心功能改进和 bug 修复。

---

## Git Tag 管理

### 重要原则

**避免手动推送 tag，尽量让 workflow 统一处理。**

如果必须手动操作：
1. 确保本地和远程代码完全同步
2. 推送前检查 workflow 状态，避免与正在运行的 workflow 冲突

### Tag 推送后才发现问题

如果 tag 推送后需要修改，不要依赖 `gh run rerun`（它会使用触发时的代码版本）：

1. 修改代码并提交
2. 删除本地和远程 tag：
   ```bash
   git tag -d vX.Y.Z
   git push origin :refs/tags/vX.Y.Z
   ```
3. 重新推送 tag 触发新的 workflow

---

## GitHub Actions Workflow 注意事项

### 1. Gitee 同步 Tag Clobber 问题

**问题描述**：`git push gitee --tags --force` 被拒绝，错误信息 `would clobber existing tag`。

**解决方案**：在 workflow 的 `Mirror code to Gitee` 步骤中，强制推送前先删除远程 tag：

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

### 2. latest.json 生成时机

**问题描述**：更新 GitHub Release notes 后，`latest.json` 中的 notes 仍为旧内容。

**原因**：`Build updater manifest` 步骤从 tag 对应的代码（`release-notes/v*.md` 文件）生成 `latest.json`，而非从 GitHub Release API 获取。

**解决方案**：更新 release notes 后，需要手动重新生成并上传 `latest.json`：

```bash
# 1. 确保签名文件存在（从 GitHub 下载）
gh release download vX.Y.Z --pattern "*.sig" --clobber

# 2. 复制到正确位置
cp JX3RaidManager_X.Y.Z_x64-setup.exe.sig src-tauri/target/release/bundle/nsis/

# 3. 重新生成 latest.json
node scripts/build-updater-manifest.mjs vX.Y.Z

# 4. 上传到 GitHub Release
gh release upload vX.Y.Z src-tauri/target/release/bundle/latest.json --clobber
```

### 3. Workflow Rerun 不使用最新代码

**问题描述**：`gh run rerun` 使用触发时的代码版本，不会使用后续提交的修复。

**原因**：GitHub Actions 行为决定。

**解决方案**：
- 提交 workflow 修复后，必须重新推送 tag 才能使用新 workflow
- 不要依赖 rerun 来应用代码修复

### 4. 并发 Workflow 冲突

**问题描述**：`Prepare next patch version` 步骤因本地文件被远程覆盖而失败。

**解决方案**：
- 避免短时间内多次 rerun workflow
- 或修改 workflow 在 checkout 前先 stash 本地更改

---

## Gitee 同步问题

### 网络连通性

如果无法直接连接 Gitee（`fatal: unable to access 'https://gitee.com/...': Failed to connect`）：

1. 检查本地网络代理设置
2. 或手动在 Gitee 界面同步代码

### 手动同步到 Gitee

如果 workflow 中的 Gitee 同步失败，可以手动同步：

1. 访问 Gitee 仓库页面
2. 点击 "拉取请求" → "从-github/lesswhite810/jx3-raid-manager"
3. 或手动 clone 并 push 代码

---

## 发布后验证

发布完成后，确认以下内容：

- [ ] GitHub Release 页面显示正确的 notes
- [ ] latest.json 中的 notes 与 Release 一致
- [ ] 安装包可以正常下载
- [ ] 自动更新功能可以获取到新版本信息
- [ ] Gitee 同步完成（如果适用）

---

## 常见问题

### Q: `gh run rerun` 后还是失败？

A: `gh run rerun` 使用的是触发时的代码版本。如果之后提交了修复，需要重新推送 tag。

### Q: latest.json 一直是旧内容？

A: 需要手动重新生成并上传。参见上面的"latest.json 生成时机"章节。

### Q: Gitee 同步失败，提示 clobber？

A: 确保 workflow 中有删除远程 tag 的步骤。参见上面的"Gitee 同步 Tag Clobber 问题"章节。

### Q: 可以跳过某些 workflow 步骤吗？

A: 不可以。workflow 的步骤是串联的，跳过会导致后续步骤失败。但如果某个步骤失败（如 Gitee 同步），不影响 Release 本身的功能。
