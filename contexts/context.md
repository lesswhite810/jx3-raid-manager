# JX3 Raid Manager 项目上下文

## 项目概述
JX3 Raid Manager 是一个用于管理剑网3副本和账号的桌面应用程序。
主要功能包括副本管理（Raid Manager）和账号管理（Account Manager）。

## 技术栈
- **UI 框架**: React
- **样式**: Tailwind CSS
- **构建工具**: Vite
- **后端/Shell**: Tauri (Rust)
- **图标库**: Lucide React

## 关键文件
- `components/RaidManager.tsx`: 副本列表和管理，包含内联的添加副本表单。
- `components/AccountManager.tsx`: 账号列表管理。
- `components/AddAccountModal.tsx`: 用于添加账号的模态框。
- `src-tauri/src/db.rs`: 后端数据库逻辑。

## 当前 UI 状态
- **副本管理**: 使用内联表单添加副本，使用卡片展示副本列表。
- **账号管理**: 使用模态框 (`AddAccountModal`) 添加账号。
- **样式**: 使用 Tailwind CSS，包含自定义颜色（primary, surface 等）。

## 优化目标
- 优化弹窗界面（Modal），采用更现代、高级的设计风格（UI/UX Pro Max）。
- 将副本管理的内联表单改为模态框，保持界面一致性。
- 引入 Glassmorphism（毛玻璃）、悬停特效、平滑过渡等现代化元素。
