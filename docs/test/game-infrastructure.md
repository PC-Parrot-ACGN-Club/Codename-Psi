# 测试用例设计：游戏基础设施

**状态：** 等待用户审核  
**关联设计：** [游戏基础设施运行架构](../development/system/game-infrastructure-architecture.md)、[Component Specs](../../development/component/)、[Component Integration Contracts](../../development/contract/)  
**关联实现：** `../../crates/core`、`../../crates/client`、`../../crates/net`、`../../assets/data`、`../../assets/i18n`  
**设计范围提交：** `29494d5`（`docs: design game infrastructure`）、`2ce3b47`（`docs: pin localization InvalidData, split UIAction, scope fixed-tick to Match`）

## 需求理解摘要

**功能：** 建立 Bevy 客户端的应用状态、统一输入、60Hz 规则调度、版本化数据、本地化、用户设置与 workspace 职责边界。  
**测试性质：** 新功能（含既有设计的补充变更）  
**本轮范围：** 提交 `29494d5` 在 `../development/component`、`../development/contract` 与 `../development/system` 新增的全部运行基础设施行为，叠加提交 `2ce3b47` 引入的四项变更：本地化 catalog locale 语义校验（`InvalidData`）、独立于 `GameAction` 的 `UIAction` Spec、`FixedGameSet::Input`/`Rules` 限定在 `AppState::Match` 执行并定义 `Paused` 生命周期、以及生产客户端与自动化 startup smoke 复用同一项目根插件（方案 A）并拆分两者各自的验收条件。  
**Test Basis：**

- [Confirmed] [游戏基础设施运行架构](../development/system/game-infrastructure-architecture.md)：启动屏障、顶层主流程、60Hz 规则路径、crate 边界、`Paused` 对局 simulation 生命周期、Production build 与 Automated startup smoke 的验收拆分。
- [Confirmed] [应用状态机 Spec](../development/component/application-state-machine.md)与[协作 Contract](../development/contract/application-state-machine.md)：状态边、生命周期、请求校验、去重和仲裁。
- [Confirmed] [统一游戏动作与 Tick 输入 Spec](../development/component/game-action-input.md)、[UI 动作输入 Spec](../development/component/ui-action-input.md)、[本地输入采样器 Spec](../development/component/local-input-sampler.md)与[采样 Contract](../development/contract/local-input-sampling.md)：规则动作、UI 动作、输入上下文、参与者槽位、设备采样及归一化边界。
- [Confirmed] [固定频率规则调度 Contract](../development/contract/fixed-tick-simulation.md)：fixed schedule 的频率、阶段顺序、唯一规则推进路径，以及 `FixedGameSet::Input`/`Rules` 仅在 `AppState::Match` 执行、`Paused` 停止对局 simulation 并在恢复后延续暂停前状态的运行边界。
- [Confirmed] [版本化运行数据加载 Contract](../development/contract/runtime-data-loading.md)、[本地化运行时 Spec](../development/component/localization-runtime.md)与[本机用户设置 Spec](../development/component/user-settings.md)：解析、版本、错误上下文、fallback、查询和持久化语义。
- [Confirmed] [PRD](../PRD.md)、[TDD](../TDD.md)与[assets/README.md](../../assets/README.md)：产品操作、workspace 职责、数据路径与版本头要求。

**设计基线：** 使用内存内 Component 测试覆盖纯行为，使用最小 Bevy `App` 或临时配置目录覆盖 Component Integration，仅以完整客户端装配覆盖启动、构建和主状态路径。  
**关键假设：**

- 参数化用例的一行代表一个逻辑用例，实施时每组测试数据都作为独立 case 执行并报告。
- 诊断由可捕获的结构化错误、事件或测试日志槽提供；具体 API 名称随实现确定。
- Windows 构建验收由 Windows CI runner 执行，Linux 构建验收由 Linux CI runner 执行。
- 生产客户端与自动化 startup smoke 共用同一项目根插件（方案 A）；smoke 侧不要求真实窗口，production build 侧使用与生产环境一致的 `DefaultPlugins` 配置。

**待确认问题：**

- CI 当前对 `x86_64-pc-windows-gnullvm` 只执行 build-only job（不运行 `cargo test`，见 `../../CLAUDE.md` Commands 一节），而自动化 startup smoke 需要实际运行二进制才能验证 `Boot → MainMenu`；Windows 侧的 smoke 用例（TC-057）目前缺少可执行该断言的 CI 环境，需要确认是否新增 Windows 执行 job，或该用例仅作为本地/未来验收目标记录。

- 设置写入失败的可注入文件系统边界与诊断载体尚未命名；实现测试需要提供等价的故障注入点。

## 范围边界

本稿覆盖文档中已具备可判定结果的行为。以下内容由后续设计提供 Test Basis 后增加用例：

- `PlayerActions` 的底层整数类型、bit 编号及稳定网络编码。
- `RuleProfile`、角色、Fever 题面及其它玩法配置的完整 schema 和语义约束。
- UI 返回、退出、再赛等新增状态边及其仲裁优先级。
- `Pause` 的具体 client action 类型、输入上下文和持久化绑定位置。
- 规则 tick 的网络帧号、回滚状态、渲染表现和音频结果。

`main.rs` 薄入口、模块所有权和禁止平行状态字段属于架构审查项；TC-051 对可机械验证的 crate 依赖边界提供回归保护。

## 测试点清单

### Component — Configuration

- 完整默认设置、缺失文件恢复、解析失败与不支持版本恢复（TC-001～TC-003）。
- 设置序列化往返、P1/P2 独立绑定与动作领域内绑定冲突结果（TC-004～TC-006）。
- 英文默认语言、当前目录查询、英文回退、key 占位与 catalog 校验（TC-007～TC-012；Concern: Content Validation）。
- 内存数据解析器的支持版本、解析错误、不支持版本与语义错误分类（TC-028～TC-031；Concern: Content Validation）。

### Component — Input

- `TickInputs` 的 0、2、8、9 人边界、slot 顺序与尾部清空不变量（TC-013～TC-016）。
- 三类动作冲突及其它组合保持规则（TC-017～TC-020；Concern: Determinism）。
- 连续 tick 动作表达与值语义（TC-021～TC-022；Concern: Determinism）。
- 本地采样的未绑定输入、玩家隔离、同义来源合并、持续动作和一次性动作时序（TC-023～TC-027）。
- UI 动作集合、输入上下文解释及与 GameAction 的领域隔离（TC-052）。

### Component — Client

- 应用状态初值、完整合法边、非法边与同状态 no-op（TC-034～TC-037）。
- 用户设置保存采用替换语义，失败时保留内存值与正式文件（TC-033）。

### Component Integration — Configuration

- RON/JSON 资源加载、资源上下文、fallback 值与错误保留（TC-032、TC-047；Concern: Content Validation、Smoke）。

### Component Integration — Input

- 键盘/手柄采样经过统一归一化后形成 canonical `PlayerActions`（TC-027；Concern: Determinism）。
- fixed tick 中 `Input → Rules` 排序、单次消费和 Update 频率独立性（TC-042～TC-045；Concern: Determinism）。

### Component Integration — Client

- 状态生命周期、重复请求、已定义优先级与未知冲突诊断（TC-038～TC-041）。
- 启动屏障的四种状态组合及 fallback 后的 resolved 语义（TC-046～TC-047；Concern: Smoke）。
- `FixedGameSet::Input`/`Rules` 仅在 `AppState::Match` 执行、`Paused` 停止对局 simulation、`Paused → Match` 后从暂停前状态延续推进（TC-053～TC-055；Concern: Determinism）。

### System — Client

- 从 `Boot` 到 `Result` 再回主菜单的基础状态主路径（TC-048；Concern: Smoke）。
- Linux、Windows production client 构建与链接（TC-049～TC-050；Concern: Smoke）。
- Linux、Windows 复用项目根插件的自动化 startup smoke（TC-056～TC-057；Concern: Smoke）。
- workspace 依赖方向与 `core` 平台隔离（TC-051）。

## 设计方法与覆盖模型

| 方法 | 输入模型 / 风险 | 关联用例 |
| --- | --- | --- |
| 等价类划分 | 有效、缺失、malformed、unsupported、invalid 数据；已绑定与未绑定输入 | TC-002～TC-003、TC-012、TC-023、TC-028～TC-032 |
| 边界值分析 | participant 数量 `0/2/8/9`；fixed tick 前后事件边界 | TC-013～TC-016、TC-025～TC-026 |
| 判定表 | 输入冲突组合；启动任务 `Pending/Resolved` 组合 | TC-017～TC-020、TC-046 |
| 状态迁移 | 全部基础状态边、同状态、非法边、请求冲突、`Match`/`Paused` 对局 simulation 生命周期 | TC-034～TC-041、TC-048、TC-053～TC-055 |
| 场景法 | 设置保存恢复、资源加载 fallback、客户端主路径、跨平台构建与自动化启动 smoke | TC-004、TC-032～TC-033、TC-047～TC-050、TC-056～TC-057 |
| 变形测试 | Update 次数变化时 fixed tick 结果保持一致 | TC-045 |
| 错误猜测 | tick 间短按、持续按住一次性动作、保存 replace 失败、重复迁移请求 | TC-026、TC-033、TC-039 |

## 测试用例列表

| ID | 标题 | Priority | Test Level | Concern | Domain | 前置条件 | 操作/刺激 | 测试数据 | 预期结果 | Test Basis / 证据状态 |
| --- | --- | --- | --- | --- | --- | --- | --- | --- | --- | --- |
| TC-001 | 默认构造产生完整安全设置 | P0 | Component | — | Configuration；Client | 无设置输入 | 构造 `UserSettings::default()` 或等价默认值 | language=`en`；window=`Windowed`；master/sfx=`1.0`；vibration/performance=`true`；animation=`Normal/1.0`；P1/P2 默认绑定 | 所有字段存在且等于文档默认值，P1/P2 各有独立的 GameAction/UIAction 键盘与手柄绑定集合 | [Confirmed] [本机用户设置 Spec：默认值](../development/component/user-settings.md#默认值) |
| TC-002 | 缺失、malformed 与 unsupported 设置均恢复完整默认值并区分诊断 | P0 | Component | — | Configuration；Client | 可调用设置解析/恢复入口并捕获诊断 | 分别提交三类输入 | 文件不存在；`(`；`schema_version=255` | 三组结果均为完整默认设置；缺失文件走缺省结果；后两组分别留下 Parse、UnsupportedSchema 等价诊断 | [Confirmed] [本机用户设置 Spec：启动加载](../development/component/user-settings.md#启动加载) |
| TC-003 | 设置解析成功恢复全部持久化字段 | P1 | Component | — | Configuration；Client | 支持的 settings schema | 解析完整 RON | 非默认语言、窗口、音量、两名玩家各自的 GameAction/UIAction 键盘与手柄绑定、震动、角色演出、动画强度 | 结果逐字段等于输入；GameAction/UIAction 绑定保持各自类型；P1/P2 数据未互换或合并 | [Confirmed] [本机用户设置 Spec：数据模型](../development/component/user-settings.md#数据模型) |
| TC-004 | 设置序列化后重新加载保持值相等 | P0 | Component | — | Configuration；Client | 一份包含全部字段的非默认设置 | 序列化到内存，再解析序列化结果 | language=`zh-CN`；window 非默认；音量边界内非默认值；P1/P2 各两类动作的互异绑定 | 恢复值与原值逐字段相等；GameAction/UIAction 绑定保持独立；schema 版本存在 | [Confirmed] [本机用户设置 Spec：保存设置](../development/component/user-settings.md#保存设置) |
| TC-005 | P1/P2 两类动作的键盘与手柄绑定独立保存恢复 | P1 | Component | — | Input；Configuration | P1/P2 的 GameAction/UIAction 使用可区分映射 | 保存并恢复设置 | P1 GameAction::Left=`KeyA`、UIAction::Left=`KeyA`；P2 GameAction::Left=`ArrowLeft`、UIAction::Left=`ArrowLeft`；两名玩家各有对应手柄映射 | 两名玩家及两个动作领域的映射分别保持原值；修改任一玩家的任一动作领域不会覆盖其它映射 | [Confirmed] [本机用户设置 Spec：验收条件](../development/component/user-settings.md#验收条件) |
| TC-006 | 绑定冲突按玩家与动作领域划分 | P1 | Component | — | Input；Configuration | 已存在一个绑定 | 参数化查询新增绑定冲突 | 同一玩家 GameAction 中 `KeyA→Left` 后添加 `KeyA→Right`；同一玩家 `KeyA→GameAction::Left` 与 `KeyA→UIAction::Left`；另一玩家使用 `KeyA` | 第一组返回具名冲突；跨 GameAction/UIAction 组不冲突；另一玩家案例按独立配置范围处理；冲突组设置数据在 UI 决定前不被覆盖 | [Confirmed] [本机用户设置 Spec：输入绑定冲突](../development/component/user-settings.md#输入绑定冲突) |
| TC-007 | 无有效语言设置时本地化默认使用英文 | P0 | Component | — | Client；Configuration | 英文目录已构建，无有效用户 locale | 初始化 `Localization` 并查询已知 key | `main_menu.start` | 当前 locale 为 `en`，返回英文文本 | [Confirmed] [本地化运行时 Spec：默认语言](../development/component/localization-runtime.md#默认语言) |
| TC-008 | 当前语言存在 key 时直接返回当前语言文本 | P0 | Component | — | Client；Configuration | `zh-CN` 与 `en` 均含相同 key，当前 locale=`zh-CN` | 查询 key | `main_menu.start`：中文=`开始`，英文=`Start` | 返回 `开始`，无 missing-key 诊断 | [Confirmed] [本地化运行时 Spec：查询文本](../development/component/localization-runtime.md#查询文本) |
| TC-009 | 当前语言缺 key 时回退英文并记录诊断 | P0 | Component | — | Client；Configuration | 中文缺 key，英文含 key，当前 locale=`zh-CN` | 查询 key | `main_menu.settings` 仅英文=`Settings` | 返回 `Settings`；诊断包含 locale=`zh-CN` 与该 key | [Confirmed] [本地化运行时 Spec：查询文本](../development/component/localization-runtime.md#查询文本) |
| TC-010 | 当前语言和英文均缺 key 时返回 key 并记录诊断 | P1 | Component | — | Client；Configuration | 两目录均缺目标 key | 查询 key | `missing.example` | 返回 `missing.example`；诊断包含 locale 与 key | [Confirmed] [本地化运行时 Spec：查询文本](../development/component/localization-runtime.md#查询文本) |
| TC-011 | 切换语言后后续查询使用新目录 | P1 | Component | — | Client；Configuration | 两种目录均加载，当前 locale=`en` | 查询一次，切换至 `zh-CN`，再次查询 | `main_menu.start` | 首次返回 `Start`，切换后返回中文文本，资源保持可只读查询 | [Confirmed] [本地化运行时 Spec：切换语言](../development/component/localization-runtime.md#切换语言) |
| TC-012 | catalog 有效、malformed 与 unsupported 输入得到对应解析结果 | P0 | Component | Content Validation | Configuration；Client | catalog 内存解析器 | 参数化解析三份 JSON | 有效 schema 1；截断 JSON；schema 255 | 有效输入得到 locale 和 messages；其余分别得到 Parse、UnsupportedSchema 等价错误，可由加载层 fallback | [Confirmed] [本地化运行时 Spec：加载文本目录](../development/component/localization-runtime.md#加载文本目录) |
| TC-013 | 零参与者构造空 TickInputs | P2 | Component | — | Input | `PlayerActions::EMPTY` 可用 | 用空序列构造 | `len=0` | 构造成功，`len=0`，8 个存储槽均为空 | [Confirmed] [统一游戏动作与 Tick 输入 Spec：TickInputs](../development/component/game-action-input.md#tickinputs) |
| TC-014 | 双人输入保持 participant slot 顺序并清空尾部 | P0 | Component | Determinism | Input | 两份不同动作集合 | 构造双人 `TickInputs` | slot 0=`Left`；slot 1=`HardDrop` | `len=2`；前两槽与输入顺序一致；槽 2～7 全部为空 | [Confirmed] [统一游戏动作与 Tick 输入 Spec：构造参与者输入](../development/component/game-action-input.md#构造参与者输入) |
| TC-015 | 八名参与者达到容量上限时构造成功 | P1 | Component | — | Input | 8 份可区分动作集合 | 按 slot 构造 | 8 个输入，使用动作组合区分相邻槽 | `len=8`，全部 slot 保持顺序和值 | [Confirmed] [统一游戏动作与 Tick 输入 Spec：构造参与者输入](../development/component/game-action-input.md#构造参与者输入) |
| TC-016 | 九名参与者超过容量时拒绝构造 | P0 | Component | — | Input | 9 份动作集合 | 调用构造入口 | `len=9` | 返回可判定错误，不产生截断后的 `TickInputs` | [Confirmed] [统一游戏动作与 Tick 输入 Spec：构造参与者输入](../development/component/game-action-input.md#构造参与者输入) |
| TC-017 | 左右同时成立归一化为无水平方向 | P0 | Component | Determinism | Input | raw actions 支持组合 | 归一化 | `Left + Right` | 结果同时清除 Left、Right | [Confirmed] [统一游戏动作与 Tick 输入 Spec：水平方向冲突](../development/component/game-action-input.md#水平方向冲突) |
| TC-018 | 双旋转同时成立归一化为无旋转 | P0 | Component | Determinism | Input | raw actions 支持组合 | 归一化 | `RotateClockwise + RotateCounterClockwise` | 结果同时清除两个旋转动作 | [Confirmed] [统一游戏动作与 Tick 输入 Spec：旋转方向冲突](../development/component/game-action-input.md#旋转方向冲突) |
| TC-019 | 软降与硬降同时成立时仅保留硬降 | P0 | Component | Determinism | Input | raw actions 支持组合 | 归一化 | `SoftDrop + HardDrop` | 结果含 HardDrop 且不含 SoftDrop | [Confirmed] [统一游戏动作与 Tick 输入 Spec：下落冲突](../development/component/game-action-input.md#下落冲突) |
| TC-020 | 独立动作与冲突外动作在归一化后保持 | P1 | Component | Determinism | Input | raw actions 支持六种动作 | 参数化归一化 | 单动作六组；`Left + SoftDrop + RotateClockwise`；三类冲突同时出现并附加无关动作 | 单动作和合法组合保持；冲突位按三项规则消解；无关动作不受影响；重复归一化结果相同 | [Confirmed] [统一游戏动作与 Tick 输入 Spec：逻辑动作归一化](../development/component/game-action-input.md#逻辑动作归一化) |
| TC-021 | 连续 tick 重复动作保持相同逻辑输入值 | P1 | Component | Determinism | Input | 三个连续 tick 输入容器 | 每个 tick 构造相同动作 | tick 100～102 均为 `SoftDrop` | 三个 tick 均含 SoftDrop，无额外 held/edge 状态影响值比较 | [Confirmed] [统一游戏动作与 Tick 输入 Spec：连续动作](../development/component/game-action-input.md#连续动作) |
| TC-022 | PlayerActions 支持复制与相等比较 | P2 | Component | Determinism | Input | 一份多动作 canonical 值 | 复制后比较并独立用于两个 `TickInputs` | `Left + SoftDrop` | 副本与原值相等，构造过程不改变任一值 | [Confirmed] [统一游戏动作与 Tick 输入 Spec：验收条件](../development/component/game-action-input.md#验收条件) |
| TC-023 | 键盘与手柄可产生六种动作且未绑定输入保持为空 | P1 | Component | — | Input；Client | 一名玩家为键盘与常见手柄分别绑定六个动作 | 参数化注入 12 个已绑定输入与 2 个未绑定输入并采样 | device=`keyboard/gamepad` × 六种 `GameAction`；未绑定 key/button | 每个已绑定 case 仅产生对应逻辑动作；未绑定 case 的 raw `PlayerActions` 为空 | [Confirmed] [本地输入采样器 Spec：捕获物理输入](../development/component/local-input-sampler.md#捕获物理输入) |
| TC-024 | P1/P2 独立绑定只影响对应 participant slot | P0 | Component | — | Input；Client | P1 Left=`KeyA`，P2 Left=`ArrowLeft` | 分别按下两键并在 fixed tick 采样 | 四组：只 A、只 ArrowLeft、同时、均未按 | 每组仅对应玩家 raw actions 含 Left；同时按下时两槽各含 Left；绑定互不覆盖 | [Confirmed] [本地输入采样器 Spec：不变量](../development/component/local-input-sampler.md#不变量) |
| TC-025 | 多个物理来源映射同一动作时合并为一个逻辑动作 | P1 | Component | — | Input；Client | 键盘 A 与手柄 Left 都映射到 Left | 两来源同时处于 pressed 并采样 | Keyboard A + Gamepad Left | raw actions 仅含一个 Left 位，无冲突或重复 | [Confirmed] [本地输入采样器 Spec：合并多个物理输入源](../development/component/local-input-sampler.md#合并多个物理输入源) |
| TC-026 | 持续动作按 fixed 边界 pressed 状态采样 | P0 | Component | — | Input；Client | 参数化 Left、Right、SoftDrop | 执行按住跨 3 tick、tick 间短按短放、下个 tick 前松开三种时序 | fixed tick=`T0/T1/T2` | 按住时三个 tick 都含动作；完整发生于 tick 间的短按不产生动作；边界前松开后下一 tick 不含动作 | [Confirmed] [本地输入采样器 Spec：持续动作采样](../development/component/local-input-sampler.md#持续动作采样) |
| TC-027 | 一次性动作每次 press edge 只提交一次并经过 core 归一化 | P0 | Component Integration | Determinism | Input；Client | 参数化 HardDrop、双旋转；最小采样与 core 输入装配 | 执行 tick 间短按短放、按住跨 3 tick、松开后再次按下；另注入双旋转与软硬降冲突 | 明确的 press/release/tick 序列 | 每个 press edge 在最近后续 tick 产生一次；持续按住不重复；第二次按下再产生一次；raw 冲突经 core 得到 canonical 冲突结果 | [Confirmed] [本地输入采样 Contract：协作时序](../development/contract/local-input-sampling.md#协作时序) |
| TC-028 | 支持版本的内存 RON 数据解析为 typed data | P0 | Component | Content Validation | Configuration | `core::config` 内存解析器 | 解析最小 `rules.stub.ron` 等价内容 | `schema_version=1` 与当前最小合法字段 | 返回 typed data，解析器不读取文件系统或 Bevy 资源 | [Confirmed] [版本化运行数据加载 Contract：双方承诺](../development/contract/runtime-data-loading.md#双方承诺) |
| TC-029 | malformed RON/JSON 返回 Parse typed error | P0 | Component | Content Validation | Configuration | core/client 两类内存解析器 | 参数化提交损坏文本 | 截断 RON；截断 JSON | 两组均返回 Parse 类错误且保留底层原因 | [Confirmed] [版本化运行数据加载 Contract：错误语义](../development/contract/runtime-data-loading.md#错误语义) |
| TC-030 | 未支持 schema 返回 UnsupportedSchema typed error | P0 | Component | Content Validation | Configuration | 已知仅支持 schema 1 | 参数化解析 RON/JSON | `schema_version=255` | 两组均返回 UnsupportedSchema，错误携带实际版本 | [Confirmed] [版本化运行数据加载 Contract：错误语义](../development/contract/runtime-data-loading.md#错误语义) |
| TC-031 | catalog locale 不属于支持集合时返回 InvalidData | P1 | Component | Content Validation | Configuration；Client | 本地化 catalog 内存解析器支持 schema 1，客户端支持 locale 集合为 `zh-CN`、`en` | 解析结构合法、schema 受支持且 locale 不受支持的 JSON catalog | `schema_version=1`；`locale=fr`；`messages={}` | 返回 InvalidData；错误标明 locale 必须属于当前支持集合，并保留实际值 `fr` | [Confirmed] [本地化运行时 Spec：语义验证](../development/component/localization-runtime.md#语义验证)；[版本化运行数据加载 Contract：错误语义](../development/contract/runtime-data-loading.md#错误语义) |
| TC-032 | 资源加载成功与四类失败均形成带上下文的 resolution | P0 | Component Integration | Content Validation | Configuration；Client | 最小 Bevy Asset app、内置默认值与临时 asset root | 参数化加载有效、缺失、malformed、unsupported、invalid 资源 | 有效及前三类失败使用 `assets/data/*.ron` 与 `assets/i18n/*.json` 等价 fixture；invalid 使用 schema 1、`locale=fr` 的 catalog | 有效资源为 `Loaded(typed_data)`；四类失败为 `Fallback { value: built-in default, error }`；invalid 的 typed cause 为 InvalidData；error 含 path、category、typed cause；两类结果均 resolved | [Confirmed] [版本化运行数据加载 Contract：协作时序](../development/contract/runtime-data-loading.md#协作时序)；[本地化运行时 Spec：语义验证](../development/component/localization-runtime.md#语义验证) |
| TC-033 | 平台配置目录中的原子保存成功可恢复，replace 失败保留正式文件与内存值 | P0 | Component Integration | — | Configuration；Client | 注入平台配置根目录、旧正式设置、可注入 rename/replace 失败 | 解析设置路径并执行成功保存、重载；再更新内存值并令 replace 失败 | 旧 language=`en`；新 language=`zh-CN` | 正式路径位于平台配置根目录；成功重载得到新值且无不完整文件；失败返回可观察错误，内存保持新值，正式文件仍为旧值 | [Confirmed] [本机用户设置 Spec：保存设置](../development/component/user-settings.md#保存设置) |
| TC-034 | 注册应用状态机后初始状态为 Boot | P0 | Component | — | Client | 最小 Bevy `App` 注册状态插件 | 完成初始化/首个必要 schedule | 无 | 当前唯一 `AppState=Boot` | [Confirmed] [应用状态机 Spec：初始化](../development/component/application-state-machine.md#初始化) |
| TC-035 | 基础状态表的每条合法边均可提交 | P0 | Component | — | Client | 参数化设置当前状态并注册迁移入口 | 对每条边提交请求并运行状态提交 schedule | Boot→MainMenu；MainMenu→ModeSelect；ModeSelect→CharacterSelect；CharacterSelect→Match；Match→Paused；Paused→Match；Match→Result；Result→MainMenu | 每组仅提交目标状态，当前状态变为目标 | [Confirmed] [应用状态机 Spec：有效状态转移](../development/component/application-state-machine.md#有效状态转移) |
| TC-036 | 表外状态边被拒绝并记录非法边诊断 | P0 | Component | — | Client | 每种当前状态可独立构造 | 参数化提交未列入该状态允许目标的请求 | 至少每个源状态一个非法目标，含 Boot→Match、Paused→Result、Result→Match | 当前状态和 `NextState` 均未改变；每组诊断包含 source、target、cause | [Confirmed] [应用状态机 Spec：请求迁移](../development/component/application-state-machine.md#请求迁移) |
| TC-037 | 同状态请求为 no-op 且无生命周期或非法边诊断 | P0 | Component | — | Client | 为 OnExit/OnEnter 安装计数器 | 对七种状态分别请求自身 | Boot→Boot … Result→Result | `NextState` 未写入；当前状态保持；进入/退出计数不增；无非法边诊断 | [Confirmed] [应用状态机 Spec：同状态请求](../development/component/application-state-machine.md#同状态请求) |
| TC-038 | 合法迁移实际提交后各触发一次退出与进入生命周期 | P1 | Component Integration | — | Client | 最小 Bevy App 为源/目标状态注册观察器 | 提交一条合法边并运行完整状态提交周期 | MainMenu→ModeSelect | OnExit(MainMenu)=1，OnEnter(ModeSelect)=1，消费者观察到目标状态所需预置数据 | [Confirmed] [应用状态机协作 Contract：协作时序](../development/contract/application-state-machine.md#协作时序) |
| TC-039 | 同周期重复目标请求合并为一次迁移 | P1 | Component Integration | — | Client | 当前 Match，生命周期计数器已注册 | 同周期提交两份 Result 请求 | 两个 `MatchCompleted` | 仅写入/提交一次 Match→Result；生命周期各触发一次 | [Confirmed] [应用状态机协作 Contract：重复目标](../development/contract/application-state-machine.md#重复目标) |
| TC-040 | MatchCompleted 与 PauseRequested 同周期时 Result 获胜 | P0 | Component Integration | — | Client | 当前 Match | 同周期以两种顺序提交两个请求 | Result/MatchCompleted；Paused/PauseRequested | 两种顺序最终均进入 Result，未进入 Paused | [Confirmed] [应用状态机协作 Contract：冲突目标](../development/contract/application-state-machine.md#冲突目标) |
| TC-041 | 无优先级的不同目标冲突拒绝整组并记录诊断 | P0 | Component Integration | — | Client | Arbiter 的冲突决策入口接收已通过合法性筛选的候选请求 | 直接提交两个无已定义优先级的不同候选目标 | candidate A=`ModeSelect/StartGame`；candidate B=`Result/ReturnToMainMenu`；priority lookup=`None` | 决策结果不写入 NextState；冲突诊断包含两个 target 与 cause | [Confirmed] [应用状态机协作 Contract：冲突目标](../development/contract/application-state-machine.md#冲突目标) |
| TC-042 | fixed schedule 配置为 60Hz | P0 | Component Integration | Determinism | Client | 最小客户端 app 注册 simulation 插件并转移至 `AppState::Match` | 检查 fixed time 配置并推进精确时长 | timestep=`1/60s`；elapsed=`1s` | 配置周期等于 `1/60s`；受控时间推进产生 60 次规则机会 | [Confirmed] [固定频率规则调度 Contract：调度约束](../development/contract/fixed-tick-simulation.md#调度约束) |
| TC-043 | 每个 fixed tick 严格执行 Input 后 Rules | P0 | Component Integration | Determinism | Input；Client | Input/Rules 测试 system 向 trace 写标记，app 已处于 `AppState::Match` | 推进多个 fixed tick | 3 ticks | trace 精确为 `[Input, Rules] × 3`，每个 Rules 都能读取同 tick Input 产物 | [Confirmed] [固定频率规则调度 Contract：Fixed System Set](../development/contract/fixed-tick-simulation.md#fixed-system-set) |
| TC-044 | 每个 fixed tick 只形成并消费一次 TickInputs | P0 | Component Integration | Determinism | Input；Client | 输入生产和规则消费均带 tick 序号/计数器，app 已处于 `AppState::Match` | 推进 120 ticks | 每 tick 使用可区分输入标记 | 生产数=消费数=120；每个序号恰好消费一次；无跨 tick 复用或漏用 | [Confirmed] [固定频率规则调度 Contract：调度约束](../development/contract/fixed-tick-simulation.md#调度约束) |
| TC-045 | Update 无法推进规则且分段次数变化保持 fixed 结果一致 | P0 | Component Integration | Determinism | Client | 两个相同初始 app 均已转移至 `AppState::Match`，具有相同时间/输入脚本 | 先执行不触发 fixed tick 的 Update burst；再令 app A 均匀分段、app B 不均匀分段推进相同 fixed 时间 | 100 次 Update-only；总 fixed 时间 2s；相同量化输入；不同 Update 次数 | Update-only 后规则状态不变；两者 fixed tick 数、输入序列、规则计数/checksum 完全相等 | [Confirmed] [固定频率规则调度 Contract：调度约束](../development/contract/fixed-tick-simulation.md#调度约束) |
| TC-046 | Boot 仅在设置与本地化均 Resolved 时请求 MainMenu | P0 | Component Integration | Smoke | Client | 当前 Boot，可设置 `BootstrapStatus` | 覆盖四种 Pending/Resolved 组合并运行协调 system | PP、PR、RP、RR | 前三组保持 Boot 且无迁移请求；RR 只产生一次 Boot→MainMenu 请求 | [Confirmed] [游戏基础设施运行架构：启动准备](../development/system/game-infrastructure-architecture.md#启动准备) |
| TC-047 | 设置与本地化加载失败经 fallback 后仍解除启动屏障 | P0 | Component Integration | Smoke；Content Validation | Configuration；Client | 最小启动 app，设置与 catalog 均使用失败 fixture | 完成两类加载与启动协调 | malformed settings；missing/unsupported locale catalog | 两项 resolution 均含可用默认值和原始诊断并标记 Resolved；应用进入 MainMenu；查询设置与文本安全可用 | [Confirmed] [游戏基础设施运行架构：主流程](../development/system/game-infrastructure-architecture.md#主流程) |
| TC-048 | 完整基础状态主路径保持单一顶层状态并准备目标数据 | P0 | System | Smoke | Client；Match Flow | 已装配客户端，启动资源可 resolved，各请求方可用测试驱动替代 UI | 完成启动并依次触发开始、模式确认、角色确认、暂停、继续、比赛结束、返回主菜单 | Boot→MainMenu→ModeSelect→CharacterSelect→Match→Paused→Match→Result→MainMenu | 每步仅有一个当前 AppState；目标数据在进入前可读；各状态消费者仅在对应生命周期运行 | [Confirmed] [游戏基础设施运行架构：状态与分支](../development/system/game-infrastructure-architecture.md#状态与分支) |
| TC-049 | Linux 目标构建并链接 production client | P0 | System | Smoke | Client | Linux CI runner 与发布支持的 Rust toolchain | 使用生产 Bevy plugin 配置（`DefaultPlugins` + 项目根插件）构建 workspace/client | Linux x86_64；默认功能集合 | 编译、链接、插件装配成功，产出可执行的 production 二进制；本用例不要求运行到 MainMenu（运行路径由 TC-056 覆盖） | [Confirmed] [游戏基础设施运行架构：Production build](../development/system/game-infrastructure-architecture.md#production-build) |
| TC-050 | Windows 目标构建并链接 production client | P0 | System | Smoke | Client | Windows CI runner 与发布支持的 Rust toolchain | 使用生产 Bevy plugin 配置（`DefaultPlugins` + 项目根插件）构建 workspace/client | Windows x86_64；默认功能集合 | 编译、链接、插件装配成功，产出可执行的 production 二进制；本用例不要求运行到 MainMenu | [Confirmed] [游戏基础设施运行架构：Production build](../development/system/game-infrastructure-architecture.md#production-build) |
| TC-051 | workspace 依赖图保持 client/net 指向 core 且 core 与平台运行时隔离 | P1 | System | — | Client | 可读取 Cargo metadata 与各 crate manifest | 生成依赖图并检查禁止依赖集合，同时独立构建/测试 core | 禁止 core 直接依赖 Bevy、窗口、文件系统路径能力、网络 crate；禁止 core→client/net | client→core 与 net→core 可成立；core 禁止集合为空并可独立测试；依赖图无反向边 | [Confirmed] [游戏基础设施运行架构：架构职责](../development/system/game-infrastructure-architecture.md#架构职责) |
| TC-052 | 同一物理输入按上下文产生独立领域动作 | P0 | Component | — | Input；Client | `KeyA` 分别绑定 `GameAction::Left` 与 `UIAction::Left`，可切换输入上下文 | 分别在 Match gameplay context 与 Menu UI context 注入 `KeyA`；参数化检查 UIAction 六个枚举值的键盘和手柄绑定 | context=`Match/Menu`；UI actions=`Left/Right/Up/Down/Confirm/Back` | Match 中只产生 `GameAction::Left` 并可进入规则输入；Menu 中只产生 `UIAction::Left` 并用于 UI；六个 UIAction 均可由两类设备产生；两个动作类型和输出容器保持独立 | [Confirmed] [UI 动作输入 Spec：验收条件](../development/component/ui-action-input.md#验收条件) |
| TC-053 | fixed tick 仅在 `AppState::Match` 执行，其余状态不产生 Input/Rules | P0 | Component Integration | Determinism | Client | 最小客户端 app 注册状态机与 simulation 插件，Input/Rules 执行计数可观测 | 参数化在 `Boot`、`MainMenu`、`Paused` 停留并推进 fixed 时间，再转移至 `Match` 推进相同 fixed 时间 | 非 Match 状态各推进 1s；随后 Match 状态推进 1s | 非 Match 状态下 Input/Rules 执行计数为 0；进入 Match 后计数按 60Hz 正常递增 | [Confirmed] [固定频率规则调度 Contract：运行边界](../development/contract/fixed-tick-simulation.md#运行边界) |
| TC-054 | `Match → Paused` 后对局 simulation 立即停止 | P0 | Component Integration | — | Client | 当前 `Match`，Input/Rules 执行计数器已运行若干 tick | 提交 `Paused` 请求并运行状态提交，随后推进 fixed 时间 | 转移前计数=N；转移后推进 1s | 状态转移当拍起不再产生新的 Input/Rules 执行；转移后计数保持为 N，不随时间推进增加 | [Confirmed] [固定频率规则调度 Contract：Pause 行为](../development/contract/fixed-tick-simulation.md#pause-行为) |
| TC-055 | `Paused → Match` 恢复后规则状态从暂停前继续推进 | P0 | Component Integration | Determinism | Client | Match 中已推进至可观察的非初始规则状态 S，并记录已消费 tick 数 | 转移至 `Paused`、停留任意 fixed 时间、转移回 `Match` 并再推进若干 tick | 暂停前状态=S；暂停期间尝试推进 2s；恢复后再推进 3 ticks | 恢复后的起始规则状态等于 S，不因暂停期间的时间流逝而重置或跳变；恢复后 tick 计数从暂停前的计数继续累加 | [Confirmed] [固定频率规则调度 Contract：Pause 行为](../development/contract/fixed-tick-simulation.md#pause-行为) |
| TC-056 | Linux 自动化 startup smoke 复用项目根插件跑通 Boot→MainMenu | P0 | System | Smoke | Client | Linux CI runner；最小 Bevy runtime + 与生产客户端相同的项目根插件，不含真实窗口依赖 | 运行可自动退出的 smoke harness | Linux x86_64 | 项目根插件装配成功；`AppState` 初始化为 `Boot`；`UserSettings` 与 `Localization` 完成 bootstrap resolution；应用到达 `MainMenu`；进程正常退出；全程不要求真实窗口交互 | [Confirmed] [游戏基础设施运行架构：自动化启动验收](../development/system/game-infrastructure-architecture.md#自动化启动验收) |
| TC-057 | Windows 自动化 startup smoke 复用项目根插件跑通 Boot→MainMenu | P2 | System | Smoke | Client | Windows 执行环境；最小 Bevy runtime + 与生产客户端相同的项目根插件，不含真实窗口依赖 | 运行可自动退出的 smoke harness | Windows x86_64 | 项目根插件装配成功；`AppState` 初始化为 `Boot`；`UserSettings` 与 `Localization` 完成 bootstrap resolution；应用到达 `MainMenu`；进程正常退出；全程不要求真实窗口交互 | [Inferred] [游戏基础设施运行架构：自动化启动验收](../development/system/game-infrastructure-architecture.md#自动化启动验收)；当前 CI 的 Windows job 为 build-only，本用例的实际执行环境待确认（见"待确认问题"），Priority 暂降为 P2 |

## 风险查漏

| 风险领域 | 覆盖结论 |
| --- | --- |
| 状态与流程 | 主路径、全部基础边、非法边、同状态、重复、显式优先级、未知冲突和启动屏障均有直接用例；`Match ⇄ Paused` 对局 simulation 的启停与状态延续现已有直接用例（TC-053～TC-055）。 |
| Configuration | schema 支持、解析、版本、语义错误、资源路径、fallback、设置默认值与原子保存均有直接用例；本地化 catalog locale 语义约束（`InvalidData`）已覆盖（TC-031）。 |
| Client 与 Input | 六种 GameAction、六种 UIAction、输入上下文隔离、slot 容量、玩家隔离、多来源、fixed 边界与 edge 保留均有直接用例；Pause 的具体 client action 分类（是否属于 UIAction、bit 归属）留给后续输入上下文设计，仅其对 simulation 生命周期的可观察影响已被 TC-054～TC-055 覆盖。 |
| Determinism | 输入归一化、fixed 阶段顺序、单次消费、Update 分段变形关系及 `Paused` 恢复后状态延续（不重置/不跳变）均有直接用例。 |
| Rules 与数值 | 本轮只覆盖规则调用边界；玩法公式由后续规则设计覆盖。 |
| AI | 统一 `PlayerActions`/`TickInputs` 类型边界已覆盖；AI 动作合法性由 AI 设计覆盖。 |
| Network | crate 依赖方向与统一输入容量已覆盖；握手、同步、回滚和断线由 R2 设计覆盖。 |
| CI 环境匹配 | Linux production build、Linux 自动化 smoke 均有对应 CI 执行路径（`cargo build`/`cargo test --workspace`）；Windows production build 对应 CI 的 build-only job，但 Windows 自动化 smoke（TC-057）目前没有可运行 `cargo test` 的 Windows CI job 承接，属于已知缺口（见"待确认问题"）。 |

## 实施顺序

1. 先实现 TC-013～TC-022 的纯 core 输入测试，固定最底层数据与归一化语义。
2. 实现 TC-001～TC-012、TC-028～TC-033 的解析、fallback 与持久化测试。
3. 实现 TC-023～TC-027、TC-034～TC-045、TC-052 的最小 Bevy App 组件及组件集成测试。
4. 实现 TC-053～TC-055 的 `Match`/`Paused` 对局 simulation 生命周期测试，复用 TC-042～TC-045 已建立的 fixed tick 观测手段。
5. 最后接入 TC-046～TC-051、TC-056～TC-057 的启动、主路径、平台 CI 与依赖边界测试；TC-057 的落地依赖"待确认问题"中 Windows 执行环境的结论，可先以 `#[ignore]` 或本地专项验证的方式占位。

## 审核记录

| 审核人 | 日期 | 结论 | 备注 |
| --- | --- | --- | --- |
| | | 通过 / 需修订 | |
