# 茗伊插件集（JX3MY）深度分析文档

> 仓库地址：<https://github.com/tinymins/JX3MY>
> 分析版本：master 分支（v29.0.10，build 20260707）
> 文档目的：系统梳理茗伊插件集的源码结构、数据持久化机制、运行时生成的各类文件及其内容结构，为 JX3 Raid Manager 项目的数据采集与分析提供权威参考。

---

## 一、概述

茗伊插件集（JX3MY）是剑网3（JX3）的 Lua 辅助插件集合，由作者「茗伊」维护，遵循 BSD-3-Clause 协议开源。仓库使用 Lua（93.9%）与 Python（6.1%，构建脚本）编写，截至分析时已有 10,294 次提交、401 个 Release。

**核心定位**：侧重 PVE，遵循简单实用原则，完全遵守白名单 API，开源免费。

**主要功能**：聊天辅助、喊话辅助、聊天监控、职业染色、点数监控、截图助手、扁平血条、常用工具、快速登出、金团记账、伤害统计、角色统计、战斗事件记录等。

**安装位置**：解压到 JX3 游戏目录下的 `bin/zhcn/interface/`，插件本体目录为 `Interface/MY/`。

---

## 二、仓库结构

仓库顶层包含 30 个子插件目录（每个子目录是一个独立 addon），以及构建配置与文档：

### 2.1 子插件目录（30 个）

| 目录 | 名称 | 类别 | 是否生成数据文件 |
|------|------|------|------------------|
| `MY_!Base` | 基础库 | 核心 | ✅ 多种数据文件 |
| `MY_BagEx` | 背包扩展 | 工具 | 仅配置 |
| `MY_Cataclysm` | 大灾难 | 工具 | 仅配置 |
| `MY_Chat` | 聊天辅助 | 聊天 | ✅ chatmonitor.jx3dat |
| `MY_ChatLog` | 聊天记录 | 聊天 | ✅ chatlog_*.v2.db |
| `MY_CombatText` | 战斗文本 | 战斗 | 仅配置 |
| `MY_Farbnamen` | 彩色名字 | UI | 仅配置 |
| `MY_Focus` | 焦点 | UI | 仅配置 |
| `MY_Font` | 字体 | UI | 仅配置 |
| `MY_FontResource` | 字体资源 | UI | 仅配置 |
| `MY_Force` | 门派 | UI | 仅配置 |
| `MY_GKP` | 金团记录 | PVE | ✅ *.gkp.jx3dat |
| `MY_LifeBar` | 头顶血条 | UI | 仅配置 |
| `MY_Logoff` | 快速登出 | 工具 | 仅配置 |
| `MY_MiddleMapMark` | 地图标记 | UI | 仅配置 |
| `MY_Recount` | 伤害统计 | 战斗 | ✅ *.fstt.jx3dat |
| `MY_Resource` | 资源 | UI | 仅配置 |
| `MY_RoleStatistics` | 角色统计 | PVE | ✅ role_stat / task_stat / equip_stat.v4.db |
| `MY_RollMonitor` | 摇点监控 | 工具 | 仅配置 |
| `MY_ScreenShot` | 截图助手 | 工具 | 仅配置（截图本身为图片） |
| `MY_Target` | 目标辅助 | UI | 仅配置 |
| `MY_TargetMon` | 目标监控 | 战斗 | 仅配置 |
| `MY_TeamMon` | 团队监控 | PVE | 仅配置（订阅数据除外） |
| `MY_TeamTools` | 团队工具 | PVE | ✅ *.jcl 战斗日志 |
| `MY_ThreatRank` | 仇恨排名 | 战斗 | 仅配置 |
| `MY_Toolbox` | 工具箱 | 工具 | 仅配置（含 37 个子模块） |
| `MYDev_Snaplines` | 开发-连线 | 开发 | 仅配置 |
| `MYDev_UIEventID` | 开发-UI事件ID | 开发 | 仅配置 |
| `MYDev_UITexViewer` | 开发-贴图查看 | 开发 | 仅配置 |
| `MYDev_VarWatch` | 开发-变量监视 | 开发 | 仅配置 |

### 2.2 构建/配置文件

| 文件 | 用途 |
|------|------|
| `!src-dist/` | 构建产物目录 |
| `.github/` | CI 工作流（Auto Tag、Release） |
| `.7zipignore` / `.7zipignore-classic` / `.7zipignore-remake` | 打包忽略列表（正式服/缘起/重制版） |
| `.emmyrc.json` | EmmyLua IDE 配置 |
| `.luacheckrc` | Lua 静态检查配置 |
| `.travis.yml` | 旧版 Travis CI |
| `package.ini` / `package.ini.zh_TW` | 打包元信息 |
| `CHANGELOG.md` | 更新日志 |
| `LICENSE` | BSD-3-Clause 许可证 |
| `README.md` | 项目说明 |

### 2.3 第三方依赖

均采用 MIT/zlib 许可：`lua-schema`、`semver.lua`、`basexx`、`uuid`（RFC 4122）、`LibDeflate`（zlib）。

---

## 三、核心基础库 MY_!Base

`MY_!Base` 是所有子插件的依赖，提供命名空间 `MY`（即 `X`）、数据持久化、路径管理、加密、SQLite、JSON、压缩、UUID、Schema 校验等基础能力。`info.ini` 中定义了 116 个 Lua 文件（`lua_0` … `lua_115`）的严格加载顺序。

### 3.1 目录结构

```
MY_!Base/
├── audio/          # 音频资源
├── data/           # 静态数据（bookfix/bosslist/colors/inpclist/serendipities）
├── img/            # 图片资源
├── lang/           # 多语言包（default/zhcn/zhtw）
├── licenses/       # 第三方许可证
├── src/            # 源码（核心）
│   ├── lib/        # 核心库
│   └── ...
├── ui/             # UI 组件
└── info.ini        # 加载清单
```

### 3.2 关键源码文件

| 序号 | 文件 | 作用 |
|------|------|------|
| lua_1 | `src/lib/Base.lua` | 命名空间初始化、`PACKET_INFO`/`SECRET`/`ENVIRONMENT`/`DATA_ROOT` |
| lua_3 | `src/lib/BaseAPI.lua` | `X.EncodeLUAData`(`var2str`)/`X.DecodeLUAData`(`str2var`) |
| lua_7 | `src/lib/Deflate.lua` | zlib 压缩 |
| lua_11 | `src/lib/Uuid.lua` | RFC 4122 v4 UUID 生成 |
| lua_12 | `src/lib/Json.lua` | JSON 编解码 |
| lua_13 | `src/lib/BaseXX.lua` | Base64/Base32 编码 |
| lua_15 | `src/lib/String.lua` | 加密函数群（`X.EncryptString`/`X.KGUIEncrypt`/`X.SimpleEncryptString`） |
| lua_21 | `src/lib/Storage.SQLite.lua` | SQLite 封装（含 malformed 自动修复） |
| lua_22 | `src/lib/Storage.Path.lua` | 路径格式化、`info.jx3dat` 写入、目录创建 |
| lua_23 | `src/lib/Storage.LUAData.lua` | **★ jx3dat 持久化核心（passphrase/manifest 管理）** |
| lua_24 | `src/lib/Storage.RemoteStorage.lua` | 游戏官方位级同步 + 云存储（push-storage.j3cx.com） |
| lua_25 | `src/lib/Storage.UserSettings.lua` | 用户设置系统（基于 Schema 声明式持久化） |
| lua_36 | `src/lib/System.InfoCache.lua` | 分段键值缓存（按 key 前缀分桶存多 .jx3dat） |

### 3.3 数据根目录

源码位置：`MY_!Base/src/lib/Base.lua`

```lua
local _DATA_ROOT_ = (_GAME_PROVIDER_ == 'remote'
    and (GetUserDataFolder() .. '/' .. GetUserAccount() .. '/interface/')
    or _INTERFACE_ROOT_) .. _NAME_SPACE_ .. '#DATA/'
```

| 运行模式 | DATA_ROOT |
|----------|-----------|
| 本地（local） | `<JX3>/bin/zhcn_hd/interface/MY#DATA/` |
| 云端（remote） | `<GetUserDataFolder()>/<账号>/interface/MY#DATA/` |

> **关键**：数据目录名是 `MY#DATA`（`#` 是 JX3 数据目录约定符），与插件安装目录 `Interface/MY/` 平级、相互独立。

### 3.4 PATH_TYPE 路径体系

源码位置：`MY_!Base/src/lib/Storage.Path.lua` 的 `X.FormatPath`

| PATH_TYPE | 目录模板 | 用途 |
|-----------|----------|------|
| `NORMAL`(0) / `DATA`(1) | `<DATA_ROOT>/<rel>` | 通用 |
| `ROLE`(2) | `<DATA_ROOT>/{$uid}@{$edition}/<rel>` | **角色私有数据** |
| `GLOBAL`(3) | `<DATA_ROOT>/!all-users@{$edition}/<rel>` | 全角色共享 |
| `SERVER`(4) | `<DATA_ROOT>/#{$server_origin}@{$edition}/<rel>` | 服务器共享 |

路径占位符：`{$uid}`（角色 GlobalID）、`{$name}`、`{$lang}`、`{$edition}`（如 `zhcn_hd`）、`{$branch}`、`{$version}`、`{$server}`、`{$server_origin}`、`{$date}` 等。

> 旧版路径使用 `@{$lang}`（如 `@zhcn`），`Storage.Path.lua` 内置自动迁移到 `@{$edition}` 命名。

---

## 四、jx3dat 文件格式与加密机制

### 4.1 三层加密架构

```
┌─────────────────────────────────────────────────────────┐
│  顶层：X.SaveLUAData / X.LoadLUAData（MY_!Base 包装）     │
│  ─ 自动调用 GetLUADataPathPassphrase 注入 passphrase     │
│  ─ 调用游戏全局 SaveLUAData/LoadLUAData                   │
├─────────────────────────────────────────────────────────┤
│  中层：manifest.jx3dat（每个 domain 一份）                 │
│  ─ 存储 {相对路径 → UUID} 映射                            │
│  ─ 自身用 szPassphraseSalted 加密                         │
├─────────────────────────────────────────────────────────┤
│  底层：游戏引擎 XOR/流密码（不在仓库内）                   │
│  ─ 用 passphrase 作 key 对序列化字节流做 XOR              │
│  ─ info.jx3dat 用 passphrase=false → 等价 key 全 0 → 明文 │
└─────────────────────────────────────────────────────────┘
```

**关键认知**：`.jx3dat` 的底层二进制格式与 XOR/流密码算法由 JX3 游戏引擎内置的全局函数 `SaveLUAData` / `LoadLUAData` 实现，**不在 JX3MY 仓库内**。MY_!Base 通过 `X.SaveLUAData` / `X.LoadLUAData`（带 `X.` 前缀）包装并附加 passphrase 管理机制。

### 4.2 序列化器

源码位置：`MY_!Base/src/lib/BaseAPI.lua`

```lua
X.EncodeLUAData = _G.var2str           -- 序列化（游戏内置）
X.DecodeLUAData = _G.str2var or function(szText)  -- 反序列化
    -- 兜底实现：写入临时 .jx3dat 再 LoadLUAData 读回
end
```

- `luatext` 编码器的产物就是 `var2str` 输出的 Lua 文本（类似 `return { ["id"]=123, ["name"]="xxx" }`），可直接 `loadstring` 反序列化。
- 任意明文 luatext 内容写入 .jx3dat 后，`LoadLUAData` 都能按明文读回。

### 4.3 默认 passphrase 生成算法

源码位置：`MY_!Base/src/lib/Storage.LUAData.lua`

```lua
local function GetPassphrase(nSeed, nLen)
    local a, b, c = {}, 0x20, 0x7e - 0x20 + 1  -- 可见 ASCII 范围 [0x20, 0x7e]
    for i = 1, nLen do
        table.insert(a, ((i + nSeed) % 256 * (2 * i + nSeed) % 32) % c + b)
    end
    return string.char(X.Unpack(a))
end
local szPassphrase = GetPassphrase(666, 233)  -- 硬编码种子 666，长度 233 字节
```

- 种子 `666`、长度 `233` 是硬编码常量，所有客户端一致。
- 产物是 233 字节的可见 ASCII 字符串，确定性、可复算。
- 用于加密 `manifest.jx3dat`。

### 4.4 加盐 passphrase

```lua
local szPassphraseSalted = X.SECRET['@@LUA_DATA_MANIFEST_SALT@@']
    and (X.KGUIEncrypt(X.SECRET['@@LUA_DATA_MANIFEST_SALT@@']) .. szPassphrase)
    or szPassphrase
```

- `X.SECRET` 来自 `Interface/MY/secret.jx3dat`（只读，`Base.lua` 通过 `LoadLUAData(_ADDON_ROOT_ .. 'secret.jx3dat')` 加载）。
- 公开发布版（无 `secret.jx3dat`）的 manifest 密钥 = `GetPassphrase(666, 233)`（公开可算）。

### 4.5 X.KGUIEncrypt 多层混淆

源码位置：`MY_!Base/src/lib/String.lua`

```lua
function X.KGUIEncrypt(szText)
    if EncodeData then
        szText = EncodeData(X.SimpleEncryptString(szText)) or szText
    end
    if KGUIEncrypt then
        szText = KGUIEncrypt(X.SimpleEncryptString(szText)) or szText
    end
    return MD5 and string.lower(MD5(X.SimpleEncryptString(szText)))
           or X.SimpleEncryptString(szText)
end
```

`X.SimpleEncryptString` 是 Caesar 位移 +13 后 Base64（URL-safe，`/`→`-`、`+`→`_`、`=`→`.`）。

### 4.6 每文件 UUID 密钥机制

源码位置：`Storage.LUAData.lua` 的 `GetLUADataPathPassphrase(szPath)`

完整流程：
1. 路径小写化，校验必须位于 `szDataRoot` 下
2. 拆分出 `szDomain`（路径第一段，含尾 `/`）和剩余相对路径
3. **过滤**：若剩余路径首段为 `export`，返回 `nil`（不加密 → 明文）
4. 查找/创建 `<dataRoot>/<szDomain>/manifest.jx3dat`：
   - 先用 `szPassphraseSalted` 加载，失败则用 `szPassphrase`，再失败用空表
   - 若该路径尚无 UUID，**生成新 UUID**（`X.GetUUID():gsub('-', '')`，去掉横杠的 32 位 hex），写入 manifest 并保存
5. 返回该 UUID 作为此文件的 passphrase

### 4.7 新文件「先明文后加密」特殊逻辑

```lua
function X.LoadLUAData(oFilePath, tConfig)
    if X.IsNil(config.passphrase) then
        szPassphrase, bNew = GetLUADataPathPassphrase(szFilePath)
        if not bNew then
            config.passphrase = szPassphrase  -- 已有文件：用 UUID 解密
        end
        -- bNew=true 时 config.passphrase 保持 nil → 以明文模式加载
    end
    local data = LoadLUAData(szFilePath, config)
    if bNew and data then
        config.passphrase = szPassphrase
        SaveLUAData(szFilePath, data, config)  -- 立即用新 UUID 重新加密落盘
    end
end
```

> 含义：新生成的数据文件**首次落地是明文**，随即在同一调用中被改写为 UUID 加密的密文。理论上若抓取到「刚创建瞬间」的文件可读到明文。

### 4.8 明文 vs 加密文件清单

| 文件类型 | encoder | crc | passphrase | 实际状态 |
|----------|---------|-----|------------|----------|
| `info.jx3dat` | `luatext` | `false` | **`false`** | **完全明文**（Lua 文本） |
| `manifest.jx3dat` | 默认二进制 | 默认 | `szPassphraseSalted` | **加密** |
| 普通业务数据 `*.jx3dat` | 默认二进制 | 默认 | 每文件 UUID | **加密** |
| `export/**.jx3dat` | 默认 | 默认 | **`nil`** | **明文**（导出用） |
| `debug.level.jx3dat` / `log.level.jx3dat` | - | - | 顶层文件，`GetLUADataPathPassphrase` 返回 nil | 明文 |
| `secret.jx3dat` | - | - | 由 `Base.lua` 直接 `LoadLUAData` 读取（位于 ADDON_ROOT，非 DATA_ROOT） | - |
| `no.runtime.optimize.jx3dat` | - | - | 同上，存在性检测 | - |
| `achievement_acquire_shot.jx3dat` | - | - | 实测明文 | **明文** |

> 关于「info.jx3dat XOR key 全 0」的准确表述：源码层面是 `passphrase = false`（显式禁用加密），等价于 XOR key 为空/全 0，效果上 info.jx3dat 就是明文 Lua 文本，可直接打开阅读。

---

## 五、运行时生成的数据文件详解

### 5.1 info.jx3dat（角色目录元数据，明文）

**生成模块**：`MY_!Base/src/lib/Storage.Path.lua` 的 `X.CreateDataRoot(ePathType)`

**文件路径**：`<DATA_ROOT>/{$uid}@{$edition}/info.jx3dat`（PATH_TYPE.ROLE）

**写入时机**：角色登录初始化阶段调用 `X.CreateDataRoot(X.PATH_TYPE.ROLE)` 时（通过 `CREATED[ePathType]` 标志位保证每 PATH_TYPE 只执行一次）。

**写入代码**：

```lua
X.SaveLUAData(
    {'info.jx3dat', X.PATH_TYPE.ROLE},
    {
        id            = X.GetClientPlayerInfo().dwID,
        uid           = X.GetClientPlayerGlobalID(),
        name          = X.GetClientPlayerInfo().szName,
        lang          = X.ENVIRONMENT.GAME_LANG,
        edition       = X.ENVIRONMENT.GAME_EDITION,
        branch        = X.ENVIRONMENT.GAME_BRANCH,
        version       = X.ENVIRONMENT.GAME_VERSION,
        region        = X.GetRegionName(),
        server        = X.GetServerName(),
        region_origin = X.GetServerOriginName(),
        server_origin = X.GetRegionOriginName(),
        time          = GetCurrentTime(),
        time_str      = X.FormatTime(GetCurrentTime(), '%yyyy%MM%dd%hh%mm%ss'),
    },
    { encoder = 'luatext', crc = false, passphrase = false }
)
```

**字段含义**：

| 字段 | 含义 |
|------|------|
| `id` | 角色 dwID（数字） |
| `uid` | `GetClientPlayerGlobalID()`（玩家全局 ID，用作目录名 `{$uid}`） |
| `name` | 角色名 |
| `lang` | 游戏语言（zhcn / zhtw） |
| `edition` | 发行版（如 `zhcn_hd`、`zhcn_remake`、`zhcn_yq`） |
| `branch` | 运营分支（remake / intl / classic） |
| `version` | 游戏版本号 |
| `region` / `server` | 大区 / 服务器显示名 |
| `region_origin` / `server_origin` | 数据互通主大区 / 主服务器 |
| `time` | 写入时间戳（GetCurrentTime，秒） |
| `time_str` | 写入时间字符串 |

**实测样例**（GBK 编码的 Lua return 语句）：

```lua
return {region="电信五区",uid="432345564243886337",name="角色名",server="梦江南",...}
```

**重要特性**：
- 只在登录时写入一次，退出时不更新（不能单独作为「当前在线」判定依据，需配合 mtime）。
- 明文可直接读取，无需解密——是多账号/多角色目录识别的最佳入口。
- 同函数还会创建 `temporary/{$version}/`、`audio/`、`cache/`、`config/`、`export/`、`logs/`、`font/`、`userdata/` 子目录。

### 5.2 manifest.jx3dat（密钥清单，加密）

**生成模块**：`MY_!Base/src/lib/Storage.LUAData.lua`

**文件路径**：`<DATA_ROOT>/<szDomain>/manifest.jx3dat`

按 PATH_TYPE 展开后：

| PATH_TYPE | 完整路径示例 |
|-----------|--------------|
| ROLE | `Interface/MY#DATA/{$uid}@{$edition}/manifest.jx3dat` |
| GLOBAL | `Interface/MY#DATA/!all-users@{$edition}/manifest.jx3dat` |
| SERVER | `Interface/MY#DATA/#{$server_origin}@{$edition}/manifest.jx3dat` |

**数据结构**：

```lua
{
    ["info.jx3dat"] = "a1b2c3d4e5f6a7b8c9d0e1f2a3b4c5d6",  -- 32 字符 hex（去横杠 UUID）
    ["config/foo.jx3dat"] = "e1f2a3b4c5d6a7b8c9d0e1f2a3b4c5d6",
    ["cache/bar.jx3dat"] = "...",
}
```

- **键**：相对 domain 的路径（已小写化）
- **值**：32 字符 hex 字符串（无横杠 UUID），作为该文件的加密 passphrase

**生成时机**：首次 `X.SaveLUAData` / `X.LoadLUAData` 访问某 domain 下任意文件时按需创建。新 UUID 通过 `X.GetUUID():gsub('-', '')` 生成。manifest 自身用 `szPassphraseSalted`（或回退 `szPassphrase`）加密保存。

**进程内缓存**：`CACHE[szDomain]` 避免反复读 manifest。

### 5.3 chatlog_*.v2.db（聊天记录，SQLite）

**生成模块**：`MY_ChatLog`（`MY_ChatLog_DB.lua` + `MY_ChatLog_DS.lua` + `MY_ChatLog.lua`）

**文件路径**：`<DATA_ROOT>/{$uid}@{$edition}/userdata/chat_log/chatlog_XXXXXX.v2.db`

#### 5.3.1 表结构

**表 ChatInfo**（节点元数据，键值对）：

```sql
CREATE TABLE IF NOT EXISTS ChatInfo (
    key   NVARCHAR(128)  NOT NULL,
    value NVARCHAR(4096) NOT NULL,
    PRIMARY KEY (key)
)
```

已知 key（value 经 `X.EncodeLUAData` 序列化）：

| key | 含义 |
|-----|------|
| `version` | 数据库版本号（`'2'`） |
| `user_global_id` | 所属玩家全局 ID（跨角色隔离） |
| `min_time` | 该分片覆盖的最小时间戳 |
| `max_time` | 该分片覆盖的最大时间戳（`0` 表示 `math.huge` 无穷大，即活跃节点） |

**表 ChatLog**（消息主体）：

```sql
CREATE TABLE IF NOT EXISTS ChatLog (
    hash   INTEGER        NOT NULL,
    type   VARCHAR(64)    NOT NULL,
    time   INTEGER        NOT NULL,
    talker NVARCHAR(20)   NOT NULL,
    text   NVARCHAR(400)  NOT NULL,
    msg    NVARCHAR(4000) NOT NULL,
    PRIMARY KEY (time, hash)
)
```

索引（3 个）：

```sql
CREATE INDEX IF NOT EXISTS ChatLog_type_idx   ON ChatLog(type)
CREATE INDEX IF NOT EXISTS ChatLog_talker_idx ON ChatLog(talker)
CREATE INDEX IF NOT EXISTS ChatLog_text_idx   ON ChatLog(text)
```

| 列名 | 类型 | 含义 |
|------|------|------|
| `hash` | INTEGER | 消息内容哈希，`GetStringCRC(szMsg)` 计算（对原始 ANSI szMsg 求 CRC） |
| `type` | VARCHAR(64) | 频道/消息类型字符串常量，如 `MSG_WHISPER`、`MSG_PARTY`、`MSG_MY_MONITOR` |
| `time` | INTEGER | 消息时间戳，`GetCurrentTime()`（游戏运行时间秒数，非真实世界时间） |
| `talker` | NVARCHAR(20) | 发言者名称，已转 UTF-8（`AnsiToUTF8`） |
| `text` | NVARCHAR(400) | 纯文本内容（`GetPureText` 去富文本），已转 UTF-8 |
| `msg` | NVARCHAR(4000) | 完整富文本 UI XML（`GetFormatText` 格式化），已转 UTF-8 |

主键 `(time, hash)`，写入用 `REPLACE INTO`，同时间同 hash 会被覆盖去重。

#### 5.3.2 文件名 hash 与分片机制

**文件名 hash**：来自 `MY_ChatLog_DS.lua` 的 `NewDB()`：

```lua
local function NewDB(szRoot, nMinTime, nMaxTime)
    local szPath
    repeat
        szPath = szRoot .. ('chatlog_%x'):format(X.Random(0x100000, 0xFFFFFF)) .. '.v2.db'
    until not IsLocalFileExist(szPath)
end
```

- `X.Random(0x100000, 0xFFFFFF)` 生成随机整数，`%x` 格式化为小写十六进制，由于下限 `0x100000`，结果恒为 **6 位 hex**。
- 文件名形如 `chatlog_e55e49.v2.db`，与内容无关，仅作随机唯一标识。
- 加载时用正则 `^chatlog_[0-9a-f][0-9a-f][0-9a-f][0-9a-f][0-9a-f][0-9a-f]%.v2%.db$` 匹配。
- 真正的内容哈希是消息级 `GetStringCRC(szMsg)`，存在 `ChatLog.hash` 列。

**分片策略**（按条数触发，按时间区间划分）：

```lua
local SINGLE_DB_AMOUNT = 20000  -- 单个数据库节点最大数量
```

- 集群是一组按 `[min_time, max_time]` 时间区间排序的 db 节点数组 `self.aDB`。
- 最新（活跃）节点的 `max_time = math.huge`（无穷大），其余节点 `max_time` 为固定值。
- 当活跃节点记录数 `> 20000` 时，把它的 `max_time` 收敛为实际最大记录时间，并 `NewDB()` 创建新活跃节点。
- `OptimizeDB()` 还会做：超限节点再拆分、过小节点合并、`VACUUM` 压缩、修复时间区间不连续/冲突。

#### 5.3.3 条数上限（正式服）

`MY_ChatLog.lua` 的 `LOG_LIMIT`（仅正式服 + 非 Debug 客户端生效）：

```lua
local LOG_LIMIT = {
    { aKey = {'whisper'},                      nLimit = 5000 },
    { aKey = {'party', 'team', 'room'},        nLimit = 5000 },
    { aKey = {'friend'},                       nLimit = 5000 },
    { aKey = {'guild', 'guild_a'},             nLimit = 1000 },
    { aKey = {'death', 'journal'},             nLimit = 1000 },
    { aKey = {'monitor'},                      nLimit = 1000 },
}
```

#### 5.3.4 默认记录频道

| szKey | 标题 | 记录的 MSG 类型 |
|-------|------|----------------|
| `whisper` | 密聊 | `MSG_WHISPER`, `MSG_SSG_WHISPER` |
| `party` | 队伍 | `MSG_PARTY` |
| `team` | 团队 | `MSG_TEAM` |
| `room` | 房间 | `MSG_ROOM` |
| `friend` | 好友 | `MSG_FRIEND` |
| `guild` | 帮会 | `MSG_GUILD` |
| `guild_a` | 帮会联盟 | `MSG_GUILD_ALLIANCE` |
| `death` | 死亡日志 | `MSG_SELF_DEATH`, `MSG_SELF_KILL`, `MSG_PARTY_DEATH`, `MSG_PARTY_KILL` |
| `journal` | 收支日志 | 默认仅 `MSG_MONEY`, `MSG_ITEM`（`MSG_EXP`/`MSG_REPUTATION` 等被注释掉） |
| `monitor` | 茗伊监控 | `MSG_MY_MONITOR` |

**默认不记录**（重要）：
- `MSG_SYS`（系统滚动消息）—— **确认不记录**
- `MSG_NORMAL`（近聊/说）、`MSG_CAMP`（阵营）、`MSG_WORLD`（世界）、`MSG_MAP`（地图）、`MSG_SCHOOL`（门派）、`MSG_IDENTITY`（身份）等公共频道

> 用户可在设置里增删频道（`aChannel` 可配置），但默认集合只覆盖私域频道 + 死亡 + 收支 + 监控。

#### 5.3.5 旧版本兼容

`MY_ChatLog.lua` 中 `D.MigrateDB()` / `D.ImportDB()` 处理两类旧结构：
- **v0**：单文件 `userdata/chat_log.db`（角色目录下），迁移后改名 `.bak{时间}`。
- **v1**：集群 `chatlog_XXXXXX.db`（无 `.v2`），旧表结构含 `ChatLogInfo`（`key=userguid`）、`ChatLogIndex`（`name/stime/etime`）、按 `name` 命名的分表，列用数字 `channel` 字段。`V1_MSG_TYPE_MAP` 把旧数字 channel 映射回字符串 MSG 类型（共 23 种）。

#### 5.3.6 编码与时间戳

- 库内 `talker/text/msg` 均以 **UTF-8** 存储（写入 `AnsiToUTF8`，读取按需 `UTF8ToAnsi`）。
- 时间戳统一用 `GetCurrentTime()`（游戏内运行时间秒，非真实世界时间）。
- 写入流程：`DS:InsertMsg` → 计算 `szHash = GetStringCRC(szMsg)` → UTF-8 转换 → 入队 → `DS:FlushDB` 按时间路由到对应节点 db → `db:Flush()` 在事务里批量 `REPLACE INTO`。

### 5.4 chatmonitor.jx3dat（聊天监控记录，非强加密）

**生成模块**：`MY_Chat/src/MY_ChatMonitor.lua`

**文件路径**：
- 默认：`<DATA_ROOT>/{$uid}@{$edition}/userdata/chatmonitor.jx3dat`（PATH_TYPE.ROLE）
- 「分服独立」`bDistinctServer` 开启时：`<DATA_ROOT>/#{$server_origin}@{$edition}/userdata/chatmonitor.jx3dat`（PATH_TYPE.SERVER）

旧版配置文件：`<GLOBAL>/config/chatmonitor.jx3dat`（仅作兼容迁移，迁移后 `CPath.DelFile` 删除）。

**文件格式**：JX3 框架的标准 `X.SaveLUAData` / `X.LoadLUAData` 序列化的 Lua 表，**非强加密**（JX3 框架的序列化编码，可被同框架 `LoadLUAData` 直接读回）。

**数据结构**：

```lua
{
    list = RECORD_LIST,  -- 数组：监控记录
    hash = RECORD_HASH,  -- 哈希表：去重计数
}
```

**RECORD_LIST 元素结构**：

| 字段 | 含义 |
|------|------|
| `text` | 消息纯文本（用于关键字匹配） |
| `hash` | 去重哈希字符串（系统消息=纯文本；非系统消息=去掉「XX说：」前缀后的文本，`gsub` 去空白） |
| `html` | 消息源 UI XML（富文本序列化值） |
| `fuzzy_text` | 小写化的「`[频道名]\t` + text」，用于模糊匹配 |
| `r`,`g`,`b` | 消息颜色 |
| `font` | 字体 ID |
| `time` | `GetCurrentTime()` 时间戳 |

**RECORD_HASH**：`[hash] = 捕获次数`，用于 `bIgnoreSame` 去重；为 0 时删除该键。

**默认监控频道**（`DEFAULE_CHANNEL`，注意源码拼写如此）：

```lua
local DEFAULE_CHANNEL = {
    ['MSG_NORMAL'] = true, ['MSG_CAMP'] = true, ['MSG_WORLD'] = true, ['MSG_MAP'] = true,
    ['MSG_SCHOOL'] = true, ['MSG_GUILD'] = true, ['MSG_FRIEND'] = true, ['MSG_IDENTITY'] = true,
}
```

> 与 MY_ChatLog 不同，**MY_ChatMonitor 默认监控的是公共频道**。`MSG_SYS` 不在默认集合内，但代码对 `MSG_SYS` 做了专门处理（系统消息直接用全文做 hash）。

**触发逻辑**：仅捕获匹配关键字的消息。命中后通过 `OutputMessage('MSG_MY_MONITOR', szMsg, ...)` 重新广播到 `MSG_MY_MONITOR` 频道，进而被 MY_ChatLog 持久化到 SQLite。

**保存时机**：
- `D.Init()`（用户设置初始化时）：`D.LoadData()` 读盘
- `D.Exit()`（插件退出/重载）：`D.SaveData()` 存盘
- `X.RegisterUserSettingsRelease`：置 `bReady=false`（不存盘，只停采集）
- **不实时写盘**（实时保存代码已注释），仅在退出时落盘；运行期数据驻留内存。
- `nMaxRecord` 默认 30，超过则从头弹出。

### 5.5 *.gkp.jx3dat（金团记录，加密）

**生成模块**：`MY_GKP`（`MY_GKP_DS.lua` 数据源类 + `MY_GKP_MI.lua` 主实例化 + `MY_GKP.lua` 主模块）

**文件路径**：`<DATA_ROOT>/{$uid}@{$edition}/userdata/gkp/`

| 文件 | 路径 | 含义 |
|------|------|------|
| 当前进行中 | `current.gkp.jx3dat` | 所有拍卖/收钱操作实时写入 |
| 历史归档 | `YYYY-MM-DD-HH-MM-SS[-i]_<GKP_Map>.gkp.jx3dat` | 进入秘境或手动清空时归档 |

**加密状态**：茗伊 jx3dat 加密格式（`X.SaveLUAData` / `X.LoadLUAData`）。

#### 5.5.1 数据结构

```lua
{
  GKP_Map    = '',          -- 地图名（Table_GetMapName 返回值，如 "25人英雄阆风悬城"）
  GKP_Time   = 0,           -- 记录创建时间戳（GetCurrentTime，秒）
  GKP_Record = {},          -- 拍卖记录数组（收入/罚款/补贴）
  GKP_Account = {},         -- 收钱记录数组（玩家付款/退款）
}
```

**GKP_Record 元素结构**（拍卖记录）：

```lua
{
  key         = X.GetUUID(),      -- 唯一标识
  nTime       = GetCurrentTime(), -- 记录时间
  szPlayer    = '',               -- 购买者角色名
  dwForceID   = 0,                -- 门派 ID
  nMoney      = 0,                -- 金额（金，正数=拍卖收入/罚款，负数=宴席补贴等）
  bDelete     = false,            -- 是否已删除
  bSystem     = false,            -- 是否系统金团同步
  bSync       = false,            -- 是否同步而来
  bEdit       = false,            -- 是否编辑过
  -- 物品相关
  dwIndex     = 0,                -- 物品 dwIndex
  dwTabType   = 0,                -- 物品 dwTabType
  nQuality    = 0,                -- 品质
  nVersion    = 0,                -- 版本
  nGenre      = 0,                -- 物品类型（ITEM_GENRE.BOOK 等）
  szName      = '',               -- 物品名
  nBookID     = nil,              -- 秘典 ID（仅 BOOK 类型）
  nStackNum   = 0,                -- 堆叠数
  dwDoodadID  = 0,                -- 分配物件 ID
  nUiId       = 0,
  szNpcName   = '',               -- 来源 NPC 名
}
```

**GKP_Account 元素结构**（收钱记录）：

```lua
{
  key         = X.GetUUID(),
  nTime       = GetCurrentTime(),
  szPlayer    = '',               -- 交易对端名（'System' 表示系统交易）
  dwForceID   = 0,
  nGold       = 0,                -- 金额（金，正数=收入，负数=支出）
  dwMapID     = 0,                -- 地图 ID
}
```

#### 5.5.2 文件命名规则

**历史归档命名**：

```
YYYY-MM-DD-HH-MM-SS[-i]_<GKP_Map>.gkp.jx3dat
```

- 时间戳：`X.FormatTime(GetCurrentTime(), '%yyyy-%MM%dd-%hh-%mm-%ss')`
- `<GKP_Map>`：`Table_GetMapName(mapID)` 返回值（如 `25人英雄阆风悬城`）
- `-i`：同时间戳冲突时的序号（i=0 时省略，从 1 开始递增）

**文件名解析正则**（来自 jx3-raid-manager 的 `game_directory.rs` `parse_gkp_file_name`）：

```
^(\d{4}-\d{2}-\d{2}-\d{2}-\d{2}-\d{2})_(.+?)人(.+)\.gkp\.jx3dat$
```

样本：`2026-07-12-14-52-51-25人普通阆风悬城(794)-笑妆娘.gkp.jx3dat`

#### 5.5.3 生成时机

- **当前文件**：`MY_GKP_MI.D.Init()` 在插件初始化时打开，所有拍卖/收钱操作实时写入。`DS:DelaySaveData()` 下一帧异步存盘，避免频繁 IO。
- **历史归档**触发：
  1. 进入秘境地图时弹窗确认（`LOADING_END` 事件 + `MY_GKP.bAlertMessage`）
  2. 手动清空数据
- 归档条件：`current.gkp.jx3dat` 非空且 `GKP_Time`、`GKP_Map` 均有效时，复制到历史文件，再清空当前文件。
- **历史数量限制**：`D.LimitHistoryFile()` 保留最近 **22 个**历史文件（`for i = 22, #aFiles do DelFile`）。

#### 5.5.4 同步机制

- `D.SyncSend(dwID)` 通过团队频道 `RAID` 发送 `MY_GKP_SYNC_START` + `MY_GKP_SYNC_CONTENT_<key>` 消息。
- 接收方在 `MY_GKP.bAutoSync` 开启时自动覆盖。
- 兼容第三方 `LR_GKP` 协议（`ON_BG_CHANNEL_MSG` 事件，`SYNC`/`DEL` 操作）。

#### 5.5.5 金钱变动事件处理

`D.MoneyUpdate(nGold,nSilver,nCopper)`：
- < 1 金不记
- 系统交易 < 10 金不记

### 5.6 role_stat.jx3dat（角色统计，加密）

**生成模块**：`MY_RoleStatistics/src/MY_RoleStatistics_RoleStat.lua`

**文件路径**：
- 全局汇总：`<DATA_ROOT>/!all-users@{$edition}/userdata/role_statistics/role_stat.jx3dat`（PATH_TYPE.GLOBAL）
- 单角色快照：`<DATA_ROOT>/{$uid}@{$edition}/userdata/role_statistics/role_stat.jx3dat`（PATH_TYPE.ROLE）

**加密状态**：茗伊 jx3dat 加密格式。

**数据结构**（BLOB 顶层为 `{ d=<数据 table>, v=<版本号 string> }`）：

```lua
{
  -- 标识字段
  guid              = '',     -- 角色全局 ID
  account           = '',     -- 账号名
  region            = '',     -- 大区
  server            = '',     -- 服务器
  name              = '',     -- 角色名
  force             = 0,      -- 门派 ID（dwForceID）
  camp              = 0,      -- 阵营
  level             = 0,      -- 等级

  -- 装分/积分
  equip_score       = 0,      -- 装备分数
  achievement_score = 0,      -- 成就积分
  pet_score         = 0,      -- 宠物积分

  -- 货币
  money             = { nGold=0, nSilver=0, nCopper=0 },
  justice           = 0,      -- 正义值
  justice_remain    = 0,
  exam_print        = 0,      -- 科举印
  coin              = 0,      -- 通宝
  contribution      = 0,      -- 贡献
  starve            = 0,
  prestige          = 0,      -- 威望
  camp_point        = 0,      -- 阵营威望
  arena_award       = 0,      -- 名剑币

  -- 体力/精力
  account_stamina   = { current=0, max=0 },  -- 体力
  role_stamina      = { current=0, max=0 },  -- 精力

  -- 元数据
  time              = 0,      -- 缓存时间戳（GetCurrentTime）
}
```

**分版本列配置**：从 `PLUGIN_ROOT/data/role/{$edition}.jx3dat` 动态加载。

**告警列**（`aAlertColumn`，数值下降时告警）：`money, achievement_score, pet_score, contribution, justice, starve, prestige, camp_point, arena_award, exam_print`。

**旧版迁移**：`D.Migration()` 支持从 `role_stat.v2.db` / `role_stat.v3.db` SQLite 迁移到 jx3dat，迁移后旧库改名 `.bak{时间}` 保留。

### 5.7 task_stat.jx3dat（任务统计，加密）

**生成模块**：`MY_RoleStatistics/src/MY_RoleStatistics_TaskStat.lua`

**文件路径**：
- 全局汇总：`<DATA_ROOT>/!all-users@{$edition}/userdata/role_statistics/task_stat.jx3dat`（PATH_TYPE.GLOBAL）
- 单角色快照：`<DATA_ROOT>/{$uid}@{$edition}/userdata/role_statistics/task_stat.jx3dat`（PATH_TYPE.ROLE）

**加密状态**：茗伊 jx3dat 加密格式。

**任务类型与状态常量**：

```lua
TASK_TYPE = { DAILY=1, WEEKLY=2, HALF_WEEKLY=3, ONCE=4 }
TASK_STATE = { ACCEPTABLE=1, ACCEPTED=2, FINISHABLE=3, FINISHED=4, UNACCEPTABLE=5, UNKNOWN=6 }
```

**数据结构**（标识字段同 role_stat，外加任务状态字段）：

```lua
{
  -- 标识字段（同 role_stat）
  guid, account, region, server, name, force, camp, level,

  -- 任务状态字段（默认列）
  big_war             = TASK_STATE,  -- 大战
  teahouse            = TASK_STATE,  -- 茶馆
  crystal_scramble    = TASK_STATE,  -- 晶矿争夺
  stronghold_trade    = TASK_STATE,  -- 据点贸易
  dragon_gate_despair = TASK_STATE,  -- 龙门绝境
  lexus_reality       = TASK_STATE,  -- 列星虚境
  lidu_ghost_town     = TASK_STATE,  -- 李渡鬼域
  public_routine      = TASK_STATE,  -- 公共日常
  force_routine       = TASK_STATE,  -- 勤修不辍
  rookie_routine      = TASK_STATE,  -- 浪客行
  picking_fairy_grass = TASK_STATE,  -- 采仙草
  find_dragon_veins   = TASK_STATE,  -- 寻龙脉
  illustration_routine= TASK_STATE,  -- 美人图
  sneak_routine       = TASK_STATE,  -- 美人图潜行
  exam_sheng          = TASK_STATE,  -- 省试
  exam_hui            = TASK_STATE,  -- 会试

  -- 元数据
  time                = 0,           -- 缓存时间
  time_days           = 0,           -- 距今天数（计算字段）

  -- 内部记录
  tTaskInfo = { [questID] = TASK_STATE },     -- 任务 ID 到状态的映射
  tBuffInfo = { [buffID_level] = TASK_STATE },-- BUFF 触发的任务状态
}
```

**分版本列配置**：从 `PLUGIN_ROOT/data/task/{$edition}.jx3dat` 加载，每列可包含 `aQuestInfo`/`tCampQuestInfo`/`tForceQuestInfo`/`aBuffInfo` 用于判定任务完成状态。

**刷新周期判定**：`IsInSamePeriod(dwTime, eType)` 根据 `X.GetRefreshTime('daily'/'weekly'/'half-weekly')` 判断记录是否在当前刷新周期内，过期字段会被 `D.FilterColumnCircle` 清除。

### 5.8 equip_stat.v4.db（装备统计，SQLite）

**生成模块**：`MY_RoleStatistics/src/MY_RoleStatistics_EquipStat.lua`

**文件路径**：`<DATA_ROOT>/!all-users@{$edition}/userdata/role_statistics/equip_stat.v4.db`（PATH_TYPE.GLOBAL，全局共享）

**加密状态**：SQLite 明文。

#### 5.8.1 表结构

**表 EquipItems**（装备物品）：

```sql
CREATE TABLE IF NOT EXISTS EquipItems (
    ownerkey           NVARCHAR(20)  NOT NULL,
    suitindex          INTEGER       NOT NULL,  -- 套装索引
    boxtype            INTEGER       NOT NULL,  -- 装备槽类型
    boxindex           INTEGER       NOT NULL,  -- 装备槽索引
    itemid             INTEGER       NOT NULL,
    tabtype            INTEGER       NOT NULL,
    tabindex           INTEGER       NOT NULL,
    tabsubindex        INTEGER       NOT NULL,
    stacknum           INTEGER       NOT NULL,
    uiid               INTEGER       NOT NULL,
    strength           INTEGER       NOT NULL,  -- 强化等级
    durability         INTEGER       NOT NULL,  -- 耐久
    diamond_enchant    NVARCHAR(100) NOT NULL,  -- 五彩石
    fea_enchant        INTEGER       NOT NULL,  -- 附魔
    permanent_enchant  INTEGER       NOT NULL,  -- 永久附魔
    temporary_enchant  INTEGER       NOT NULL,  -- 临时附魔
    desc               NVARCHAR(4000) NOT NULL, -- 装备描述（含「推荐心法：门派(心法)」）
    time               INTEGER       NOT NULL,
    extra              TEXT          NOT NULL,
    PRIMARY KEY(ownerkey, boxtype, boxindex)
)
```

索引：`EquipItems_tab_idx ON EquipItems(tabtype, tabindex, tabsubindex)`

**表 OwnerInfo**（装备所有者信息）：

```sql
CREATE TABLE IF NOT EXISTS OwnerInfo (
    ownerkey       NVARCHAR(20)  NOT NULL,
    ownername      NVARCHAR(20)  NOT NULL,
    servername     NVARCHAR(20)  NOT NULL,
    ownerforce     INTEGER       NOT NULL,  -- 门派 ID
    ownerrole      INTEGER       NOT NULL,
    ownerlevel     INTEGER       NOT NULL,
    ownerscore     NVARCHAR(100) NOT NULL,  -- 装分（序列化的 Lua 表，按 suitindex 索引）
    ownersuitindex INTEGER       NOT NULL,
    time           INTEGER       NOT NULL,
    extra          TEXT          NOT NULL,
    PRIMARY KEY(ownerkey)
)
```

索引：`OwnerInfo_ownername_idx`、`OwnerInfo_servername_idx`

#### 5.8.2 装备槽列表（EQUIPMENT_ITEM_LIST）

13 个主装备槽：头盔、上衣、护腕、腰带、下装、鞋子、项链、腰坠、左戒指、右戒指、近战武器、重剑（藏剑专用）、远程武器、箭囊。

11 个扩展装备槽（`EQUIPMENT_EXTRA_ITEM_LIST`）：腰坠扩展、背部、面部、左肩、右肩、披风、背包、眼镜、左手套、右手套、挂坠宠物。

#### 5.8.3 写入逻辑

- `D.FlushDB()` 在 `O.bSaveDB`（默认 false）开启时写入。
- 触发：`EQUIP_CHANGE` / `EQUIP_ITEM_UPDATE` 事件，延迟 100ms 更新当前角色装分缓存。
- `ownerkey` = `AnsiToUTF8(X.GetClientPlayerGlobalID())`
- `ownerscore` 字段存储序列化的 Lua 表 `{ [suitindex] = 装分 }`，读取时用 `X.DecodeLUAData` 反序列化。
- **`desc` 字段含装备描述文本**，其中包含「推荐心法：门派(心法)」信息——这是 jx3-raid-manager 推断角色心法的关键数据源。

#### 5.8.4 旧版迁移

支持从 `equip_stat.v2.db` / `equip_stat.v3.db` SQLite 迁移，迁移后旧库改名 `.bak{时间}` 保留。

### 5.9 *.jcl（战斗事件记录，GBK 文本 + CRC）

**生成模块**：`MY_TeamTools/src/MY_CombatLogs.lua`

**文件路径**：`<DATA_ROOT>/{$uid}@{$edition}/userdata/combat_logs/`

#### 5.9.1 设置项

```lua
O = {
  bEnable                = false,  -- 数据记录总开关
  nMaxHistory            = 300,    -- 最大历史数据数量
  nMinFightTime          = 30,     -- 最小战斗时间（秒），不足则删除
  bEnableInDungeon       = false,  -- 在秘境中启用
  bEnableInArena         = true,   -- 在名剑大会中启用
  bEnableInBattleField   = false,  -- 在战场中启用
  bEnableInOtherMaps     = false,  -- 在其他类型地图中启用
  bNearbyAll             = true,   -- 保存附近所有角色事件记录
  bTargetInformation     = false,  -- 保存角色状态数据（PVP 模式）
  nTargetInformationThrottle = 200,-- 角色状态记录节流（毫秒）
}
```

**设置入口**：茗伊插件集 → 团队 → 团队工具 → 面板中由 `MY_CombatLogs.OnPanelActivePartial` 渲染的「战斗事件记录」开关（`bEnable`）。

**总开关限制**（`D.UpdateEnable`）：
- 受 `MY_CombatLogs.BanHDD` 限制：HDD 磁盘强制禁用
- 按当前地图类型选择对应子开关（秘境/名剑/战场/其他）
- 在 `LOADING_ENDING` 事件时重算
- 开关切换时：从开→关调用 `CloseCombatLogs`，从关→开且正在战斗调用 `OpenCombatLogs`

#### 5.9.2 文件命名规则

**战斗中临时文件**（`D.OpenCombatLogs`）：

```
YYYY-MM-DD-HH-MM-SS-<副本名>(<副本ID>).jcl.log
```

- 时间戳：`X.FormatTime(GetCurrentTime(), '%yyyy-%MM%dd-%hh-%mm-%ss')`
- `<副本名>` = `X.GetMapInfo(me.GetMapID()).szName`（如 `25人英雄阆风悬城`）
- `<副本ID>` = `me.GetMapID()`

**战斗结束归档**（`D.CloseCombatLogs`）：
- 若战斗时长 `< nMinFightTime`（默认 30 秒）→ 直接删除
- 否则重命名为：

```
YYYY-MM-DD-HH-MM-SS-<副本名>(<副本ID>)-<BOSS名>(<BOSS模板ID>).jcl
```

- `<BOSS名>` 与 `<BOSS模板ID>` 来自 `LOG_NAMING_COUNT` 中被提及次数最多的 NPC（`p.nCount` 最大者）
- 当整场战斗无 NPC 被记录（`LOG_NAMING_COUNT` 为空）时，`szName` 为空字符串，文件名形如 `2024-05-25-17-01-57-天山碎冰谷(127)-(46297).jcl`

**文件名解析正则**（jx3-raid-manager 的 `drop_scanner.rs` 验证样本）：

```
^(\d{4})-(\d{2})-(\d{2})-(\d{2})-(\d{2})-(\d{2})-(.+?)\((\d+)\)-(.*?)\((\d+)\)\.jcl$
```

样本：
- `2026-06-13-18-13-59-25人普通阆风悬城(794)-笑妆娘(137088).jcl`
- `2026-06-14-16-50-08-英雄天龙寺(683)-枯荣大师·幻影(129124).jcl`
- `2024-05-25-17-01-57-天山碎冰谷(127)-(46297).jcl`（BOSS 名为空）

**文件名编码**：GBK（需 `encoding_rs` 解码）。

#### 5.9.3 文件内容格式

JCL 为**流式追加**的 TSV 文本，每行一条事件：

```
<CRC>\t<LogicFrameCount>\t<GetCurrentTime>\t<GetTime>\t<EventID>\t<EncodeLUAData(oData)>\n
```

| 字段 | 说明 |
|------|------|
| CRC | `GetStringCRC(上一行CRC .. 本行szLog .. X.SECRET['HASH::MY_COMBAT_JCL'])`，流式校验防篡改 |
| LogicFrameCount | `GetLogicFrameCount()` 逻辑帧计数 |
| GetCurrentTime | Unix 时间戳（秒） |
| GetTime | `GetTime()` 毫秒级时间戳 |
| EventID | 见下表 |
| EncodeLUAData(oData) | 茗伊 LUA 数据序列化（转义 `\n`→`\\n`、`\t`→`\\t`） |

**存盘策略**：`LOG_CACHE_LIMIT = 20`，缓存满 20 条或战斗结束/过图时强制 flush。

**进战回放**：`LOG_REPLAY` 缓冲区保存最近 `GAME_FPS * 1` 逻辑帧（约 1 秒）的事件，进战时通过 `D.ImportRecentLogs` 压入新文件，避免丢失进战瞬间的事件。

#### 5.9.4 事件类型（LOG_TYPE）

| ID | 常量 | 说明 |
|----|------|------|
| 1 | FIGHT_TIME | 战斗时间（数据：`{ bFighting, szUUID, nDuring, dwMapID }`） |
| 2 | PLAYER_ENTER_SCENE | 玩家进入场景 |
| 3 | PLAYER_LEAVE_SCENE | 玩家离开场景 |
| 4 | PLAYER_INFO | 玩家信息（`{ dwID, szName, dwForceID, dwKungfuID, nEquipScore, aEquip, aTalent, szGUID, tZhenPai }`） |
| 5 | PLAYER_FIGHT_HINT | 玩家战斗状态改变 |
| 6 | NPC_ENTER_SCENE | NPC 进入场景 |
| 7 | NPC_LEAVE_SCENE | NPC 离开场景 |
| 8 | NPC_INFO | NPC 信息（`{ dwID, szName, dwTemplateID, dwEmployer, nX, nY, nZ, nFaceDirection, [state] }`，state `0`=存活 `192`=死亡） |
| 9 | NPC_FIGHT_HINT | NPC 战斗状态改变（`{ dwID, bFight, fCurrentLife, fMaxLife, ... }`） |
| 10 | DOODAD_ENTER_SCENE | 交互物件进入场景 |
| 11 | DOODAD_LEAVE_SCENE | 交互物件离开场景 |
| 12 | DOODAD_INFO | 交互物件信息（`{ dwID, dwTemplateID, nX, nY, nZ, nFaceDirection }`） |
| 13 | BUFF_UPDATE | BUFF 刷新 |
| 14 | PLAYER_SAY | 角色喊话（仅 NPC） |
| 15 | ON_WARNING_MESSAGE | 显示警告框 |
| 16 | PARTY_ADD_MEMBER | 团队添加成员 |
| 17 | PARTY_SET_MEMBER_ONLINE_FLAG | 团队成员在线状态改变 |
| 18 | MSG_SYS | 系统消息 |
| 19 | SYS_MSG_UI_OME_SKILL_CAST_LOG | 技能施放日志（`{ dwCaster, dwSkillID, dwLevel }`） |
| 20 | SYS_MSG_UI_OME_SKILL_CAST_RESPOND_LOG | 技能施放结果日志 |
| 21 | SYS_MSG_UI_OME_SKILL_EFFECT_LOG | 技能最终产生的效果（生命值变化） |
| 22 | SYS_MSG_UI_OME_SKILL_BLOCK_LOG | 格挡日志 |
| 23 | SYS_MSG_UI_OME_SKILL_SHIELD_LOG | 技能被屏蔽日志 |
| 24 | SYS_MSG_UI_OME_SKILL_MISS_LOG | 技能未命中目标日志 |
| 25 | SYS_MSG_UI_OME_SKILL_HIT_LOG | 技能命中目标日志 |
| 26 | SYS_MSG_UI_OME_SKILL_DODGE_LOG | 技能被闪避日志 |
| 27 | SYS_MSG_UI_OME_COMMON_HEALTH_LOG | 普通治疗日志 |
| 28 | SYS_MSG_UI_OME_DEATH_NOTIFY | 死亡日志 |
| 29 | TARGET_INFORMATION | 目标状态信息（`{ dwType, dwID, nX, nY, nZ, nFaceDirection, nLife, nMaxLife, nMana, nMaxMana, nDamageAbsorbValue }`） |

#### 5.9.5 触发时机

- **开始记录**：`MY_FIGHT_HINT` 事件 + `bFighting=true` → `D.OpenCombatLogs()` + `D.ImportRecentLogs()`
- **结束记录**：`MY_FIGHT_HINT` 事件 + `bFighting=false` → `D.CloseCombatLogs()`（先写入 `FIGHT_TIME` 事件）
- **强制 flush**：`LOADING_ENDING`、`RELOAD_UI_ADDON_END`、`BATTLE_FIELD_END`、`ARENA_END`、`MY_CLIENT_PLAYER_LEAVE_SCENE`
- **UI 重载**：`X.RegisterReload('MY_CombatLogs', D.CloseCombatLogs)` 保证不丢数据

### 5.10 *.jcl.tsv（JCL 导出格式）

`D.GetHistoryFiles()` 在 `MY_CombatLogs.lua` 第 209 行过滤 `.jcl.tsv$` 后缀的文件作为历史列表。

推断：`.jcl.tsv` 是 `.jcl` 的导出/解码版本（TSV = Tab-Separated Values），可能由 `OnPanelActivePartial` 提供的「导出历史」功能生成，用于外部工具分析。具体创建代码位于 `MY_CombatLogs.lua` 第 470 行之后（WebFetch 在约 16KB 处截断，未能完整获取）。

### 5.11 *.fstt.jx3dat（伤害统计历史）

**生成模块**：`MY_Recount`（`MY_Recount_DS.lua` 数据源 + `MY_Recount.lua` 主模块 + `MY_Recount_UI.lua` / `MY_Recount_DT.lua` UI）

**文件路径**：`MY_Recount_DS.GetHistoryRoot()` 返回的目录（角色目录下）

**加密状态**：茗伊 jx3dat 加密格式。

**保存触发**：
- `bSaveHistoryOnExit`（退出时保存）
- `bSaveHistoryOnExFi`（退出战斗立即保存）

**数量限制**：`nMaxHistory` 可选 5/10/20/30/50/100/200/500/1000。

**数据结构**（`MY_Recount_DS.lua`，层级极深）：

```lua
Data = {
  [DK.UUID]      = 战斗标识,
  [DK.TIME_BEGIN] = 开始时间,
  [DK.DAMAGE]    = { [玩家dwID] = { TOTAL, DETAIL, SKILL, Target } },  -- 伤害
  [DK.HEAL]      = { ... },  -- 治疗
  [DK.BE_DAMAGE] = { ... },  -- 承伤
  [DK.BE_HEAL]   = { ... },  -- 承疗
  [DK.ABSORB]    = { ... },  -- 化解
}
```

含 SKILL → Target → Detail 多层嵌套（命中/暴击/识破/化解等分类统计）。文件打开对话框确认扩展名：

```lua
GetOpenFileName(..., 'Recount File(*.fstt.jx3dat)\0*.fstt.jx3dat\0\0', szRoot)
```

### 5.12 settings.db / userdata.db（用户设置，SQLite）

**生成模块**：`MY_!Base/src/lib/Storage.UserSettings.lua` 的 `CreateUserSettingsModule`

**文件路径**：
- `config/settings.db`（各 PATH_TYPE 下）
- `userdata/userdata.db`

**说明**：茗伊插件集将配置项持久化分为两套机制——配置项走 SQLite `.db`（由 `CreateUserSettingsModule` 处理），业务数据文件走 `.jx3dat`（由 `X.SaveLUAData` 处理）。**绝大多数模块只生成配置项 `.db`，不生成 `.jx3dat` 数据文件**。

### 5.13 achievement_acquire_shot.jx3dat（成就获取截图，明文）

**加密状态**：实测明文（可直接解析）。

> **来源说明**：该文件在 jx3-raid-manager 项目记忆中被明确标注为茗伊明文文件。在 JX3MY 仓库源码层面，已检查 `MY_ScreenShot`（纯截图助手，不挂钩成就事件）与 `MY_Toolbox/MY_AchievementWiki`（成就百科查询器，仅打开网页 URL），均未发现生成此文件的代码。可能由 `MY_Toolbox` 的 37 个子模块中未完整分析的成就相关子模块生成，或由独立分发的扩展模块生成。鉴于项目实测确认其为茗伊格式明文文件，本文件仍归入茗伊数据体系。

### 5.14 非茗伊生成的文件

以下文件在 jx3-raid-manager 项目记忆中被提及，但**并非由 JX3MY 仓库代码生成**：

#### 5.14.1 fight_stat/log_*_*.jx3dat

**来源**：JX3 游戏客户端自身（非茗伊插件）。

**特征**：
- 明文 KLua 格式
- 文件名 `log_<seq>_<type>.jx3dat`，seq 序号单调递增不循环
- 同一场战斗产生 4 个类型文件：`0=伤害`、`1=治疗`、`2=承伤`、`3=承疗`
- 含战斗统计但**无副本/场景字段**

在已获取的 `MY_TeamTools` 全部 31 个 Lua 文件清单中未发现生成此文件的代码，`MY_CombatLogs.lua` 第 470 行后的截断部分亦未涉及（其归档产物是 `.jcl` 与 `.jcl.tsv`）。

#### 5.14.2 角色目录的 userdata.db（角色统计 BLOB）

**说明**：jx3-raid-manager 项目从 `{uid}@zhcn_hd/userdata/userdata.db` 的 `data` 表 `MY_RoleStatistics_RoleStat.tAlertTodayVal` BLOB 中解析 `equip_score`、`force` 等字段。

> 注意：此处的 `userdata.db` 是 JX3 客户端的角色数据存储（SQLite，表结构 `data(key TEXT, value BLOB)`），内容层为 KLua 二进制流。**虽然键名前缀是 `MY_RoleStatistics_RoleStat`，但 BLOB 由 JX3 引擎按 KLua 序列化写入**，茗伊插件仅读取，不直接生成该 db 文件。茗伊自身生成的装备统计数据存储在 `equip_stat.v4.db`。

---

## 六、纯 UI / 配置类模块清单

以下模块**仅生成配置项（存 `.db`）**，不生成 `.jx3dat` 业务数据文件：

| 模块 | 功能 | 配置 PATH_TYPE |
|------|------|----------------|
| MY_Toolbox（全部 37 个子模块） | 常用工具集 | 多为 ROLE |
| MY_ScreenShot | 截图助手 | GLOBAL |
| MY_Target | 目标辅助（方位/朝向/连线/选择） | ROLE |
| MY_BagEx | 背包扩展 | ROLE |
| MY_Cataclysm | 大灾难 | ROLE |
| MY_CombatText | 战斗文本 | ROLE |
| MY_Farbnamen | 彩色名字 | ROLE |
| MY_Focus | 焦点 | ROLE |
| MY_Font | 字体 | GLOBAL |
| MY_FontResource | 字体资源 | GLOBAL |
| MY_Force | 门派 | ROLE |
| MY_LifeBar | 头顶血条 | ROLE |
| MY_Logoff | 快速登出 | ROLE |
| MY_MiddleMapMark | 地图标记 | ROLE |
| MY_Resource | 资源 | ROLE |
| MY_RollMonitor | 摇点监控 | ROLE |
| MY_TargetMon | 目标监控 | ROLE |
| MY_TeamMon | 团队监控（订阅数据除外） | ROLE |
| MY_ThreatRank | 仇恨排名 | ROLE |
| MYDev_Snaplines | 开发-连线 | - |
| MYDev_UIEventID | 开发-UI事件ID | - |
| MYDev_UITexViewer | 开发-贴图查看 | - |
| MYDev_VarWatch | 开发-变量监视 | - |

---

## 七、MY_!Base 公共 API 全览

### 7.1 文件级持久化（最常用）

| API | 说明 |
|-----|------|
| `X.SaveLUAData(oFilePath, oData, tConfig?)` | 保存数据，自动注入 passphrase |
| `X.LoadLUAData(oFilePath, tConfig?)` | 加载数据，自动注入 passphrase；新文件先明文读再加密重写 |
| `X.GetLUADataPath(oFilePath)` | 补全 `.jx3dat` 后缀 |
| `X.FormatPath(oFilePath, tParams?)` | 展开占位符、处理 PATH_TYPE |
| `X.CreateDataRoot(ePathType)` | 创建目录并写 info.jx3dat |

### 7.2 序列化 / 压缩

| API | 说明 |
|-----|------|
| `X.EncodeLUAData(data)` | `= _G.var2str`，Lua 文本序列化 |
| `X.DecodeLUAData(szText)` | `= _G.str2var`，反序列化 |
| `X.EncodeJSON(vData, bIndent?)` | JSON 编码 |
| `X.DecodeJSON(value)` | JSON 解码 |
| `X.CompressLUAData(xData)` | var2str → zlib → URL-safe Base64 |
| `X.DecompressLUAData(sz)` | 逆过程 |
| `X.Deflate:CompressZlib` / `DecompressZlib` | zlib 原始接口 |
| `X.EncodeBase64` / `X.DecodeBase64` | BaseXX 库 |
| `X.GetLUADataHash(data, fnAction?)` | 基于 CRC 的表哈希（支持异步） |

### 7.3 加密原语

| API | 说明 |
|-----|------|
| `X.EncryptString(szText)` | 强加密：SECRET 中有 map 时用置换+seed+Base64；否则退化为 Caesar+13 hex |
| `X.DecryptString(szText)` | 逆操作 |
| `X.SimpleEncryptString(szText)` | Caesar +13 → Base64（URL-safe） |
| `X.SimpleDecryptString(szCipher)` | 逆操作 |
| `X.KGUIEncrypt(szText)` / `X.KE` | 多层：EncodeData → KGUIEncrypt → MD5(SimpleEncryptString(...)) |

### 7.4 高阶存储

| API | 说明 |
|-----|------|
| `X.InfoCache(SZ_DATA_PATH, SEG_LEN, L1_SIZE, ValueComparer?)` | 分段键值缓存，按 key 前缀分桶存到多个 .jx3dat，带 L1 弱引用缓存 |
| `X.SQLiteConnect(szCaption, oPath, fnAction)` | SQLite 连接，自带 malformed 修复 |
| `X.RegisterRemoteStorage(szKey, nBitPos, nBitNum, fnGetter, fnSetter, bForceOnline?)` | 游戏官方位级存储（32 字节 = 256 位，按位分配） |
| `X.StorageData(szKey, oData)` | 云存储：上传到 `https://push-storage.j3cx.com/api/storage/uploads`，用 `X.EncryptString(ConvertToUTF8(EncodeJSON(...)))` 加密载荷 |
| `X.RegisterUserSettings*` | 用户设置系统（基于 schema 的声明式持久化） |

### 7.5 辅助

- `X.GetUUID()` — RFC 4122 v4 UUID
- `X.GetClientGUID()` — 客户端软唯一标识
- `X.GetClientPlayerGlobalID()` — 角色 GlobalID（uid），失败时用 `0 + CRC(区) + CRC(服) + dwID` 兜底
- `X.GetAccount()` — 账号名（多源兜底）

---

## 八、数据文件总览速查表

| 文件 | 格式 | 加密 | 生成模块 | PATH_TYPE | 关键内容 |
|------|------|------|----------|-----------|----------|
| `info.jx3dat` | Lua 文本 | 明文 | MY_!Base | ROLE | 角色元数据（uid/name/server/edition/time） |
| `manifest.jx3dat` | jx3dat 二进制 | 加密（加盐默认 key） | MY_!Base | 各 domain | 路径→UUID 密钥映射 |
| `chatlog_XXXXXX.v2.db` | SQLite | 明文 | MY_ChatLog | ROLE | 聊天记录（ChatInfo + ChatLog 两表） |
| `chatmonitor.jx3dat` | jx3dat | 非强加密 | MY_Chat | ROLE/SERVER | 监控记录（list + hash） |
| `current.gkp.jx3dat` | jx3dat | 加密 | MY_GKP | ROLE | 当前金团记录 |
| `*.gkp.jx3dat`（历史） | jx3dat | 加密 | MY_GKP | ROLE | 历史金团归档（最多 22 个） |
| `role_stat.jx3dat` | jx3dat | 加密 | MY_RoleStatistics | GLOBAL/ROLE | 角色统计（装分/货币/积分） |
| `task_stat.jx3dat` | jx3dat | 加密 | MY_RoleStatistics | GLOBAL/ROLE | 任务统计（日常/周常状态） |
| `equip_stat.v4.db` | SQLite | 明文 | MY_RoleStatistics | GLOBAL | 装备统计（EquipItems + OwnerInfo） |
| `*.jcl` | GBK TSV + CRC | 明文 | MY_TeamTools/MY_CombatLogs | ROLE | 战斗事件记录（29 种事件） |
| `*.jcl.log` | GBK TSV + CRC | 明文 | MY_TeamTools/MY_CombatLogs | ROLE | 战斗中临时文件 |
| `*.jcl.tsv` | GBK TSV | 明文 | MY_TeamTools/MY_CombatLogs | ROLE | JCL 导出格式 |
| `*.fstt.jx3dat` | jx3dat | 加密 | MY_Recount | ROLE | 伤害统计历史 |
| `achievement_acquire_shot.jx3dat` | jx3dat | 明文 | （茗伊体系，模块待确认） | - | 成就获取截图 |
| `config/settings.db` | SQLite | 明文 | MY_!Base | 各 PATH_TYPE | 用户配置项 |
| `userdata/userdata.db` | SQLite | 明文 | MY_!Base | 各 PATH_TYPE | 用户数据 |
| `export/**.jx3dat` | jx3dat | **明文** | 各模块导出 | - | 导出数据（强制明文） |
| `debug.level.jx3dat` / `log.level.jx3dat` | jx3dat | 明文 | MY_!Base | DATA_ROOT 顶层 | 日志级别配置 |
| `fight_stat/log_*_*.jx3dat` | KLua | 明文 | **JX3 客户端**（非茗伊） | - | 战斗统计（4 类型） |

---

## 九、对 JX3 Raid Manager 项目的应用价值

基于 jx3-raid-manager 项目实测，茗伊插件集的 6 大数据源已被有效利用：

| 数据源 | 用途 | 可读性 |
|--------|------|--------|
| `info.jx3dat` | 登录信号 + 角色识别（name/uid/server） | ✅ 明文直接读 |
| `userdata/userdata.db` | 角色装分（`MY_RoleStatistics_RoleStat.tAlertTodayVal` BLOB） | ✅ SQLite + KLua 解析 |
| `userdata/chat_log/*.v2.db` | 聊天记录（GKP 收支、购买装备、底薪） | ✅ SQLite 直接查 |
| `userdata/gkp/*.gkp.jx3dat` | 金团收支信号（副本结束归档） | ⚠️ 加密，靠文件名 + mtime 信号 |
| `userdata/combat_logs/*.jcl` | 副本信号 + BOSS 击杀判定（文件名 + 内容） | ✅ GBK 文本解析 |
| `equip_stat.v4.db` | 心法推断（装备描述含「推荐心法」） | ✅ SQLite 直接查 |

### 9.1 关键应用要点

1. **目录定位**：茗伊数据根目录是 `Interface/MY#DATA/`（本地）而非 `Interface/MY/data/`。角色目录形如 `Interface/MY#DATA/<uid>@<edition>/`。
2. **info.jx3dat 是最佳入口**：明文 luatext，可直接用 Lua `loadstring` 或正则解析角色名、服务器、uid、登录时间，无需解密。
3. **业务数据解密难点**：每个 `*.jx3dat` 用独立 UUID 加密，UUID 存在 `manifest.jx3dat`，manifest 又用加盐 passphrase 加密。若无 `secret.jx3dat`，manifest 密钥 = `GetPassphrase(666, 233)`（可复算）。但**底层 XOR/流密码算法在游戏引擎内**，仓库未公开，需逆向游戏 `SaveLUAData`/`LoadLUAData` 才能离线解密。
4. **明文文件可直接读取**：`info.jx3dat`、`export/**.jx3dat`、`debug.level.jx3dat`、`log.level.jx3dat`、`equip_stat.v4.db`、`chatlog_*.v2.db`、`*.jcl`。
5. **版本兼容**：`@{$edition}` 命名较新，旧版用 `@{$lang}`，`Storage.Path.lua` 有自动迁移逻辑，但磁盘上可能仍有旧路径残留需兼容。
6. **chatlog 不记录 MSG_SYS**：副本进入/退出事件无法从 chatlog 获取，需依赖 JCL 文件名信号。
7. **JCL 文件名含副本与 BOSS 信息**：是副本识别与 BOSS 击杀判定的关键信号，但需 GBK 解码。
8. **GKP 历史最多 22 个**：超出会被自动删除，早期记录可能丢失。
9. **chatlog 分片 20000 条**：单文件超 20000 条即新建分片，一个角色可能有多个 chatlog 文件，需全部扫描。
10. **chatlog 时间戳是游戏运行时间**：非真实世界时间，跨文件查询时需注意。

---

## 十、局部分析局限与未确认项

1. **MY_CombatLogs.lua 第 470-504 行未获取**：WebFetch 与 jsdelivr CDN 均在约 16KB 处截断，`.jcl.tsv` 创建逻辑位于此范围内。建议 `git clone` 后本地读取。
2. **achievement_acquire_shot.jx3dat 生成模块未定位**：在已检查的 `MY_ScreenShot`、`MY_AchievementWiki` 中均未发现生成代码。建议检查 `MY_Toolbox` 全部 37 个子模块或独立扩展模块。
3. **fight_stat/log_*_*.jx3dat 确认为 JX3 客户端生成**：非茗伊插件，但其键名前缀 `MY_RoleStatistics_RoleStat` 暗示茗伊可能通过游戏 API 触发写入。建议通过本地运行时观察 `userdata/fight_stat/` 目录文件创建时机确认。
4. **MY_Farbnamen / MY_Focus / MY_MiddleMapMark 的 info.ini** 三次获取均失败（WebFetch 间歇性错误），但其架构属纯 UI 模块，按茗伊惯例仅生成配置。
5. **MY_Serendipity.lua（奇遇模块）** 获取失败，未确认是否生成奇遇数据文件。
6. **MY_TeamMon 订阅数据**：含 `MY_TeamMon_Subscribe_Data.lua` / `Subscribe_FavoriteData.lua` / `VoicePacket_Custom.lua`，可能持久化语音包等自定义数据，未深入分析。
7. **GitHub 代码搜索需登录**：无法通过 `github.com/search` 验证 `achievement_acquire_shot` 字符串在仓库中的出现位置。

---

## 附录 A：关键源码文件路径索引

| 关注点 | 文件 |
|--------|------|
| jx3dat 持久化核心 | `MY_!Base/src/lib/Storage.LUAData.lua` |
| 路径与 info.jx3dat 写入 | `MY_!Base/src/lib/Storage.Path.lua` |
| 命名空间/SECRET/DATA_ROOT | `MY_!Base/src/lib/Base.lua` |
| var2str/str2var 包装 | `MY_!Base/src/lib/BaseAPI.lua` |
| 加密函数群 | `MY_!Base/src/lib/String.lua` |
| UUID 生成 | `MY_!Base/src/lib/Uuid.lua` |
| Base64 编码 | `MY_!Base/src/lib/BaseXX.lua` |
| zlib 压缩 | `MY_!Base/src/lib/Deflate.lua` |
| SQLite | `MY_!Base/src/lib/Storage.SQLite.lua` |
| 云存储 + 位存储 | `MY_!Base/src/lib/Storage.RemoteStorage.lua` |
| 用户设置 | `MY_!Base/src/lib/Storage.UserSettings.lua` |
| 分段键值缓存 | `MY_!Base/src/lib/System.InfoCache.lua` |
| 加载清单 | `MY_!Base/info.ini` |
| chatlog 单库操作 | `MY_ChatLog/src/MY_ChatLog_DB.lua` |
| chatlog 集群控制 | `MY_ChatLog/src/MY_ChatLog_DS.lua` |
| chatlog 主逻辑 | `MY_ChatLog/src/MY_ChatLog.lua` |
| chatmonitor | `MY_Chat/src/MY_ChatMonitor.lua` |
| GKP 数据源 | `MY_GKP/src/MY_GKP_DS.lua` |
| GKP 主实例化 | `MY_GKP/src/MY_GKP_MI.lua` |
| GKP 主模块 | `MY_GKP/src/MY_GKP.lua` |
| 角色统计 | `MY_RoleStatistics/src/MY_RoleStatistics_RoleStat.lua` |
| 任务统计 | `MY_RoleStatistics/src/MY_RoleStatistics_TaskStat.lua` |
| 装备统计 | `MY_RoleStatistics/src/MY_RoleStatistics_EquipStat.lua` |
| 战斗日志 | `MY_TeamTools/src/MY_CombatLogs.lua` |
| 伤害统计 | `MY_Recount/src/MY_Recount.lua` / `MY_Recount_DS.lua` |

---

## 附录 B：MSG 类型常量速查

茗伊 chatlog 与 chatmonitor 中涉及的 MSG 类型：

| 常量 | 说明 | chatlog 默认 | chatmonitor 默认 |
|------|------|--------------|------------------|
| `MSG_WHISPER` | 密聊 | ✅ | - |
| `MSG_SSG_WHISPER` | 系统密聊 | ✅ | - |
| `MSG_PARTY` | 队伍 | ✅ | - |
| `MSG_TEAM` | 团队 | ✅ | - |
| `MSG_ROOM` | 房间 | ✅ | - |
| `MSG_FRIEND` | 好友 | ✅ | ✅ |
| `MSG_GUILD` | 帮会 | ✅ | ✅ |
| `MSG_GUILD_ALLIANCE` | 帮会联盟 | ✅ | - |
| `MSG_SELF_DEATH` | 自身死亡 | ✅ | - |
| `MSG_SELF_KILL` | 自身击杀 | ✅ | - |
| `MSG_PARTY_DEATH` | 队友死亡 | ✅ | - |
| `MSG_PARTY_KILL` | 队友击杀 | ✅ | - |
| `MSG_MONEY` | 金钱变动 | ✅ | - |
| `MSG_ITEM` | 物品获取 | ✅ | - |
| `MSG_MY_MONITOR` | 茗伊监控 | ✅ | - |
| `MSG_SYS` | 系统滚动 | ❌ 不记录 | ❌（可手动加） |
| `MSG_NORMAL` | 近聊/说 | ❌ | ✅ |
| `MSG_CAMP` | 阵营 | ❌ | ✅ |
| `MSG_WORLD` | 世界 | ❌ | ✅ |
| `MSG_MAP` | 地图 | ❌ | ✅ |
| `MSG_SCHOOL` | 门派 | ❌ | ✅ |
| `MSG_IDENTITY` | 身份 | ❌ | ✅ |

---

*文档生成时间：2026-08-02*
*分析基于：JX3MY master 分支 v29.0.10 + jx3-raid-manager 项目实测数据*
