# AGENTS.md

本文档为 AI 编码代理提供 JX3 Raid Manager 代码库开发规范。

## 1. 核心指引

### 1.1 必读文档

- 业务与架构上下文：`contexts/context.md`
- UI 与设计规范：`specs/design-tokens.md`
- 发版、Release Notes 、防乱码流程：`docs/release-process.md`

### 1.2 工作原则

- 先理解现有实现，再修改逻辑。
- 不臆造不存在的 Tauri 命令、API 或数据结构。
- 用户可感知的行为变更，要同时补验证结果或验证路径。
- 功能、Bug、版本变更时，同步更新 `README.md`。
- 发布相关变更时，同步更新 `release-notes/*.md`。

## 2. 常用命令

```bash
npm run tauri dev
npm run build
npm run tauri build
npm run tauri:bundle
npm run test
npm run release:notes -- v2.1.6 release-notes/v2.1.6.md
```

- `npm run tauri build`：本地默认只生成可执行文件，不打安装包。
- `npm run tauri:bundle`：本地显式生成 NSIS 安装包。

## 3. 前后端通信约定

- 前端通过 Tauri `invoke` 与 Rust 后端通信。
- 前端 `services/db.ts` 对外使用 camelCase，Rust 命令使用 snake_case。
- 修改 `services/db.ts` 、目录扫描、数据删除等功能前，先确认 `src-tauri/` 中对应能力存在。
- 如需 IPC 联调，可参考 `docs/TEST_CASES.md` 中的 MCP Bridge 调用方式。

## 4. 开发规范

### 4.1 TypeScript / React

- 优先使用 `interface` 定义对象类型。
- 优先使用 `enum` 定义固定值集合。
- 严禁 `as any`、`@ts-ignore`、`@ts-expect-error`。
- 组件优先使用函数组件和命名导出。
- 列表渲染前做数组安全检查，异步加载处理空状态与失败状态。
- Hooks 、工具函数、组件命名保持与现有仓库风格一致。

### 4.2 命名规则

- 组件：PascalCase，例如 `ConfigManager.tsx`
- Hooks：camelCase 且以 `use` 开头
- 工具函数：camelCase
- 常量：UPPER_SNAKE_CASE
- Rust 命令：snake_case
- CSS 变量：kebab-case

### 4.3 本地化

- 所有面向用户的 UI 文本必须使用简体中文。
- 日志、Toast、提示文案、Release Notes 更新后，都要确认没有中文乱码。

## 5. 样式规范

### 5.1 总体视觉方向

- 保持当前扁平、简洁、本地工具型的产品风格。
- 避免引入过重的卡片阴影、强烈渐变、发光特效和不必要的装饰元素。
- 新组件或重构时，优先延续现有颜色体系和交互约定。

### 5.2 Tailwind / 语义色

- 优先使用语义化 class，例如 `bg-surface`、`text-muted`、`border-base`。
- 避免直接使用原始色值取代设计系统中的语义色。
- 如果需要新增颜色约束，优先更新 `specs/design-tokens.md`，再改 UI 实现。

### 5.3 卡片、容器与阴影

- 卡片和容器优先使用单色背景与轻边框表达层次。
- 禁止默认引入重阴影，例如 `shadow-lg`、`shadow-xl` 类大强度阴影。
- 禁止把线性渐变当作默认卡片背景方案。

### 5.4 状态颜色与标签

- 收入、正向结果、通用激活状态优先使用 `emerald` 系。
- 支出、警示、已用状态优先使用 `amber` 系。
- `red` 系仅用于删除操作、错误提示和高风险警告。
- 多选 Tag 、筛选项、状态切换组件的激活色优先统一到 `emerald` 系，不要在不同模块自由发散新颜色系统。

### 5.5 收入 / 支出图标规则

- 收入优先使用 `TrendingUp`。
- 支出优先使用 `TrendingDown`。
- 不要自行用其他箭头或圆形箭头图标替代这套语义。
- 收入图标与文本颜色优先使用 `emerald`，支出优先使用 `amber`。

### 5.6 图标克制

- 图标服务于信息传达，不要为了装饰而加入冗余图标。
- 文本已经足够清晰时，不要再额外附加无功能性图标。

### 5.7 已知样式注意事项

- 如果项目 `tailwind.config.js` 中存在自定义 `base` 颜色同名冲突，需要警惕 `text-base` 被覆盖问题。
- 当需要明确 1rem 字号且项目確实存在这类冲突时，优先使用 `text-[1rem]` 而不是 `text-base`。

## 6. 数据与错误处理

- 异步操作统一使用 `try/catch`。
- 用户可感知的失败要有中文提示。
- 保存、删除、扫描、迁移等操作要同时覆盖成功、失败和跳过路径。
- 目录扫描、账号删除、角色删除等功能更改时，要同步关注日志文案和路径兼容性。

## 7. 目录结构

- `components/`：React 组件
- `contexts/`：React Context 与上下文说明
- `hooks/`：自定义 Hooks
- `services/`：业务逻辑与数据访问
- `utils/`：纯工具函数
- `data/`：静态数据
- `docs/`：项目文档
- `release-notes/`：正式 Release Notes
- `scripts/`：发布和维护脚本
- `src-tauri/`：Rust 后端与 Tauri 配置

## 8. Git 与提交规范

- 提交信息使用中文。
- 推荐格式：`类型: 描述`。
- 示例：`feat: 新增账号目录删除能力`、`fix: 修复游戏目录兼容逻辑`、`docs: 更新发布流程说明`。
- 不要把无关的历史噪音文件状态混入当前提交。

## 9. 发版与版本规则

### 9.1 版本一致性

- 发版前必须同步 git tag、`package.json`、`src-tauri/tauri.conf.json`、`src-tauri/Cargo.toml`。
- GitHub Actions 已增加发版前版本一致性校验。

### 9.2 Release Notes

- 正式 Release Notes 存放在 `release-notes/`。
- Release Notes 必须站在用户角度书写，不要只写内部实现。
- Release Notes 只写两类信息：`新增需求`、`修复 bug`。没有对应内容的栏目不要写。
- 错误发版必须明确告知用户“不要下载使用，请改用哪个版本”。

### 9.3 发布资产

- 推荐安装版资产使用 `JX3RaidManager_<version>_x64-setup.exe`。
- 便携版资产使用 `JX3RaidManager_v<version>.exe`。
- Windows 安装器定制统一维护在 `src-tauri/nsis/hooks.nsh` 与 `src-tauri/nsis/SimpChinese.nsh`，不要额外维护整份 `installer.nsi` 模板。
- 发现历史 release 资产挂错时，先移除错资产，再在 notes 中给用户明确说明。
- 自动更新发布依赖仓库 Secrets：`TAURI_PRIVATE_KEY`、`TAURI_PRIVATE_KEY_PASSWORD`、`TAURI_PUBLIC_KEY`。
- 如果自动更新启用了 Gitee 回退源，同步配置 `GITEE_PUSH_URL`，可选配置 `GITEE_REPO`、`GITEE_ASSETS_BRANCH`。
- 自动更新发布后要确认 release 中包含 `latest.json`，不要只检查安装包和 `.sig`。
- 自动更新相关资产标签保持一致：`latest.json` 显示“自动更新元数据”，`.sig` 显示“自动更新签名文件”。

## 10. 中文防乱码规则

- 不要用 PowerShell 直接写中文文件内容。
- 不要把中文 Release Notes 直接作为 GitHub CLI 参数。
- 写中文内容时，优先使用 Node 脚本并显式指定 `utf8`。
- 更新 GitHub Release Notes 时，统一使用 `npm run release:notes -- <tag> <notes-file>`。
- 如果文件疑似乱码，先用 Node 按 UTF-8 读取确认，再决定是否修复。
- 如需生成中文内容文件，尽量使用仓库内脚本或 UTF-8 文件模板，不要现场拼接 PowerShell here-string。

## 11. 代理执行要求

1. 修改前先理解现有实现，不要臆造不存在的能力。
2. 涉及用户可感知的变更，优先补充验证结果或验证方法。
3. 发现乱码时，先区分“文件本体损坏”和“终端显示编码问题”。
4. 涉及发布流程、Release Notes 、资产更新的操作，以 `docs/release-process.md` 为准。
5. 如果修改了发布相关脚本或流程，同步更新 `AGENTS.md` 与 `docs/release-process.md`。
