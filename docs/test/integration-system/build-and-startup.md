# 测试用例设计：构建与启动

**关联设计：** [游戏基础设施运行架构](../../development/system/game-infrastructure-architecture.md)、[TDD](../../TDD.md)、[版本化运行数据加载](../../development/design/runtime-data-loading.md)

**关联实现：** `../../../Cargo.toml`、`../../../crates`、`../../../.github/workflows`、`../../../.github/ci-testing`

## 需求理解摘要

**功能：** 验证 Linux production 构建、workspace 依赖边界、自动化 startup smoke 与真实窗口有界启动。
**测试性质：** 新功能（含既有设计的补充变更）
**本轮范围：** 必须由构建图或已装配客户端证明的 System 行为。
**Test Basis：**

- [Confirmed] [游戏基础设施运行架构](../../development/system/game-infrastructure-architecture.md)：crate 职责、Linux 构建和三个启动验收面。
- [Confirmed] [TDD](../../TDD.md)：CI 与 release 执行环境。
- [Confirmed] [版本化运行数据加载](../../development/design/runtime-data-loading.md)：production 资产根。

**设计基线：** 自动化 smoke 与 production client 共用项目根插件；真实窗口验证使用虚拟显示和软件 Vulkan。
**公共执行约束：** 见[游戏基础设施测试索引](../game-infrastructure.md#公共执行约束)。

## 范围边界

本文不定义启动屏障内部状态组合（见 [应用生命周期](application-lifecycle.md)）或数据解析与加载细节（见 [运行数据与设置持久化](runtime-data.md)）。

## 测试点清单

- Linux production client 构建并链接（TC-001；Concern: Smoke）。
- workspace 依赖方向与 `game_core` 平台隔离（TC-002）。
- 无真实窗口的自动化 startup smoke（TC-003；Concern: Smoke）。
- 真实 production 二进制在虚拟显示中有界启动（TC-004；Concern: Smoke）。

## 设计方法与覆盖模型

| 方法 | 输入模型 / 风险 | 关联用例 |
| --- | --- | --- |
| 架构约束检查 | Cargo dependency graph、manifest 与独立 package 构建 | TC-002 |
| 场景 / 协作路径 | production 构建、自动化 smoke 与真实窗口启动 | TC-001、TC-003～TC-004 |

## 测试用例列表

| ID | 标题 | Priority | Test Level | Concern | Domain | 前置条件 | 操作/刺激 | 测试数据 | 预期结果 | Test Basis / 证据状态 |
| --- | --- | --- | --- | --- | --- | --- | --- | --- | --- | --- |
| TC-001 | Linux 目标构建并链接 production client | P0 | System | Smoke | Client | Linux CI runner 与发布支持的 Rust toolchain | 使用生产 Bevy plugin 配置（`DefaultPlugins` + 项目根插件）构建 workspace/client | Linux x86_64；默认功能集合 | 编译、链接、插件装配成功，产出可执行的 production 二进制；本用例不要求运行到 MainMenu（运行路径由 TC-003 覆盖） | [Confirmed] [游戏基础设施运行架构：启动验收](../../development/system/game-infrastructure-architecture.md#启动验收) |
| TC-002 | workspace 依赖图保持 client/net 指向 game_core 且 game_core 与平台运行时隔离 | P1 | System | — | Client | 可读取 Cargo metadata 与各 crate manifest，并可单独选择 game_core package | 检查 Cargo dependency graph 与 game_core manifest，再独立构建和测试 game_core | 必需边：client→game_core、net→game_core；禁止边：game_core→client/net；game_core manifest 禁止 Bevy、网络、窗口、平台目录等平台运行时 crate | 必需边存在，禁止边不存在；game_core manifest 不含所列平台运行时依赖；game_core 可独立构建并通过测试 | [Confirmed] [游戏基础设施运行架构：模块职责](../../development/system/game-infrastructure-architecture.md#模块职责) |
| TC-003 | Linux 自动化 startup smoke 复用项目根插件跑通 Boot→MainMenu | P0 | System | Smoke | Client | Linux CI runner；最小 Bevy runtime + 与生产客户端相同的项目根插件，不含真实窗口依赖 | 运行可自动退出的 startup smoke | Linux x86_64 | 项目根插件装配成功；`AppState` 初始化为 `Boot`；`UserSettings` 与 `Localization` 完成 bootstrap resolution；应用到达 `MainMenu`；进程正常退出；全程不要求真实窗口交互 | [Confirmed] [游戏基础设施运行架构：启动验收](../../development/system/game-infrastructure-architecture.md#启动验收) |
| TC-004 | 真实 production 二进制有界启动到 `MainMenu` 后退出 | P0 | System | Smoke | Client | Linux 环境具备虚拟显示与软件 Vulkan 后端；以 `ci_testing` feature 构建的真实 `psi` 二进制；资产根指向仓库根 | 以指定退出帧的配置运行二进制并等待进程结束 | 退出帧取到达 `MainMenu` 之后的确定帧 | 窗口创建成功；应用到达 `MainMenu`；运行期无资源缺失诊断，即 `assets/data`、`assets/i18n` 均真实读取而非加载失败；进程在指定帧自动退出且退出码为成功；本用例覆盖 `DefaultPlugins` 运行时初始化、窗口后端与生产资产根解析，不要求交互 | [Inferred] [游戏基础设施运行架构：启动验收](../../development/system/game-infrastructure-architecture.md#启动验收)；[版本化运行数据加载：资产根](../../development/design/runtime-data-loading.md#资产根) |

## 风险查漏

编译链接、插件装配、依赖方向、Boot→MainMenu、窗口后端和真实资产根均有直接用例。

