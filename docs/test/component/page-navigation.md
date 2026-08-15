# 测试用例设计：页面导航与焦点

**关联设计：** [页面导航与焦点](../../development/design/page-navigation.md)、[UI 交互动作](../../development/design/ui-action-input.md)、[应用状态机](../../development/design/application-state-machine.md)

**关联实现：** `../../../crates/client`

## 需求理解摘要

**功能：** 验证焦点环移动、确认、返回、页面输入归属与各页面可提出的状态迁移。
**测试性质：** 新功能
**本轮范围：** 不装配 Bevy 页面实体的焦点与页面动作纯判定。
**Test Basis：**

- [Confirmed] [页面导航与焦点](../../development/design/page-navigation.md)：焦点环语义、页面与迁移对照、禁用项与输入归属。
- [Confirmed] [UI 交互动作](../../development/design/ui-action-input.md)：`UIAction` 六个成员的语义。
- [Confirmed] [应用状态机](../../development/design/application-state-machine.md)：各 cause 的合法源状态。

**设计基线：** 焦点环与页面动作是可在内存中构造和判定的纯结构，迁移请求以「是否产生、产生哪个 cause」为观察点，不提交真实状态迁移。
**公共执行约束：** 见[游戏基础设施测试索引](../game-infrastructure.md#公共执行约束)。

## 范围边界

本文不定义状态边合法性（见 [应用状态表](application-state-table.md)）、迁移提交与仲裁（见 [应用生命周期](../integration-system/application-lifecycle.md)）、页面实体的创建与销毁（见 [表现运行时](../integration-system/presentation-runtime.md)），也不定义设置项取值与绑定捕获（见 [本机用户设置](user-settings.md)）。

## 测试点清单

### Component — Client

- 焦点在环内按方向移动并在端点回绕，单项环保持不变（TC-001～TC-002）。
- 禁用项可获得焦点但确认不产生动作（TC-003）。
- 各页面焦点项确认后产生对照表规定的 cause（TC-004）。
- 返回在无返回目标的页面被丢弃，设置页返回目标取自来源（TC-005）。
- 局域网入口禁用且不进入对局（TC-006）。
- 分多列排布的页面，焦点环次序与视觉次序一致（TC-010）。
- 两个下角的按键提示分属两名玩家，给出各自当前的确认与返回键（TC-011）。
- 设置项的值与页面各行文字都随当前语言给出，数值除外（TC-012）。

### Component — Input

- 角色选择的两个焦点环按玩家隔离，其余页面共用一个环（TC-007～TC-008）。
- 两个槽位都确认后确认项才可用（TC-009）。

## 设计方法与覆盖模型

| 方法 | 输入模型 / 风险 | 关联用例 |
| --- | --- | --- |
| 边界值 | 焦点位于环首、环尾与单项环 | TC-001～TC-002 |
| 判定表 | 页面 × 焦点项 → cause；来源状态 × 返回目标 | TC-004～TC-005 |
| 状态转移 | 沿环连续移动一整圈，比对次序与列布局 | TC-010 |
| 场景法 | 改绑旋转前后的按键提示文本 | TC-011 |
| TC-012 | 设置项的值与页面文字随当前语言给出 | P1 | Component | — | Client | 两份随包语言目录均可用，页面模型与设置可构造 | 在两种语言下各求一次设置项的值、各行文字与语言自称 | 窗口模式、动画强度、震动、色觉辅助、主音量 0.5、语言 `zh-CN`、locale `xx-YY` | 从固定集合中取值的设置项在两种语言下文本不同且都非空；音量在两种语言下都是 `50%`；语言项显示语言自称，同一 locale 在两份目录中自称相同，未被命名的 locale 回退到 locale 代码；同一页面模型在两种语言下逐行文本不同，禁用项的不可用原因也随之本地化而不显示 key | [Confirmed] [本地化运行时：切换语言](../../development/design/localization-runtime.md#切换语言)；[页面导航与焦点：协作](../../development/design/page-navigation.md#协作) |
| 对比测试 | 同一页面模型与同一份设置在两种语言下的文本 | TC-012 |
| 等价类划分 | 启用项与禁用项 | TC-003、TC-006 |
| 场景法 | 双玩家分别操作角色选择的两个槽位 | TC-007～TC-009 |

## 测试用例列表

| ID | 标题 | Priority | Test Level | Concern | Domain | 前置条件 | 操作/刺激 | 测试数据 | 预期结果 | Test Basis / 证据状态 |
| --- | --- | --- | --- | --- | --- | --- | --- | --- | --- | --- |
| TC-001 | 焦点在环内按方向移动并在端点回绕 | P1 | Component | — | Client | 一个含三个启用项的焦点环，焦点在首项 | 依次施加 `Up`、`Down`、`Down`、`Down` | 三项环；起始焦点为索引 0 | `Up` 后焦点为索引 2；随后三次 `Down` 依次得到索引 0、1、2；任一时刻恰有一个焦点项 | [Confirmed] [页面导航与焦点：焦点移动](../../development/design/page-navigation.md#焦点移动) |
| TC-002 | 单项环上的焦点移动不改变焦点且不产生诊断 | P2 | Component | — | Client | 只含一项的焦点环 | 施加四个方向各一次 | 单项环 | 焦点始终为该项；不产生诊断 | [Confirmed] [页面导航与焦点：焦点移动](../../development/design/page-navigation.md#焦点移动) |
| TC-003 | 禁用项可获得焦点但确认不产生任何动作 | P1 | Component | — | Client | 焦点环含一个启用项与一个禁用项 | 将焦点移到禁用项并施加 `Confirm` | 禁用项携带不可用原因 | 焦点成功落在禁用项；`Confirm` 不产生迁移请求、不切换设置取值、不进入绑定捕获；该项的不可用原因可读 | [Confirmed] [页面导航与焦点：确认](../../development/design/page-navigation.md#确认) |
| TC-004 | 各页面焦点项确认后产生对照表规定的 cause | P0 | Component | — | Client | 可为每个页面构造其焦点环 | 参数化对每个页面的每个启用项施加 `Confirm` | MainMenu 开始游戏/设置；ModeSelect 单人/本地双人/返回；CharacterSelect 确认/返回；Settings 返回；Paused 继续/重开/设置/返回主菜单；Result 再来一局/返回主菜单 | 每项产生[页面与迁移](../../development/design/page-navigation.md#页面与迁移)对照表规定的唯一 cause；MainMenu 的「退出」产生应用退出请求而非状态迁移；不产生对照表以外的 cause | [Confirmed] [页面导航与焦点：页面与迁移](../../development/design/page-navigation.md#页面与迁移) |
| TC-005 | 返回在无返回目标的页面被丢弃，设置页返回目标取自来源 | P1 | Component | — | Client | 可设置 `SettingsOrigin` | 参数化对各页面施加 `Back` | MainMenu；Result；Settings 且 `SettingsOrigin=MainMenu`；Settings 且 `SettingsOrigin=Paused` | MainMenu 与 Result 不产生任何迁移请求；两组 Settings 分别产生目标为 `MainMenu` 与 `Paused` 的 `SettingsClosed` | [Confirmed] [页面导航与焦点：返回](../../development/design/page-navigation.md#返回)；[应用状态机：设置返回上下文](../../development/design/application-state-machine.md#设置返回上下文) |
| TC-006 | 局域网入口可聚焦、禁用且不创建对局 | P1 | Component | — | Client；Match Flow | ModeSelect 焦点环已构造 | 将焦点移到局域网项并施加 `Confirm` | 局域网项 | 焦点可落在该项且其 R2 说明文本可读；不产生 `ModeConfirmed` 或任何迁移请求；不产生开局请求 | [Confirmed] [页面导航与焦点：页面与迁移](../../development/design/page-navigation.md#页面与迁移) |
| TC-007 | 角色选择的两个焦点环按玩家隔离 | P0 | Component | — | Input；Client | 本地双人的 CharacterSelect，两个槽位各有独立焦点环 | 只对 P1 施加 `Down`，再只对 P2 施加 `Down` | P1 与 P2 各三项角色环，起始焦点均为索引 0 | 第一次操作后 P1 焦点为索引 1、P2 仍为索引 0；第二次操作后 P2 焦点为索引 1、P1 保持索引 1 | [Confirmed] [页面导航与焦点：页面输入归属](../../development/design/page-navigation.md#页面输入归属) |
| TC-008 | 非角色选择页面共用一个焦点环，单人模式 P2 输入不驱动焦点 | P1 | Component | — | Input；Client | 分别构造 MainMenu 环与单人模式的 CharacterSelect | 分别以 P1 与 P2 来源施加 `Down` | MainMenu 三项环；单人模式 CharacterSelect | MainMenu 上两个来源都移动同一个焦点环；单人模式下 P2 来源的输入不移动任何焦点环 | [Confirmed] [页面导航与焦点：页面输入归属](../../development/design/page-navigation.md#页面输入归属) |
| TC-009 | 两个槽位都确认后确认项才可用 | P1 | Component | — | Client；Match Flow | 本地双人的 CharacterSelect，两个槽位均未确认 | 依次确认 P1 槽位、检查确认项、确认 P2 槽位、再检查确认项 | 两槽位各选一个角色，允许相同角色 | 仅 P1 确认时确认项为禁用且 `Confirm` 不产生 `CharacterConfirmed`；两槽位均确认后确认项启用，`Confirm` 产生一次 `CharacterConfirmed`；两槽位选择相同角色时同样可确认 | [Confirmed] [页面导航与焦点：页面输入归属](../../development/design/page-navigation.md#页面输入归属) |
| TC-010 | 设置页焦点环次序与两列排布一致 | P1 | Component | — | Client | 已构造 Settings 焦点环 | 从环首连续施加 `Down` 走完一整圈，记录经过的项 | 通用设置项、返回项、每玩家每动作每设备的重绑定项 | 先经过全部通用设置项，随后是返回项，最后才是全部重绑定项；返回项不出现在重绑定项之后 | [Confirmed] [页面导航与焦点：焦点移动](../../development/design/page-navigation.md#焦点移动) |
| TC-011 | 下角按键提示按玩家给出当前确认与返回键 | P1 | Component | — | Client；Input | 使用默认设置与已加载的 `en` 词条，两名玩家均未连接手柄 | 取两名玩家的提示文本；再把 P1 的 `RotateCounterClockwise` 改绑到另一物理键后重取 | 默认 `UserSettings`；改绑后的新键；设置页该重绑定行的标签 | P1 提示含其默认的 `J` 与 `K` 并标明属于 P1，P2 提示标明属于 P2 且与 P1 不同；改绑后提示给出新键、不再给出原键；该重绑定行的标签同时给出规则动作名与菜单动作名 | [Confirmed] [UI 交互动作：按键提示](../../development/design/ui-action-input.md#按键提示)、[绑定来源表](../../development/design/ui-action-input.md#绑定来源表) |

## 风险查漏

焦点移动边界、禁用项、页面动作映射、返回上下文、双玩家隔离与确认前置条件均有直接用例；页面实体生命周期与真实设备输入由集成测试稿覆盖。
