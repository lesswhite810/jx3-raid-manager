# JX3 Raid Manager 项目上下文

## 项目简介
JX3 Raid Manager (剑网三副本管家) 是一个专为《剑网三》玩家设计的本地化副本数据管理工具。它基于 Electron/Tauri 技术栈（目前迁移至 Tauri），旨在提供安全、便捷的团本收入记录、账号管理和数据分析功能。

## 核心功能
1.  **概览 (Dashboard)**
    - 展示账户总览，包括总金币收入、副本记录数量。
    - 提供近期收入趋势图表。
    - 展示角色收入分布和副本收入分布图表。

2.  **账号管理 (Account Manager)**
    - 管理游戏账号（添加、编辑、删除）。
    - 关联角色与服务器信息。
    - 支持手动添加和自动扫描游戏目录识别账号。

3.  **副本管理 (Raid Manager)**
    - 记录副本通关数据，包括日期、副本名称、难度、收入等。
    - 支持查看副本详情和角色记录详情。
    - 提供副本筛选和搜索功能。

4.  **配置 (Configuration)**
    - 设置游戏路径。
    - 调整界面主题（暗色/亮色模式）。
    - 数据迁移（从本地存储迁移到 IndexedDB）。

## 技术栈
- **前端框架**: React 18, TypeScript
- **构建工具**: Vite
- **UI 库**: Tailwind CSS, Lucide React (图标)
- **图表库**: Recharts
- **本地化**: Tauri (Rust backend for system interactions)
- **数据存储**: IndexedDB (主要), LocalStorage (辅助/旧数据)

## 目录结构
- `src/components`: React 组件 (Dashboard, AccountManager, RaidManager 等)
- `src/contexts`: Context Providers (ThemeContext 等)
- `src/hooks`: Custom Hooks
- `src/services`: 业务逻辑服务 (数据库, 扫描器, AI 服务等)
- `src/types`: TypeScript 类型定义
- `src/utils`: 工具函数
- `src-tauri`: Tauri 后端配置和逻辑

## 关键业务逻辑
- **数据迁移**: 应用启动时会检查 LocalStorage 中的旧数据，并尝试迁移到 IndexedDB (`services/migration.ts`)。
- **目录扫描**: 支持扫描剑网三游戏目录，解析账号和角色信息 (`services/gameDirectoryScanner.ts`, `services/gkpDirectoryScanner.ts`)。
- **AI 辅助**: 集成 Gemini AI 进行数据分析或辅助功能 (`services/geminiService.ts`, `services/aiService.ts`)。
