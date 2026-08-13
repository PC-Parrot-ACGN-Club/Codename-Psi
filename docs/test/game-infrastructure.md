# 游戏基础设施测试索引

**关联需求：** [Issue #11](https://github.com/PC-Parrot-ACGN-Club/Codename-Psi/issues/11)

**关联设计：** [游戏基础设施运行架构](../development/system/game-infrastructure-architecture.md)、[模块设计](../development/design/)

本索引组织游戏基础设施的测试设计。纯 Component 行为按被测能力收录在 `component/`，需要组件协作或完整运行栈的行为收录在 `integration-system/`；具体断言只在对应测试稿中定义。

## Component 测试

| 测试稿 | 覆盖主题 |
| --- | --- |
| [本机用户设置](component/user-settings.md) | 默认值、解析恢复、序列化、玩家绑定与冲突检测。 |
| [本地化](component/localization.md) | 文本查询、英文回退、诊断与 catalog 校验。 |
| [游戏动作与 Tick 输入](component/game-actions.md) | 动作值、参与者槽位、归一化与稳定位编码。 |
| [客户端输入](component/client-input.md) | 物理输入采样、玩家隔离、输入上下文、press edge 与摇杆阈值。 |
| [运行数据解析](component/runtime-data-parsing.md) | 版本化 RON/JSON 解析与 typed error。 |
| [应用状态表](component/application-state-table.md) | 合法状态边与表外状态边的纯判定。 |

## Component Integration 与 System 测试

| 测试稿 | 覆盖主题 |
| --- | --- |
| [输入与固定调度](integration-system/input-and-fixed-tick.md) | 输入到规则的协作路径、60Hz 调度、暂停边界与设备生命周期。 |
| [应用生命周期](integration-system/application-lifecycle.md) | 状态提交、请求仲裁、启动屏障与基础状态主路径。 |
| [运行数据与设置持久化](integration-system/runtime-data.md) | Bevy Asset resolution、错误上下文、原子保存与消费者装配。 |
| [构建与启动](integration-system/build-and-startup.md) | Linux 构建、workspace 边界、自动化 smoke 与真实窗口启动。 |

## 公共执行约束

- 参数化用例的一行代表一个逻辑用例，实施时每组测试数据都作为独立 case 执行并报告。
- 诊断以实现可提供的错误、事件或日志等形式观测；测试只核对 Confirmed 文档要求的分类与上下文，不固定错误载体或 API 名称。
- Production build（`integration-system/build-and-startup::TC-001`）与真实窗口有界启动（`integration-system/build-and-startup::TC-004`）由 `release.yml` 的 Linux runner 执行。
- 自动化 startup smoke（`integration-system/build-and-startup::TC-003`）及其余自动化测试由 `test.yml` 的 Linux runner 执行。开发分支的推送不触发 CI，由本地执行覆盖。
- 除 `integration-system/build-and-startup::TC-004` 外的用例不依赖真实显示环境。
- production client 与自动化 startup smoke 共用同一项目根插件；smoke 不要求真实窗口，production build 使用与生产环境一致的 `DefaultPlugins` 配置；自动退出方式由实现选择。

## 范围边界

本组测试稿不定义以下行为：

- `PlayerActions` 位编码之上的稳定网络报文格式；bit 编号由 `component/game-actions::TC-011` 验证。
- `RuleProfile`、角色、Fever 题面及其它玩法配置的完整 schema 和语义约束；该范围见[规则配置与开局规格冻结](../development/design/rule-configuration.md)。
- UI 返回、退出、再赛等新增状态边及其仲裁优先级；应用状态基线见[应用状态机](../development/design/application-state-machine.md)。
- 方向输入的 DAS/ARR 节奏；采样器不提供连发由 `component/client-input::TC-008` 验证，重复移动规则由玩法设计定义。
- 规则 tick 的网络帧号、回滚状态、渲染表现和音频结果；对应职责分别归属 Network、Rules 与 Client 后续测试设计。

`main.rs` 薄入口、模块所有权和禁止平行状态字段属于架构审查项；可机械验证的 crate 依赖边界由 `integration-system/build-and-startup::TC-002` 回归保护。
