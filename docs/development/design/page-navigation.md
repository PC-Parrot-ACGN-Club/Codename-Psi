# 页面导航与焦点

**相关模块：** `client::page`、`client::input`
**关联文档：** [应用状态机](application-state-machine.md)、[UI 交互动作](ui-action-input.md)、[本机用户设置](user-settings.md)、[表现运行时](presentation-runtime.md)、[表现与 UI 设计 §5](../../presentation.md)、[PRD §5.1](../../PRD.md)

## 目标

为 R1 全部页面定义焦点模型、导航语义、页面实体生命周期，以及每个页面能够提出的状态迁移。

## 数据模型

| 名称 | 含义 | 约束 |
| --- | --- | --- |
| focus ring | 一个页面内可聚焦项的有序环 | 每个环任一时刻恰有一个焦点项 |
| focusable item | 可聚焦项 | 携带本地化 key 与启用状态 |
| disabled item | 可聚焦但不可确认的项 | 显示不可用原因，`Confirm` 不产生迁移 |
| page entity | 页面自身的 UI 实体 | 绑定所属 `AppState`，退出该状态时销毁 |

## 页面与迁移

| 页面（`AppState`） | 可聚焦项 | 可提出的 cause |
| --- | --- | --- |
| `MainMenu` | 开始游戏、设置、退出 | `StartGame`、`SettingsOpened` |
| `ModeSelect` | 单人、本地双人、AI 对战 AI、局域网对战、返回 | `ModeConfirmed`、`BackRequested` |
| `CharacterSelect` | 每个槽位的角色项、确认、返回 | `CharacterConfirmed`、`BackRequested` |
| `Settings` | 各设置项、返回 | `SettingsClosed` |
| `Match` | 无 | `PauseRequested` |
| `Paused` | 继续、重开、设置、返回主菜单 | `ResumeRequested`、`RestartRequested`、`SettingsOpened`、`MatchAbandoned` |
| `Result` | 再来一局、返回主菜单 | `RematchRequested`、`ReturnToMainMenu` |

`MainMenu` 的「退出」不是状态迁移，它直接请求应用退出。`Match` 没有焦点环：暂停输入由 `client::input` 直接识别，不经过焦点。

局域网对战项在 R1 保持可聚焦且禁用，附带 R2 说明文本，`Confirm` 不产生任何迁移，也不创建对局。

## 行为

### 焦点移动

- 输入：一次 `UIAction::Up` / `Down` / `Left` / `Right`。
- 处理：在当前焦点环内沿该方向移动一项，越过端点时回到另一端；禁用项同样可以获得焦点。
- 输出：新的焦点项。
- 错误语义：环内只有一项时焦点不变，不产生诊断。

焦点环的次序即页面的视觉次序。页面分多列排布时，环先走完一列的全部项再进入下一列，
列内自上而下。

### 确认

- 输入：一次 `UIAction::Confirm`。
- 处理：执行当前焦点项的页面动作——提出迁移、切换设置取值，或进入[绑定捕获](user-settings.md#绑定捕获)。
- 输出：对应的页面动作结果。
- 错误语义：焦点项禁用时不执行任何动作，页面展示该项的不可用原因。

### 返回

- 输入：一次 `UIAction::Back`。
- 处理：提出该页面的返回 cause；`Settings` 的返回目标取自 [`SettingsOrigin`](application-state-machine.md#设置返回上下文)。
- 输出：一次返回迁移。
- 错误语义：`MainMenu` 与 `Result` 没有返回目标，该次输入被丢弃；`Match` 的返回输入不产生 `UIAction`。

### 设备可用性

- 输入：当前连接的手柄数量。
- 处理：`Settings` 的手柄重绑定项在没有手柄连接时禁用，附带说明文本；手柄接入或断开时随即更新，不需要离开并重新进入页面。
- 输出：更新后的可聚焦项启用状态。
- 错误语义：禁用状态下确认该项不进入[绑定捕获](user-settings.md#绑定捕获)。

设置页列出的重绑定项不随设备数量增减：页面是「能配置什么」的固定清单，而非「插着什么」的清单。可用性只落在启用状态上。一个标志覆盖两名玩家——捕获接受任一已连接手柄的输入，按槽位区分可用性并不成立。

### 页面输入归属

- 输入：来自某名本地玩家的 `UIAction`。
- 处理：`CharacterSelect` 为每个本地控制槽位维护独立焦点环，只消费对应玩家的输入；其余页面只有一个焦点环，接受任一本地玩家的输入。
- 输出：对应焦点环的移动或确认。
- 错误语义：单人模式下 P2 的物理输入不驱动任何焦点环。

单人与 AI 对战 AI 模式的 `CharacterSelect` 由 P1 先后为两个槽位各确认一个角色，两个槽位允许相同角色；本地双人由两名玩家各自确认自己的槽位。两个槽位都确认后「确认」项才可用。

槽位归属由模式决定：只有本地双人存在第二名本地玩家，其余模式下 P2 的物理输入不驱动任何焦点环。AI 驱动的槽位是模式的函数——单人为槽位 1，AI 对战 AI 为两个槽位，本地双人为空。

### 页面实体生命周期

- 输入：一次已提交的状态迁移。
- 处理：销毁退出状态的全部 page entity，为进入状态创建 page entity，并把焦点置于其首项。
- 输出：只属于当前状态的页面实体集合。
- 错误语义：迁移被拒绝时不创建也不销毁任何 page entity。

page entity 以 `DespawnOnExit<AppState>` 绑定所属状态，由状态退出统一销毁。

对局表现实体不是 page entity：它们不携带 `DespawnOnExit<AppState>`，而是绑定[对局实例生命周期](fixed-tick-simulation.md#对局实例生命周期)。因此 `Match → Paused → Match` 和 `Paused → Settings → Paused` 期间对局画面保持可见，暂停与设置页覆盖在其上。

## 焦点承载

只有一个焦点环的页面用 `InputFocus` 承载当前焦点，方向导航用 `AutoDirectionalNavigation` 按屏幕位置推导邻居，需要固定跳转顺序的位置在 `DirectionalNavigationMap` 写入覆盖边——覆盖边优先于自动推导。

`InputFocus` 是全局唯一资源，无法表达两个并行焦点环。`CharacterSelect` 的槽位焦点由页面自己的组件承载，不写入 `InputFocus`。

禁用项用 `InteractionDisabled` 标记：它阻止交互但不阻止该项获得焦点，与本文的 disabled item 语义一致。

## 协作

| 数据或消息 | 生产方 | 消费方 | 语义与约束 |
| --- | --- | --- | --- |
| `UIAction` | [UI 交互动作](ui-action-input.md) | 本主题 | 携带来源本地玩家 |
| `AppTransitionRequest` | 本主题 | [应用状态机](application-state-machine.md) | 页面动作的唯一迁移出口 |
| `SettingsOrigin` | 本主题 | 应用状态机 | 提出 `SettingsOpened` 前写入 |
| 焦点项文本 | [本地化运行时](localization-runtime.md) | 本主题 | 按稳定 key 查询；项名、不可用原因与设置项的值都是 key，只有数值（音量百分比、按键名）直接显示 |
| 冻结的 `LockedMatchSpec` | `CharacterSelect`、`Result` 页面 | [规则配置与开局规格冻结](rule-configuration.md) | 在提出 `CharacterConfirmed` 或 `RematchRequested` 前完成 |

## 边界

- 本文不定义有效状态边与仲裁（见[应用状态机](application-state-machine.md)）。
- 本文不定义 `UIAction` 的物理绑定，也不定义各页面常驻的按键提示（见[UI 交互动作：物理绑定关系](ui-action-input.md#物理绑定关系)、[按键提示](ui-action-input.md#按键提示)）。
- 本文不定义设置项的取值域、冲突判定与持久化（见[本机用户设置](user-settings.md)）。
- 本文不定义页面的视觉布局、配色与动效（见[表现与 UI 设计](../../presentation.md)、[表现运行时](presentation-runtime.md)）。
- 本文不定义对局 HUD 的内容与更新（见[表现运行时](presentation-runtime.md)）。

## Test Basis

- [Issue #13](https://github.com/PC-Parrot-ACGN-Club/Codename-Psi/issues/13)：要求完成主菜单、模式选择、角色选择、设置、暂停与赛果页面，并能用键盘和手柄走完全部流程。
- [PRD §5.1](../../PRD.md)：定义各页面的必需内容。
- [PRD §4.1](../../PRD.md)：局域网对战在 R2 解锁。
- [表现与 UI 设计 §5](../../presentation.md)：定义各页面的关键内容与主操作。
- [表现与 UI 设计 §7](../../presentation.md)：焦点顺序、确认/返回在所有菜单中的键盘与手柄操作保持一致。
