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
```

这个脚本会：

1. 先通过 tag 查到对应 release id
2. 再用 UTF-8 JSON 请求更新 GitHub Release 正文
3. 避免 PowerShell 中文参数链路导致的乱码

## Release Notes 写法

Release Notes 站在用户角度写，不写“做了哪些代码改动”，而写“用户能得到什么变化”。

推荐结构：

```md
## 本次更新

用 1 段话概括这次版本对用户最重要的变化。

## 新增或优化

- 用户能直接感知到的功能变化
- 用户使用时会更方便的地方

## 修复的问题

- 用户之前遇到的问题现在如何解决
- 避免只写“修复若干问题”这种空话

## 安装包说明

- 推荐下载哪个安装包
- 便携版和安装版分别适合什么场景

## 额外说明

- 兼容性提示
- 已知限制
- 错误发版说明
```

## 写作要求

1. 先写用户结果，再写技术背景。
2. 每条说明尽量能回答“这对用户有什么影响”。
3. 不要只列内部实现词汇，例如“重构”“拆包”“校验脚本”，除非同时说明用户收益。
4. 错误发版要明确写“不要下载使用，请改用哪个版本”。

## 推荐流程

1. 先完成版本号同步。
2. 本地执行 `npm run build` 或 `npm run tauri build`。
3. 在 `release-notes/` 新增或更新对应版本说明。
4. 用 `npm run release:notes -- <tag> <notes-file>` 更新 GitHub Release Notes。
5. 最后用 GitHub API 或网页再次确认正文没有乱码。
