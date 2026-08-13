# 测试用例设计：游戏基础设施

**关联设计：** [游戏基础设施运行架构](../development/system/game-infrastructure-architecture.md)、[模块设计](../development/design/)

**关联实现：** `../../crates/game_core`、`../../crates/client`、`../../crates/net`、`../../assets/data`、`../../assets/i18n`

## 需求理解摘要

**功能：** 建立 Bevy 客户端的应用状态、统一输入、60Hz 规则调度、版本化数据、本地化、用户设置与 workspace 职责边界。
**测试性质：** 新功能（含既有设计的补充变更）
**Test Basis：**

- [游戏基础设施运行架构](../development/system/game-infrastructure-architecture.md)：启动屏障、主流程、crate 边界与三个启动验收面。
- [应用状态机](../development/design/application-state-machine.md)：状态边、生命周期、请求校验、去重与仲裁。
- [统一游戏动作与 Tick 输入](../development/design/game-action-input.md)：规则动作、位编码、参与者槽位与归一化。
- [UI 交互动作](../development/design/ui-action-input.md)：UI 动作、输入上下文与固定绑定表。
- [本地输入采样](../development/design/local-input-sampling.md)：设备采样时机、press edge、设备绑定与摇杆判定。
- [固定频率规则调度](../development/design/fixed-tick-simulation.md)：60Hz、阶段顺序、唯一规则推进路径与 `Paused` 运行边界。
- [版本化运行数据加载](../development/design/runtime-data-loading.md)：解析、版本、错误上下文与 fallback。
- [本地化运行时](../development/design/localization-runtime.md)：查询、回退与诊断。
- [本机用户设置](../development/design/user-settings.md)：默认值、持久化与绑定冲突范围。
- [PRD](../PRD.md)、[TDD](../TDD.md)、[assets/README.md](../../assets/README.md)：产品操作、workspace 职责、数据路径与版本头要求。

**设计基线：** 使用内存内 Component 测试覆盖纯行为，使用最小 Bevy `App` 或临时配置目录覆盖 Component Integration，仅以完整客户端装配覆盖启动、构建和主状态路径。
**关键假设：**

- 参数化用例的一行代表一个逻辑用例，实施时每组测试数据都作为独立 case 执行并报告。
- 诊断以实现可提供的错误、事件或日志等形式观测；测试只核对 Confirmed 文档要求的分类与上下文，不固定错误载体或 API 名称。
- Production build（TC-049）与真实窗口有界启动（TC-074）由 `release.yml`（`workflow_dispatch` 手动触发）的 Linux runner 执行；自动化 startup smoke（TC-056）与其余测试由 `test.yml`（目标分支为 `main` 的 pull request 触发）的 Linux runner 执行。开发分支的推送不触发 CI，该阶段由本地执行覆盖，见 [TDD](../TDD.md) §7.1、§7.2。
- 用例编号稳定：移除的用例留下编号空档，既不重新编号也不复用编号。当前空档为 TC-041、TC-050、TC-057。
- 除 TC-074 外的全部用例不依赖真实显示环境；TC-074 需要虚拟显示与软件渲染后端，因此不进入拉取请求门禁。
- 生产客户端与自动化 startup smoke 共用同一项目根插件；smoke 侧不要求真实窗口，production build 侧使用与生产环境一致的 `DefaultPlugins` 配置；自动退出方式由实现选择。

## 范围边界

本稿覆盖文档中已具备可判定结果的行为。以下内容由后续设计提供 Test Basis 后增加用例：

- `PlayerActions` 位编码之上的稳定网络报文格式（bit 编号本身已锁定，见 TC-060）。
- `RuleProfile`、角色、Fever 题面及其它玩法配置的完整 schema 和语义约束。
- UI 返回、退出、再赛等新增状态边及其仲裁优先级。
- 无已定义优先级的不同目标冲突：新增真实且可构造的冲突状态边时再增加用例。
- 方向输入的连发（DAS/ARR）节奏：采样不提供连发（TC-064 锁定该结论），重复移动规则由玩法设计定义。
- 规则 tick 的网络帧号、回滚状态、渲染表现和音频结果。

`main.rs` 薄入口、模块所有权和禁止平行状态字段属于架构审查项；TC-051 对可机械验证的 crate 依赖边界提供回归保护。

## 测试点清单

### Component — Configuration

- 完整默认设置、缺失文件恢复、解析失败与不支持版本恢复（TC-001～TC-003）。
- 设置序列化往返、P1/P2 四项可配置 `GameAction` 绑定与冲突检测范围、固定绑定不进入 `PlayerInputBindings`（TC-004～TC-006、TC-059）。
- 英文默认语言、当前目录查询、英文回退、key 占位与 catalog 校验（TC-007～TC-012；TC-012 Concern: Content Validation）。
- 内存数据解析器的支持版本、解析错误、不支持版本与语义错误分类（TC-028～TC-031；Concern: Content Validation）。

### Component — Input

- `TickInputs` 的 0、2、8、9 人边界、slot 顺序与尾部清空不变量（TC-013～TC-016）。
- 三类动作冲突及其它组合保持规则（TC-017～TC-020）。
- 连续 tick 动作表达与值语义（TC-021～TC-022）。
- 本地采样的未绑定输入、玩家隔离、同义来源合并、持续动作和一次性动作完整时序（TC-023～TC-027A）。
- UI 动作集合、输入上下文解释及与 GameAction 的领域隔离（TC-052）。
- `PlayerActions` 公开解码入口的保留位不变量（TC-072；Concern: Determinism）。

### Component — Client

- 纯状态表的完整合法边与非法边判断（TC-035～TC-036）。

### Component Integration — Configuration

- RON/JSON 资源加载、资源上下文、fallback 值与错误保留（TC-032、TC-047；Concern: Content Validation、Smoke）。
- 项目根插件装配后消费者取得 resolved typed data（TC-073；Concern: Smoke）。

### Component Integration — Input

- sampler 输出经过 game_core 归一化后形成 canonical `PlayerActions`（TC-027B）。
- fixed tick 的 60Hz 配置、`Input → Rules` 排序、单次消费及 fixed 结果等价性（TC-042～TC-045；仅 TC-045 Concern: Determinism）。
- 生产主调度下同帧输入进入当帧 fixed tick，以及单帧多 tick 的共享采样语义（TC-067～TC-068）。
- 设备适配层保留同帧内完成的 press edge（TC-069）。
- 手柄断开的采样残留清理与设备↔玩家绑定稳定性（TC-070～TC-071）。

### Component Integration — Client

- Bevy 状态初始化、同状态生命周期、合法迁移生命周期、重复请求与已定义优先级（TC-034、TC-037～TC-040）。
- 用户设置保存采用替换语义，失败时保留内存值与正式文件（TC-033）。
- 启动屏障的四种状态组合及 fallback 后的 resolved 语义（TC-046～TC-047；Concern: Smoke）。
- `FixedGameSet::Input`/`Rules` 仅在 `AppState::Match` 执行、`Paused` 停止对局 simulation、`Paused → Match` 后从暂停前状态延续推进（TC-053～TC-055）。
- `Match` 语境下固定 Start 按键由 `client::input` 直接提出 `PauseRequested`，不经过 `UIAction`/`GameAction`（TC-058）。

### System — Client

- 从 `Boot` 到 `Result` 再回主菜单的基础状态主路径（TC-048；Concern: Smoke）。
- Linux production client 构建与链接（TC-049；Concern: Smoke）。
- Linux 复用项目根插件的自动化 startup smoke（TC-056；Concern: Smoke）。
- workspace 依赖方向与 `game_core` 平台隔离（TC-051）。
- 真实 production 二进制的有界启动（TC-074；Concern: Smoke）。

## 设计方法与覆盖模型

| 方法 | 输入模型 / 风险 | 关联用例 |
| --- | --- | --- |
| 等价类划分 | 有效、缺失、malformed、unsupported、invalid 数据；已绑定与未绑定输入；可配置与固定绑定动作；合法编码与保留位置位编码 | TC-002～TC-003、TC-012、TC-023、TC-028～TC-032、TC-059、TC-072 |
| 边界值分析 | participant 数量 `0/2/8/9`；fixed tick 前后事件边界；帧边界与单帧内的 tick 数 | TC-013～TC-016、TC-026～TC-027A、TC-067～TC-068 |
| 判定表 | 输入冲突组合；启动任务 `Pending/Resolved` 组合 | TC-017～TC-020、TC-046 |
| 状态迁移 | 全部基础状态边、同状态、非法边、已定义请求仲裁、`Match`/`Paused` 对局 simulation 生命周期 | TC-034～TC-040、TC-048、TC-053～TC-055 |
| 场景 / 协作路径 | 设置保存恢复、资源加载 fallback、客户端主路径、Pause 直接触发、production 构建与启动 smoke、生产数据加载生命周期 | TC-004、TC-027B、TC-032～TC-033、TC-047～TC-049、TC-056、TC-058、TC-073～TC-074 |
| 变形测试 | Update 次数变化时 fixed tick 结果保持一致 | TC-045 |
| 错误猜测 | tick 间短按、同帧内完成的短按、持续按住一次性动作、保存 replace 失败、重复迁移请求、输入过程中设备断开 | TC-026～TC-027A、TC-033、TC-039、TC-069～TC-070 |

## 测试用例列表

| ID | 标题 | Priority | Test Level | Concern | Domain | 前置条件 | 操作/刺激 | 测试数据 | 预期结果 | Test Basis / 证据状态 |
| --- | --- | --- | --- | --- | --- | --- | --- | --- | --- | --- |
| TC-001 | 默认构造产生完整安全设置 | P1 | Component | — | Configuration；Client | 无设置输入 | 构造 `UserSettings::default()` 或等价默认值 | language=`en`；window=`Windowed`；master/sfx=`1.0`；vibration=`true`；P1/P2 默认绑定范围 | 文档已定义默认值的字段完整且取值正确；P1/P2 的 `PlayerInputBindings` 均覆盖 `SoftDrop`/`HardDrop`/`RotateClockwise`/`RotateCounterClockwise` 四项可配置 `GameAction`，不含 `UIAction` 或 `GameAction::Left`/`Right`；四项默认绑定均非空，取值按[默认输入绑定](../development/design/user-settings.md#默认输入绑定)定义 | [Confirmed] [本机用户设置：默认值](../development/design/user-settings.md#默认值)；[UI 交互动作：物理绑定关系](../development/design/ui-action-input.md#物理绑定关系) |
| TC-002 | 缺失、malformed 与 unsupported 设置均恢复完整默认值并区分诊断 | P1 | Component | — | Configuration；Client | 可调用设置解析/恢复入口并观察结果与诊断 | 分别提交三类输入 | 文件不存在；`(`；`schema_version=255` | 三组结果均为完整默认设置；缺失文件走缺省结果；malformed 与 unsupported 分别留下可区分的解析类、版本不支持类诊断；诊断载体与具体类型由实现决定 | [Confirmed] [本机用户设置：启动加载](../development/design/user-settings.md#启动加载) |
| TC-003 | 设置解析成功恢复全部持久化字段 | P1 | Component | — | Configuration；Client | 支持的 settings schema | 解析完整 RON | 非默认语言、窗口、音量、两名玩家各自四项可配置 `GameAction` 的键盘与手柄绑定、震动 | 结果逐字段等于输入；只解析四项可配置 `GameAction` 绑定，不含 `UIAction` 或 `GameAction::Left`/`Right` 字段；P1/P2 数据未互换或合并 | [Confirmed] [本机用户设置：数据模型](../development/design/user-settings.md#数据模型) |
| TC-004 | 设置序列化后重新加载保持值相等 | P1 | Component | — | Configuration；Client | 一份包含全部字段的非默认设置 | 序列化到内存，再解析序列化结果 | language=`zh-CN`；window 非默认；音量边界内非默认值；P1/P2 各四项可配置 `GameAction` 的互异绑定 | 恢复值与原值逐字段相等；四项可配置绑定往返一致；结果不含 `UIAction` 或 `GameAction::Left`/`Right` 字段；schema 版本存在 | [Confirmed] [本机用户设置：保存设置](../development/design/user-settings.md#保存设置) |
| TC-005 | P1/P2 四项可配置 `GameAction` 的键盘与手柄绑定独立保存恢复 | P1 | Component | — | Input；Configuration | P1/P2 的 `SoftDrop`/`HardDrop`/`RotateClockwise`/`RotateCounterClockwise` 使用可区分映射 | 保存并恢复设置 | P1 SoftDrop=`KeyS`、HardDrop=`KeyW`；P2 SoftDrop=`ArrowDown`、HardDrop=`ArrowUp`；两名玩家各有对应手柄映射 | 两名玩家的四项可配置绑定分别保持原值；修改任一玩家的绑定不会覆盖另一玩家；结果不含 `UIAction` 或 `GameAction::Left`/`Right` 绑定字段 | [Confirmed] [本机用户设置：数据模型](../development/design/user-settings.md#数据模型) |
| TC-006 | 绑定冲突检测限定在四项可配置 `GameAction` 范围内、按玩家划分 | P1 | Component | — | Input；Configuration | 已存在一个可配置绑定 | 参数化查询新增绑定冲突 | 同一玩家 `KeyA→SoftDrop` 后添加 `KeyA→HardDrop`；同一玩家 `KeyA→SoftDrop` 后添加 `KeyA→RotateClockwise`；另一玩家使用 `KeyA→SoftDrop` | 每组同玩家案例均返回具名冲突；另一玩家案例按独立配置范围处理，不与前者冲突；冲突组设置数据在 UI 决定前不被覆盖 | [Confirmed] [本机用户设置：输入绑定冲突](../development/design/user-settings.md#输入绑定冲突) |
| TC-007 | 无有效语言设置时本地化默认使用英文 | P1 | Component | — | Client；Configuration | 英文目录已构建，无有效用户 locale | 初始化 `Localization` 并查询已知 key | `main_menu.start` | 当前 locale 为 `en`，返回英文文本 | [Confirmed] [本地化运行时：数据模型](../development/design/localization-runtime.md#数据模型) |
| TC-008 | 当前语言存在 key 时直接返回当前语言文本 | P1 | Component | — | Client；Configuration | `zh-CN` 与 `en` 均含相同 key，当前 locale=`zh-CN` | 查询 key | `main_menu.start`：中文=`开始`，英文=`Start` | 返回 `开始`，无 missing-key 诊断 | [Confirmed] [本地化运行时：查询文本](../development/design/localization-runtime.md#查询文本) |
| TC-009 | 当前语言缺 key 时回退英文并记录诊断 | P1 | Component | — | Client；Configuration | 中文缺 key，英文含 key，当前 locale=`zh-CN` | 查询 key | `main_menu.settings` 仅英文=`Settings` | 返回 `Settings`；诊断包含 locale=`zh-CN` 与该 key | [Confirmed] [本地化运行时：查询文本](../development/design/localization-runtime.md#查询文本) |
| TC-010 | 当前语言和英文均缺 key 时返回 key 并记录诊断 | P1 | Component | — | Client；Configuration | 两目录均缺目标 key | 查询 key | `missing.example` | 返回 `missing.example`；诊断包含 locale 与 key | [Confirmed] [本地化运行时：查询文本](../development/design/localization-runtime.md#查询文本) |
| TC-011 | 切换语言后后续查询使用新目录 | P1 | Component | — | Client；Configuration | 两种目录均加载，当前 locale=`en` | 查询一次，切换至 `zh-CN`，再次查询 | `main_menu.start` | 首次返回 `Start`，切换后返回中文文本，资源保持可只读查询 | [Confirmed] [本地化运行时：切换语言](../development/design/localization-runtime.md#切换语言) |
| TC-012 | catalog 有效、malformed 与 unsupported 输入得到对应解析结果 | P1 | Component | Content Validation | Configuration；Client | catalog 内存解析器 | 参数化解析三份 JSON | 有效 schema 1；截断 JSON；schema 255 | 有效输入得到 locale 和 messages；其余分别得到 Parse、UnsupportedSchema 等价错误，可由加载层 fallback | [Confirmed] [本地化运行时：加载文本目录](../development/design/localization-runtime.md#加载文本目录) |
| TC-013 | 零参与者构造空 TickInputs | P2 | Component | — | Input | `PlayerActions::EMPTY` 可用 | 用空序列构造 | `len=0` | 构造成功，`len=0`，8 个存储槽均为空 | [Confirmed] [统一游戏动作与 Tick 输入：TickInputs](../development/design/game-action-input.md#tickinputs) |
| TC-014 | 双人输入保持 participant slot 顺序并清空尾部 | P1 | Component | — | Input | 两份不同动作集合 | 构造双人 `TickInputs` | slot 0=`Left`；slot 1=`HardDrop` | `len=2`；前两槽与输入顺序一致；槽 2～7 全部为空 | [Confirmed] [统一游戏动作与 Tick 输入：构造参与者输入](../development/design/game-action-input.md#构造参与者输入) |
| TC-015 | 八名参与者达到容量上限时构造成功 | P1 | Component | — | Input | 8 份可区分动作集合 | 按 slot 构造 | 8 个输入，使用动作组合区分相邻槽 | `len=8`，全部 slot 保持顺序和值 | [Confirmed] [统一游戏动作与 Tick 输入：构造参与者输入](../development/design/game-action-input.md#构造参与者输入) |
| TC-016 | 九名参与者超过容量时拒绝构造 | P2 | Component | — | Input | 9 份动作集合 | 调用构造入口 | `len=9` | 返回可判定错误，不产生截断后的 `TickInputs` | [Confirmed] [统一游戏动作与 Tick 输入：构造参与者输入](../development/design/game-action-input.md#构造参与者输入) |
| TC-017 | 左右同时成立归一化为无水平方向 | P1 | Component | — | Input | raw actions 支持组合 | 归一化 | `Left + Right` | 结果同时清除 Left、Right | [Confirmed] [统一游戏动作与 Tick 输入：水平方向冲突](../development/design/game-action-input.md#水平方向冲突) |
| TC-018 | 双旋转同时成立归一化为无旋转 | P1 | Component | — | Input | raw actions 支持组合 | 归一化 | `RotateClockwise + RotateCounterClockwise` | 结果同时清除两个旋转动作 | [Confirmed] [统一游戏动作与 Tick 输入：旋转方向冲突](../development/design/game-action-input.md#旋转方向冲突) |
| TC-019 | 软降与硬降同时成立时仅保留硬降 | P1 | Component | — | Input | raw actions 支持组合 | 归一化 | `SoftDrop + HardDrop` | 结果含 HardDrop 且不含 SoftDrop | [Confirmed] [统一游戏动作与 Tick 输入：下落冲突](../development/design/game-action-input.md#下落冲突) |
| TC-020 | 独立动作与冲突外动作在归一化后保持 | P1 | Component | — | Input | raw actions 支持六种动作 | 参数化归一化 | 单动作六组；`Left + SoftDrop + RotateClockwise`；三类冲突同时出现并附加无关动作 | 单动作和合法组合保持；冲突位按三项规则消解；无关动作不受影响；重复归一化结果相同 | [Confirmed] [统一游戏动作与 Tick 输入：逻辑动作归一化](../development/design/game-action-input.md#逻辑动作归一化) |
| TC-021 | 连续 tick 重复动作保持相同逻辑输入值 | P1 | Component | — | Input | 三个连续 tick 输入容器 | 每个 tick 构造相同动作 | tick 100～102 均为 `SoftDrop` | 三个 tick 均含 SoftDrop，无额外 held/edge 状态影响值比较 | [Confirmed] [统一游戏动作与 Tick 输入：连续动作](../development/design/game-action-input.md#连续动作) |
| TC-022 | PlayerActions 支持复制与相等比较 | P2 | Component | — | Input | 一份多动作 canonical 值 | 复制后比较并独立用于两个 `TickInputs` | `Left + SoftDrop` | 副本与原值相等，构造过程不改变任一值 | [Confirmed] [统一游戏动作与 Tick 输入：`PlayerActions`](../development/design/game-action-input.md#playeractions) |
| TC-023 | 固定方向输入与可配置动作输入均可采样，未映射输入保持为空 | P1 | Component | — | Input；Client | 一名玩家具备已确认的固定 Left/Right 物理输入，四项可配置动作已有可区分映射 | 参数化注入固定方向输入、四项已映射输入与未映射输入并采样 | device=`keyboard/gamepad`；action=六种 `GameAction` | 每个已确认输入 case 仅产生对应逻辑动作；未映射 case 的 raw `PlayerActions` 为空；Left/Right 不经用户可配置绑定 | [Confirmed] [本地输入采样：捕获物理输入](../development/design/local-input-sampling.md#捕获物理输入)；[UI 交互动作：物理绑定关系](../development/design/ui-action-input.md#物理绑定关系) |
| TC-024 | P1/P2 输入来源只影响对应 participant slot | P1 | Component | — | Input；Client | P1/P2 分别关联可区分的本地输入来源 | 分别产生两名玩家的固定 Left 方向输入并在 fixed tick 采样 | 四组：仅 P1、仅 P2、两者同时、均无输入 | 每组仅对应玩家 raw actions 含 Left；同时输入时两槽各含 Left；玩家输入状态互不覆盖；Left 使用固定绑定语义 | [Confirmed] [本地输入采样：设备与玩家绑定](../development/design/local-input-sampling.md#设备与玩家绑定)；[UI 交互动作：物理绑定关系](../development/design/ui-action-input.md#物理绑定关系) |
| TC-025 | 多个固定物理来源产生同一动作时合并为一个逻辑动作 | P1 | Component | — | Input；Client | 键盘与手柄的固定方向输入均可产生 Left | 两来源同时处于 pressed 并采样 | Keyboard Left source + Gamepad Left direction | raw actions 仅含一个 Left 位，无冲突或重复 | [Confirmed] [本地输入采样：合并多个物理输入源](../development/design/local-input-sampling.md#合并多个物理输入源)；[UI 交互动作：物理绑定关系](../development/design/ui-action-input.md#物理绑定关系) |
| TC-026 | 持续动作按 fixed 边界 pressed 状态采样 | P1 | Component | — | Input；Client | 参数化 Left、Right、SoftDrop | 执行按住跨 3 tick、tick 间短按短放、下个 tick 前松开三种时序 | fixed tick=`T0/T1/T2` | 按住时三个 tick 都含动作；完整发生于 tick 间的短按不产生动作；边界前松开后下一 tick 不含动作 | [Confirmed] [本地输入采样：持续动作采样](../development/design/local-input-sampling.md#持续动作采样) |
| TC-027A | 一次性动作按完整输入时序每次 press edge 只提交一次 | P1 | Component | — | Input；Client | 参数化 HardDrop 与两种旋转动作的采样行为 | 执行 tick 间短按短放、按住跨 3 tick、松开后再次按下 | 明确的 press/release/tick 序列 | 每个 press edge 在最近后续 tick 产生一次；持续按住不重复；松开后第二次按下再产生一次 | [Confirmed] [本地输入采样：一次性动作采样](../development/design/local-input-sampling.md#一次性动作采样) |
| TC-027B | sampler 输出经 game_core 归一化形成 canonical input | P0 | Component Integration | — | Input；Client | 最小 sampler 与 game_core 输入协作路径可运行 | 采样可形成冲突组合的物理输入，并将 raw `PlayerActions` 交给 game_core 归一化 | 双旋转；软降+硬降 | game_core 收到 sampler 的 raw 输出，并分别形成无旋转、仅 HardDrop 的 canonical `PlayerActions` | [Confirmed] [本地输入采样：协作](../development/design/local-input-sampling.md#协作) |
| TC-028 | 支持版本的内存 RON 数据解析为 typed data | P1 | Component | Content Validation | Configuration | `game_core::config` 内存解析器 | 解析最小 `rules.stub.ron` 等价内容 | `schema_version=1` 与当前最小合法字段 | 返回对应 typed data | [Confirmed] [版本化运行数据加载：边界](../development/design/runtime-data-loading.md#边界) |
| TC-029 | malformed RON/JSON 返回 Parse typed error | P2 | Component | Content Validation | Configuration | game_core/client 两类内存解析器 | 参数化提交损坏文本 | 截断 RON；截断 JSON | 两组均返回 Parse 类错误且保留底层原因 | [Confirmed] [版本化运行数据加载：错误语义](../development/design/runtime-data-loading.md#错误语义) |
| TC-030 | 未支持 schema 返回 UnsupportedSchema typed error | P2 | Component | Content Validation | Configuration | 已知仅支持 schema 1 | 参数化解析 RON/JSON | `schema_version=255` | 两组均返回 UnsupportedSchema，错误携带实际版本 | [Confirmed] [版本化运行数据加载：错误语义](../development/design/runtime-data-loading.md#错误语义) |
| TC-031 | catalog locale 不属于支持集合时返回 InvalidData | P1 | Component | Content Validation | Configuration；Client | 本地化 catalog 内存解析器支持 schema 1，客户端支持 locale 集合为 `zh-CN`、`en` | 解析结构合法、schema 受支持且 locale 不受支持的 JSON catalog | `schema_version=1`；`locale=fr`；`messages={}` | 返回 InvalidData；错误标明 locale 必须属于当前支持集合，并保留实际值 `fr` | [Confirmed] [本地化运行时：语义验证](../development/design/localization-runtime.md#语义验证)；[版本化运行数据加载：错误语义](../development/design/runtime-data-loading.md#错误语义) |
| TC-032 | 资源加载成功与四类失败均形成带上下文的 resolution | P1 | Component Integration | Content Validation | Configuration；Client | 最小 Bevy Asset app、内置默认值与临时 asset root | 参数化加载有效、缺失、malformed、unsupported、invalid 资源 | 有效及前三类失败使用 `assets/data/*.ron` 与 `assets/i18n/*.json` 等价 fixture；invalid 使用 schema 1、`locale=fr` 的 catalog | 有效资源为 `Loaded(typed_data)`；四类失败为 `Fallback { value: built-in default, error }`；invalid 的 typed cause 为 InvalidData；error 含 path、category、typed cause；两类结果均 resolved | [Confirmed] [版本化运行数据加载：协作时序](../development/design/runtime-data-loading.md#协作时序)；[本地化运行时：语义验证](../development/design/localization-runtime.md#语义验证) |
| TC-033 | 平台配置目录中的原子保存成功可恢复，replace 失败保留正式文件与内存值 | P1 | Component Integration | — | Configuration；Client | 平台配置根目录中已有旧正式设置，并可通过实现选择的测试环境构造 replace 失败 | 解析设置路径并执行成功保存、重载；再更新内存值并构造 replace 失败 | 旧 language=`en`；新 language=`zh-CN` | 正式路径位于平台配置根目录；成功重载得到新值且无不完整文件；失败返回可观察错误，内存保持新值，正式文件仍为旧值 | [Confirmed] [本机用户设置：保存设置](../development/design/user-settings.md#保存设置) |
| TC-034 | 注册应用状态机后初始状态为 Boot | P0 | Component Integration | — | Client | 最小 Bevy `App` 注册状态能力 | 完成初始化/首个必要 schedule | 无 | 当前唯一 `AppState=Boot` | [Confirmed] [应用状态机：初始化](../development/design/application-state-machine.md#初始化) |
| TC-035 | 基础状态表的每条合法边均被判为有效 | P1 | Component | — | Client | 可对纯状态表执行合法边判断 | 参数化判断每条基础状态边 | Boot→MainMenu；MainMenu→ModeSelect；ModeSelect→CharacterSelect；CharacterSelect→Match；Match→Paused；Paused→Match；Match→Result；Result→MainMenu | 每组均被判为有效边 | [Confirmed] [应用状态机：有效状态转移](../development/design/application-state-machine.md#有效状态转移) |
| TC-036 | 表外状态边均被判为非法 | P2 | Component | — | Client | 可对纯状态表执行合法边判断 | 参数化判断未列入对应源状态允许目标的边 | 至少每个源状态一个表外目标，含 Boot→Match、Paused→Result、Result→Match | 每组均被判为非法边 | [Confirmed] [应用状态机：有效状态转移](../development/design/application-state-machine.md#有效状态转移) |
| TC-037 | 同状态请求为 no-op 且不触发生命周期 | P1 | Component Integration | — | Client | 最小 Bevy App 为 OnExit/OnEnter 安装计数器 | 对七种状态分别请求自身并运行状态提交周期 | Boot→Boot … Result→Result | 当前状态保持；进入/退出计数不增；无非法边诊断 | [Confirmed] [应用状态机：请求处理](../development/design/application-state-machine.md#请求处理) |
| TC-038 | 合法迁移实际提交后各触发一次退出与进入生命周期 | P1 | Component Integration | — | Client | 最小 Bevy App 为源/目标状态注册生命周期观察 | 提交一条合法边并运行完整状态提交周期 | MainMenu→ModeSelect | 当前唯一状态变为 ModeSelect；OnExit(MainMenu)=1，OnEnter(ModeSelect)=1；对应运行阶段在进入后激活 | [Confirmed] [应用状态机：协作时序](../development/design/application-state-machine.md#协作时序) |
| TC-039 | 同周期重复目标请求合并为一次迁移 | P1 | Component Integration | — | Client | 当前 Match，生命周期计数器已注册 | 同周期提交两份 Result 请求 | 两个 `MatchCompleted` | 仅写入/提交一次 Match→Result；生命周期各触发一次 | [Confirmed] [应用状态机：请求处理](../development/design/application-state-machine.md#请求处理) |
| TC-040 | MatchCompleted 与 PauseRequested 同周期时 Result 获胜 | P1 | Component Integration | — | Client | 当前 Match | 同周期以两种顺序提交两个请求 | Result/MatchCompleted；Paused/PauseRequested | 两种顺序最终均进入 Result，未进入 Paused | [Confirmed] [应用状态机：请求处理](../development/design/application-state-machine.md#请求处理) |
| TC-042 | fixed schedule 配置为 60Hz | P0 | Component Integration | — | Client | 最小客户端 app 注册 simulation 能力 | 读取 fixed schedule 的频率配置 | frequency=`60Hz` | fixed schedule 的配置频率为 60Hz；本用例不通过累计浮点时间推导 tick 数 | [Confirmed] [固定频率规则调度：Fixed System Set](../development/design/fixed-tick-simulation.md#fixed-system-set) |
| TC-043 | 每个 fixed tick 严格执行 Input 后 Rules | P0 | Component Integration | — | Input；Client | Input/Rules 运行标记可观测，app 已处于 `AppState::Match` | 推进多个受控 fixed tick | 3 ticks | 观察序列为 `[Input, Rules] × 3`，每个 Rules 都能读取同 tick Input 产物 | [Confirmed] [固定频率规则调度：Fixed System Set](../development/design/fixed-tick-simulation.md#fixed-system-set) |
| TC-044 | 每个 fixed tick 只形成并消费一次 TickInputs | P0 | Component Integration | — | Input；Client | 输入生产与规则消费次数及本 tick 标记可观测，app 已处于 `AppState::Match` | 推进少量受控 fixed tick | 3 个带可区分输入标记的 tick | 生产数=消费数=3；每个标记恰好消费一次；无跨 tick 复用或漏用 | [Confirmed] [固定频率规则调度：Fixed System Set](../development/design/fixed-tick-simulation.md#fixed-system-set) |
| TC-045 | 相同初始规则状态与量化输入产生相同 fixed 规则结果 | P0 | Component Integration | Determinism | Client | 两个 app 处于相同初始 `AppState::Match` 与规则状态，使用相同量化输入序列 | 以不同数量的普通 Update 穿插执行相同数量的受控 fixed tick | 相同的 6 tick canonical input；不同 Update 交错序列 | 额外 Update 不推进规则；两者消费相同输入序列后得到相同 tick 数与规则状态/checksum | [Confirmed] [固定频率规则调度：Fixed System Set](../development/design/fixed-tick-simulation.md#fixed-system-set) |
| TC-046 | Boot 仅在设置与本地化均 Resolved 时请求 MainMenu | P0 | Component Integration | Smoke | Client | 当前 Boot，可设置 `BootstrapStatus` | 覆盖四种 Pending/Resolved 组合并运行协调 system | PP、PR、RP、RR | 前三组保持 Boot 且无迁移请求；RR 只产生一次 Boot→MainMenu 请求 | [Confirmed] [游戏基础设施运行架构：启动准备](../development/system/game-infrastructure-architecture.md#启动准备) |
| TC-047 | 设置与本地化加载失败经 fallback 后仍解除启动屏障 | P0 | Component Integration | Smoke；Content Validation | Configuration；Client | 最小启动 app，设置与 catalog 均使用失败 fixture | 完成两类加载与启动协调 | malformed settings；missing/unsupported locale catalog | 两项 resolution 均含可用默认值和原始诊断并标记 Resolved；应用进入 MainMenu；查询设置与文本安全可用 | [Confirmed] [游戏基础设施运行架构：主流程](../development/system/game-infrastructure-architecture.md#主流程) |
| TC-048 | 完整基础状态主路径保持单一顶层状态与对应运行阶段 | P0 | System | Smoke | Client；Match Flow | 已装配客户端，启动资源可 resolved，已确认的状态迁移请求均可触发 | 完成启动并依次触发开始、模式确认、角色确认、暂停、继续、比赛结束、返回主菜单 | Boot→MainMenu→ModeSelect→CharacterSelect→Match→Paused→Match→Result→MainMenu | 每步仅有一个当前 AppState；对应状态的运行阶段在进入后激活；本用例不规定 ModeSelect、CharacterSelect、Result 等状态的业务数据结构或具体内容 | [Confirmed] [应用状态机：有效状态转移](../development/design/application-state-machine.md#有效状态转移) |
| TC-049 | Linux 目标构建并链接 production client | P0 | System | Smoke | Client | Linux CI runner 与发布支持的 Rust toolchain | 使用生产 Bevy plugin 配置（`DefaultPlugins` + 项目根插件）构建 workspace/client | Linux x86_64；默认功能集合 | 编译、链接、插件装配成功，产出可执行的 production 二进制；本用例不要求运行到 MainMenu（运行路径由 TC-056 覆盖） | [Confirmed] [游戏基础设施运行架构：启动验收](../development/system/game-infrastructure-architecture.md#启动验收) |
| TC-051 | workspace 依赖图保持 client/net 指向 game_core 且 game_core 与平台运行时隔离 | P1 | System | — | Client | 可读取 Cargo metadata 与各 crate manifest，并可单独选择 game_core package | 检查 Cargo dependency graph 与 game_core manifest，再独立构建和测试 game_core | 必需边：client→game_core、net→game_core；禁止边：game_core→client/net；game_core manifest 禁止 Bevy、网络、窗口、平台目录等平台运行时 crate | 必需边存在，禁止边不存在；game_core manifest 不含所列平台运行时依赖；game_core 可独立构建并通过测试 | [Confirmed] [游戏基础设施运行架构：模块职责](../development/system/game-infrastructure-architecture.md#模块职责) |
| TC-052 | 同一固定方向输入按上下文产生独立领域动作 | P1 | Component | — | Input；Client | 固定 Left 物理方向输入可在 gameplay 与 UI 输入上下文中解释 | 分别在 Match gameplay context 与 Menu UI context 注入同一固定 Left 方向输入 | context=`Match/Menu`；physical input=`Left direction` | Match 中只产生 `GameAction::Left` 并可进入规则输入；Menu 中只产生 `UIAction::Left` 并用于 UI；两个动作类型和输出容器保持独立；Left 不作为用户可配置绑定 | [Confirmed] [UI 交互动作：物理绑定关系](../development/design/ui-action-input.md#物理绑定关系)、[边界](../development/design/ui-action-input.md#边界) |
| TC-053 | fixed tick 仅在 `AppState::Match` 执行，全部非 Match 状态均不产生 Input/Rules | P0 | Component Integration | — | Client | 最小客户端 app 注册状态机与 simulation 能力，Input/Rules 执行次数可观测 | 分别在每个非 Match 状态执行受控 fixed tick，再在 Match 执行受控 fixed tick | `Boot`、`MainMenu`、`ModeSelect`、`CharacterSelect`、`Paused`、`Result`；`Match` | 六个非 Match 状态下 Input/Rules 执行次数均为 0；Match 下两个阶段均按受控 tick 数执行 | [Confirmed] [固定频率规则调度：运行边界](../development/design/fixed-tick-simulation.md#运行边界) |
| TC-054 | `Match → Paused` 后对局 simulation 立即停止 | P0 | Component Integration | — | Client | 当前 `Match`，Input/Rules 执行计数器已运行若干 tick | 提交 `Paused` 请求并运行状态提交，随后提供若干 fixed 执行机会 | 转移前计数=N；转移后 3 个 fixed 执行机会 | 状态转移当拍起不再产生新的 Input/Rules 执行；转移后计数保持为 N | [Confirmed] [固定频率规则调度：运行边界](../development/design/fixed-tick-simulation.md#运行边界) |
| TC-055 | `Paused → Match` 恢复后规则状态从暂停前继续推进 | P0 | Component Integration | — | Client | Match 中已推进至可观察的非初始规则状态 S，并记录已消费 tick 数 | 转移至 `Paused`、停留若干受控 fixed tick、转移回 `Match` 并再推进若干 tick | 暂停前状态=S；暂停期间 3 ticks；恢复后 3 ticks | 恢复起点为暂停前状态 S，暂停期间没有重置或跳变；恢复后 tick 计数从暂停前继续累加 | [Confirmed] [固定频率规则调度：运行边界](../development/design/fixed-tick-simulation.md#运行边界) |
| TC-056 | Linux 自动化 startup smoke 复用项目根插件跑通 Boot→MainMenu | P0 | System | Smoke | Client | Linux CI runner；最小 Bevy runtime + 与生产客户端相同的项目根插件，不含真实窗口依赖 | 运行可自动退出的 startup smoke | Linux x86_64 | 项目根插件装配成功；`AppState` 初始化为 `Boot`；`UserSettings` 与 `Localization` 完成 bootstrap resolution；应用到达 `MainMenu`；进程正常退出；全程不要求真实窗口交互 | [Confirmed] [游戏基础设施运行架构：启动验收](../development/system/game-infrastructure-architecture.md#启动验收) |
| TC-058 | `Match` 语境下固定 Start 按键由 `client::input` 直接提出 `PauseRequested` | P0 | Component Integration | — | Input；Client | 当前 `AppState::Match`；`client::input` 使用固定手柄 Start 按键；状态迁移结果可观测 | 采样到手柄 Start 按键 press edge 并运行状态提交周期 | 手柄 Start press edge，`AppState::Match` | 当前状态提交为 `Paused`；该触发不产生 `UIAction` 或 `GameAction`（生命周期效果由 TC-054 覆盖）；迁移请求和内部协作类型由实现决定 | [Confirmed] [应用状态机：协作](../development/design/application-state-machine.md#协作)；[UI 交互动作：边界](../development/design/ui-action-input.md#边界) |
| TC-059 | 固定绑定动作不出现在 `PlayerInputBindings` 且不参与绑定冲突检测 | P1 | Component | — | Configuration；Input | 默认或已保存的玩家输入设置可查询和序列化 | 检查持久化结果与可配置绑定冲突行为 | `UIAction` 全部六项；`GameAction::Left`、`GameAction::Right`；四项可配置 `GameAction` | 持久化设置只包含四项可配置 `GameAction`；绑定冲突行为只处理这四项；固定绑定动作保持在可配置与冲突检测范围外 | [Confirmed] [UI 交互动作：物理绑定关系](../development/design/ui-action-input.md#物理绑定关系)；[本机用户设置：数据模型](../development/design/user-settings.md#数据模型)、[输入绑定冲突](../development/design/user-settings.md#输入绑定冲突) |
| TC-060 | `PlayerActions` 位编码为锁定的稳定格式 | P0 | Component | Determinism | Input | 可在无 Bevy 环境构造 `PlayerActions` | 分别构造六个单动作集合并读取底层 `u8` | 六个 `GameAction` 各自单独置位 | `Left`/`Right`/`SoftDrop`/`HardDrop`/`RotateClockwise`/`RotateCounterClockwise` 依次对应 bit 0–5，底层值为 `1/2/4/8/16/32`；任意动作组合的 bit 6–7 恒为 0 | [Confirmed] [统一游戏动作与 Tick 输入：位编码](../development/design/game-action-input.md#位编码) |
| TC-061 | 槽位访问器区分「参与者不存在」与「无动作」 | P1 | Component | — | Input | 构造 `len` 为 2 的 `TickInputs`，其中 slot 1 无动作 | 分别查询 slot 1 与 slot 2 | slot 1 为 `EMPTY`；slot 2 超出 `len` | slot 1 返回存在且为空动作；slot 2 返回「无该参与者」；两者结果可区分 | [Confirmed] [统一游戏动作与 Tick 输入：`TickInputs`](../development/design/game-action-input.md#tickinputs) |
| TC-062 | 默认绑定非空且覆盖六个规则动作 | P1 | Component | Content Validation | Configuration；Input | 使用内置默认设置，无设置文件 | 按默认绑定分别注入键盘与手柄物理输入并采样 | 默认 `UserSettings`；P1/P2 键盘与手柄默认键位 | 键盘与手柄均可产生全部六个 `GameAction`；四项可配置动作的默认绑定均非空 | [Confirmed] [本机用户设置：默认输入绑定](../development/design/user-settings.md#默认输入绑定)；[UI 交互动作：固定绑定表](../development/design/ui-action-input.md#固定绑定表) |
| TC-063 | 左摇杆方向按阈值 `0.5` 判定 | P2 | Component | — | Input | 采样器可接收摇杆分量 | 提交阈值上下与边界处的分量后采样 | 分量 `0.4`、`0.5`、`0.6` | `0.4` 与 `0.5` 不产生方向动作，`0.6` 产生；摇杆方向与十字键、键盘方向合并为同一逻辑动作 | [Confirmed] [本地输入采样：摇杆方向判定](../development/design/local-input-sampling.md#摇杆方向判定) |
| TC-064 | 持续保持方向输入不产生连发 | P2 | Component | — | Input | 采样器持有已绑定的方向输入 | 保持方向按下并连续采样多个 fixed tick | 保持 `Left` 按下 5 个 tick | 每个 tick 各产生一次 `Left`，采样器不额外插入重复触发 | [Confirmed] [本地输入采样：摇杆方向判定](../development/design/local-input-sampling.md#摇杆方向判定) |
| TC-065 | 键盘 `Escape` 只在 `Match` 下提出 `Pause` | P1 | Component Integration | — | Input；Client | 最小 Bevy App 已注册项目根插件 | 分别在 `MainMenu` 与 `Match` 状态下按下 `Escape` | `AppState::MainMenu`；`AppState::Match` | `MainMenu` 下不提出 `PauseRequested`，不产生 `UIAction`，且该次按下不滞留到进入 `Match` 后生效；`Match` 下提出 `PauseRequested` 并进入 `Paused` | [Confirmed] [应用状态机：协作](../development/design/application-state-machine.md#协作) |
| TC-066 | 启动资源超时后仍进入 `Resolved` 并释放屏障 | P1 | Component Integration | Smoke | Client；Configuration | 最小 Bevy App，启动资源加载不返回结果 | 推进应用直到超过启动超时 | 加载超时 `5s` | 两项启动任务均进入 `Resolved` 并使用内置默认值；保留超时诊断；`Boot → MainMenu` 完成，应用不停留在 `Boot` | [Confirmed] [游戏基础设施运行架构：启动准备](../development/system/game-infrastructure-architecture.md#启动准备) |
| TC-067 | 生产主调度下同帧输入进入当帧 fixed tick | P0 | Component Integration | — | Input；Client | 最小客户端 app 使用生产主调度（不手动驱动 fixed schedule），虚拟时间可受控推进，当前 `AppState::Match` | 在一帧内注入已绑定输入的按下并推进一帧，读取该帧 fixed tick 的 `TickInputs`；随后释放并同样推进一帧 | 每帧推进 `1/60s`；持续动作 `Left` | 按下所在帧的 fixed tick 已包含该动作，释放所在帧的 fixed tick 不再包含；两者均不延后到下一帧；本用例不规定采样系统的具体调度点 | [Inferred] [固定频率规则调度：协作时序](../development/design/fixed-tick-simulation.md#协作时序)、[Fixed System Set](../development/design/fixed-tick-simulation.md#fixed-system-set) |
| TC-068 | 单帧补跑多个 fixed tick 时共享该帧采样结果 | P1 | Component Integration | — | Input；Client | 同 TC-067，且可让单帧推进多个 fixed tick | 在一帧内保持一个持续动作并产生一次尚未提交的一次性动作，推进使该帧运行三个 fixed tick | 单帧推进 `3/60s`；保持 `Left` 并产生一次 `HardDrop` press edge | 三个 tick 均含 `Left`；`HardDrop` 只出现在第一个 tick，后两个 tick 不重复产生 | [Inferred] [固定频率规则调度：协作时序](../development/design/fixed-tick-simulation.md#协作时序) |
| TC-069 | 设备适配层保留同帧内完成的 press edge | P0 | Component Integration | — | Input；Client | 最小客户端 app 使用真实 `ButtonInput` 与生产设备捕获路径 | 在同一帧内对一次性动作的默认绑定执行按下并松开，随后推进 fixed tick | 参数化 `HardDrop`、`RotateClockwise`、`RotateCounterClockwise`；每组在同一帧内完成 press 与 release | 每组在随后的 fixed tick 产生一次对应动作；采样依据 press edge 而非采样时刻的按住状态；持续按住不因此重复产生 | [Confirmed] [本地输入采样：一次性动作采样](../development/design/local-input-sampling.md#一次性动作采样)；[Inferred] [捕获物理输入](../development/design/local-input-sampling.md#捕获物理输入) |
| TC-070 | 手柄断开清除该设备在采样状态中的残留 | P1 | Component Integration | — | Input；Client | 已接入手柄并绑定到某本地玩家槽位，采样结果可观测 | 参数化三种断开情形后继续推进 fixed tick | 按住方向时断开；无输入时断开；断开后重新接入且不按任何键 | 断开后的 fixed tick 不再产生该方向动作；无输入断开不改变其它玩家的采样结果；重连不带入断开前的按下状态 | [Inferred] [本地输入采样：设备与玩家绑定](../development/design/local-input-sampling.md#设备与玩家绑定) |
| TC-071 | 手柄接入顺序变化不改变已绑定玩家的槽位 | P2 | Component Integration | — | Input；Client | 两个手柄可分别接入与断开，各自可注入可区分输入 | 依次接入两个手柄，断开先接入的一个，再接入第三个，并在各阶段采样 | pad A→最小空闲槽位、pad B→次一槽位；A 断开后接入 pad C | B 在 A 断开后保持原槽位；C 取得空出的槽位；采样结果不随设备遍历顺序改变 | [Inferred] [本地输入采样：设备与玩家绑定](../development/design/local-input-sampling.md#设备与玩家绑定) |
| TC-072 | `PlayerActions` 的公开解码入口拒绝保留位 | P1 | Component | Determinism | Input | 可在无 Bevy 环境经全部公开解码入口构造 `PlayerActions` | 分别向 `from_bits` 与 Serde 解码提交合法值与保留位置位的值 | 合法：`0`、`63`；非法：`64`、`128`、`192` | 合法值解码成功且等于对应动作集合；三个非法值在每个公开入口均解码失败；不存在绕过保留位不变量的公开路径 | [Confirmed] [统一游戏动作与 Tick 输入：位编码](../development/design/game-action-input.md#位编码) |
| TC-073 | 项目根插件装配后消费者取得 resolved typed data | P1 | Component Integration | Smoke | Configuration；Client | 最小 Bevy App 注册项目根插件，asset root 指向仓库真实 `assets/` | 推进应用直到数据加载结束，从消费者侧读取 typed 结果 | 仓库内现有的 `assets/data/*.ron` fixture | 消费者可读到 resolved typed data，成功为 `Loaded`、失败为带诊断的 `Fallback`；请求、轮询与注册均由项目根插件完成，测试不自建加载生命周期 | [Confirmed] [版本化运行数据加载：协作](../development/design/runtime-data-loading.md#协作) |
| TC-074 | 真实 production 二进制有界启动到 `MainMenu` 后退出 | P0 | System | Smoke | Client | Linux 环境具备虚拟显示与软件 Vulkan 后端；以 `ci_testing` feature 构建的真实 `psi` 二进制 | 以指定退出帧的配置运行二进制并等待进程结束 | 退出帧取到达 `MainMenu` 之后的确定帧 | 窗口创建成功；应用到达 `MainMenu`；进程在指定帧自动退出且退出码为成功；本用例覆盖 `DefaultPlugins` 运行时初始化与窗口后端，不要求交互 | [Inferred] [游戏基础设施运行架构：启动验收](../development/system/game-infrastructure-architecture.md#启动验收) |

## 风险查漏

| 风险领域 | 覆盖结论 |
| --- | --- |
| 状态与流程 | 主路径、全部基础边、非法边、同状态、重复、已定义优先级和启动屏障均有直接用例；`Match ⇄ Paused` 对局 simulation 的启停与状态延续由 TC-053～TC-055 覆盖；`PauseRequested` 直接触发路径由 TC-058 覆盖。无已定义优先级的不同目标冲突等待新增真实可构造状态边时补充测试。 |
| Configuration | schema 支持、解析、版本、语义错误、资源路径、fallback、设置默认值与原子保存均有直接用例；本地化 catalog locale 语义约束（`InvalidData`）已覆盖（TC-031）；固定绑定与四项可配置 `GameAction` 绑定的范围划分已覆盖（TC-059）；生产装配下消费者取得 resolved typed data 由 TC-073 覆盖，与 TC-032 的解析层职责分开。 |
| Client 与 Input | 六种 GameAction、六种 UIAction、输入上下文隔离、slot 容量、玩家隔离、多来源、fixed 边界与 edge 保留均有直接用例；TC-023/024/052/059 固定了现有固定绑定与可配置绑定的范围划分；TC-058 覆盖 `PauseRequested` 的固定 Start 直接触发路径；TC-054～TC-055 覆盖其 simulation 生命周期效果；TC-065 覆盖键盘 `Escape` 只在 `Match` 下提出 `Pause`、在菜单下无输出；TC-062～TC-064 覆盖默认绑定非空、摇杆阈值与无连发。采样时机与设备生命周期由 TC-067～TC-071 覆盖：TC-067～TC-068 在生产主调度下验证同帧可见性与单帧多 tick 语义，TC-069 覆盖设备适配层的 press edge，TC-070～TC-071 覆盖断开清理与槽位绑定稳定性。TC-026～TC-027A 仍限于纯采样器层，两层不互相替代。 |
| Determinism | TC-045 直接验证相同初始规则状态与相同量化输入序列产生相同 fixed 规则结果；TC-060 钉死 `PlayerActions` 的稳定位编码，TC-072 补上解码方向，确保公开入口无法构造违反保留位不变量的值，两者共同保护校验和与后续网络编码的字节稳定性。输入结构、归一化、SystemSet 顺序和 Pause 生命周期按各自功能覆盖，均不标记 Determinism。 |
| Rules 与数值 | 本轮只覆盖规则调用边界；玩法公式由后续规则设计覆盖。 |
| AI | 统一 `PlayerActions`/`TickInputs` 类型边界已覆盖；AI 动作合法性由 AI 设计覆盖。 |
| Network | crate 依赖方向与统一输入容量已覆盖；握手、同步、回滚和断线由 R2 设计覆盖。 |
| CI 环境匹配 | `test.yml`（目标分支为 `main` 的 pull request 触发）的 `test-linux` job 执行 `cargo fmt`、`cargo clippy`、`cargo test --workspace` 与 linux-gnu 构建，自动化 smoke（TC-056）有对应 CI 执行路径；`release.yml`（`workflow_dispatch` 手动触发）以 `--release` 构建 production client（TC-049）。TC-074 需要虚拟显示与软件 Vulkan，规划在 `release.yml` 执行，对应 job 尚未实现，是当前测试设计与 workflow 之间唯一已知的未接通项。开发分支的推送不触发 CI，由本地执行覆盖。 |

## 实施顺序

1. 先实现 TC-013～TC-022 的纯 game_core 输入测试，固定最底层数据与归一化语义。
2. 实现 TC-001～TC-012、TC-028～TC-033、TC-059 的解析、fallback、持久化与固定/可配置绑定范围测试。
3. 实现 TC-023～TC-027A、TC-035～TC-036、TC-052 的纯行为测试，以及 TC-027B、TC-034、TC-037～TC-045 的最小 Bevy App 组件集成测试。
4. 实现 TC-053～TC-055、TC-058 的 `Match`/`Paused` 对局 simulation 生命周期与 `Pause` 直接触发路径测试，复用 TC-042～TC-045 已建立的 fixed tick 观测手段。
5. 最后接入 TC-046～TC-049、TC-051、TC-056 的启动、主路径、CI 与依赖边界测试；TC-049 在 `release.yml`（手动触发）执行，TC-056 在 `test.yml`（PR→`main` 触发）执行。
6. 随实现接通设备输入、默认绑定与异步资源加载时补充 TC-060～TC-066：TC-060～TC-061 属纯 game_core 层，随步骤 1 的输入语义一并固定；TC-062～TC-064 随设备采样接入；TC-065 复用步骤 4 的 Pause 观测手段；TC-066 随启动屏障异步化在步骤 5 接入。
7. 修正采样时序与设备生命周期时接入 TC-067～TC-071。TC-067 需要先建立不手动驱动 fixed schedule 的观测手段，TC-068～TC-069 复用该手段，三者随采样调度位置与 press edge 捕获的同一次改动落地；TC-070～TC-071 随设备绑定与断开清理落地。TC-072 属纯 game_core 层，可独立实现。
8. 最后接入 TC-073 与 TC-074：TC-073 随生产 `DataPlugin` 接通加载生命周期落地；TC-074 需要 `release.yml` 具备虚拟显示与软件 Vulkan 环境，并提供指定退出帧的运行配置。

