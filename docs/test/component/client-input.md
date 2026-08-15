# 测试用例设计：客户端输入

**关联设计：** [本地输入采样](../../development/design/local-input-sampling.md)、[UI 交互动作](../../development/design/ui-action-input.md)、[本机用户设置](../../development/design/user-settings.md)

**关联实现：** `../../../crates/client`

## 需求理解摘要

**功能：** 验证物理输入采样、玩家隔离、输入上下文、press edge 与摇杆阈值。
**测试性质：** 新功能（含既有设计的补充变更）
**本轮范围：** 不装配生产主调度的纯采样器和输入上下文行为。
**Test Basis：**

- [Confirmed] [本地输入采样](../../development/design/local-input-sampling.md)：物理输入捕获、玩家绑定、持续动作、一次性动作和摇杆判定。
- [Confirmed] [UI 交互动作](../../development/design/ui-action-input.md)：输入上下文、固定绑定与领域隔离。
- [Confirmed] [本机用户设置](../../development/design/user-settings.md)：四项可配置规则动作。

**设计基线：** 通过完整按下、释放与 fixed 边界序列验证采样输出，不以实现内部状态作为断言目标。
**公共执行约束：** 见[游戏基础设施测试索引](../game-infrastructure.md#公共执行约束)。

## 范围边界

本文不定义 `game_core` 的 canonical 归一化（见 [游戏动作与 Tick 输入](game-actions.md)），也不定义生产调度、设备生命周期和暂停请求协作（见 [输入与固定调度](../integration-system/input-and-fixed-tick.md)）。

## 测试点清单

- 固定与可配置输入、玩家隔离及同义来源合并（TC-001～TC-003）。
- 持续动作和一次性动作的完整输入时序（TC-004～TC-005）。
- 相同物理方向在 gameplay 与 UI 上下文中的领域隔离（TC-006）。
- 确认与返回取自玩家自己的旋转绑定，改绑后随之移动（TC-009）。
- 摇杆阈值及采样器不提供连发（TC-007～TC-008）。

## 设计方法与覆盖模型

| 方法 | 输入模型 / 风险 | 关联用例 |
| --- | --- | --- |
| 等价类划分 | 已绑定、未绑定、固定与可配置输入 | TC-001、TC-006 |
| 状态迁移 | 改绑旋转前后同一物理键的菜单含义 | TC-009 |
| 边界值分析 | fixed tick 前后的 press/release；摇杆阈值上下 | TC-004～TC-005、TC-007 |
| 场景法 | 双玩家隔离与多物理来源合并 | TC-002～TC-003 |
| 错误猜测 | tick 间短按、持续按住一次性动作 | TC-004～TC-005 |

## 测试用例列表

| ID | 标题 | Priority | Test Level | Concern | Domain | 前置条件 | 操作/刺激 | 测试数据 | 预期结果 | Test Basis / 证据状态 |
| --- | --- | --- | --- | --- | --- | --- | --- | --- | --- | --- |
| TC-001 | 固定方向输入与可配置动作输入均可采样，未映射输入保持为空 | P1 | Component | — | Input；Client | 一名玩家具备已确认的固定 Left/Right 物理输入，四项可配置动作已有可区分映射 | 参数化注入固定方向输入、四项已映射输入与未映射输入并采样 | device=`keyboard/gamepad`；action=六种 `GameAction` | 每个已确认输入 case 仅产生对应逻辑动作；未映射 case 的 raw `PlayerActions` 为空；Left/Right 不经用户可配置绑定 | [Confirmed] [本地输入采样：捕获物理输入](../../development/design/local-input-sampling.md#捕获物理输入)；[UI 交互动作：物理绑定关系](../../development/design/ui-action-input.md#物理绑定关系) |
| TC-002 | P1/P2 输入来源只影响对应 participant slot | P1 | Component | — | Input；Client | P1/P2 分别关联可区分的本地输入来源 | 分别产生两名玩家的固定 Left 方向输入并在 fixed tick 采样 | 四组：仅 P1、仅 P2、两者同时、均无输入 | 每组仅对应玩家 raw actions 含 Left；同时输入时两槽各含 Left；玩家输入状态互不覆盖；Left 使用固定绑定语义 | [Confirmed] [本地输入采样：设备与玩家绑定](../../development/design/local-input-sampling.md#设备与玩家绑定)；[UI 交互动作：物理绑定关系](../../development/design/ui-action-input.md#物理绑定关系) |
| TC-003 | 多个固定物理来源产生同一动作时合并为一个逻辑动作 | P1 | Component | — | Input；Client | 键盘与手柄的固定方向输入均可产生 Left | 两来源同时处于 pressed 并采样 | Keyboard Left source + Gamepad Left direction | raw actions 仅含一个 Left 位，无冲突或重复 | [Confirmed] [本地输入采样：合并多个物理输入源](../../development/design/local-input-sampling.md#合并多个物理输入源)；[UI 交互动作：物理绑定关系](../../development/design/ui-action-input.md#物理绑定关系) |
| TC-004 | 持续动作按 fixed 边界 pressed 状态采样 | P1 | Component | — | Input；Client | 参数化 Left、Right、SoftDrop | 执行按住跨 3 tick、tick 间短按短放、下个 tick 前松开三种时序 | fixed tick=`T0/T1/T2` | 按住时三个 tick 都含动作；完整发生于 tick 间的短按不产生动作；边界前松开后下一 tick 不含动作 | [Confirmed] [本地输入采样：持续动作采样](../../development/design/local-input-sampling.md#持续动作采样) |
| TC-005 | 一次性动作按完整输入时序每次 press edge 只提交一次 | P1 | Component | — | Input；Client | 参数化 HardDrop 与两种旋转动作的采样行为 | 执行 tick 间短按短放、按住跨 3 tick、松开后再次按下 | 明确的 press/release/tick 序列 | 每个 press edge 在最近后续 tick 产生一次；持续按住不重复；松开后第二次按下再产生一次 | [Confirmed] [本地输入采样：一次性动作采样](../../development/design/local-input-sampling.md#一次性动作采样) |
| TC-006 | 同一固定方向输入按上下文产生独立领域动作 | P1 | Component | — | Input；Client | 固定 Left 物理方向输入可在 gameplay 与 UI 输入上下文中解释 | 分别在 Match gameplay context 与 Menu UI context 注入同一固定 Left 方向输入 | context=`Match/Menu`；physical input=`Left direction` | Match 中只产生 `GameAction::Left` 并可进入规则输入；Menu 中只产生 `UIAction::Left` 并用于 UI；两个动作类型和输出容器保持独立；Left 不作为用户可配置绑定 | [Confirmed] [UI 交互动作：物理绑定关系](../../development/design/ui-action-input.md#物理绑定关系)、[边界](../../development/design/ui-action-input.md#边界) |
| TC-007 | 左摇杆方向按阈值 `0.5` 判定 | P2 | Component | — | Input | 采样器可接收摇杆分量 | 提交阈值上下与边界处的分量后采样 | 分量 `0.4`、`0.5`、`0.6` | `0.4` 与 `0.5` 不产生方向动作，`0.6` 产生；摇杆方向与十字键、键盘方向合并为同一逻辑动作 | [Confirmed] [本地输入采样：摇杆方向判定](../../development/design/local-input-sampling.md#摇杆方向判定) |
| TC-008 | 持续保持方向输入不产生连发 | P2 | Component | — | Input | 采样器持有已绑定的方向输入 | 保持方向按下并连续采样多个 fixed tick | 保持 `Left` 按下 5 个 tick | 每个 tick 各产生一次 `Left`，采样器不额外插入重复触发 | [Confirmed] [本地输入采样：摇杆方向判定](../../development/design/local-input-sampling.md#摇杆方向判定) |
| TC-009 | 确认与返回取自玩家自己的旋转绑定并随改绑移动 | P1 | Component | — | Input；Client | 菜单上下文，两名玩家使用默认绑定 | 分别注入两名玩家的默认旋转键与手柄旋转键；再把 P1 的 `RotateCounterClockwise` 改绑到另一物理键，注入新键与原键 | P1 `J` / `K`；P2 `Numpad1` / `Numpad2`；手柄 South / East；改绑后的新键 | 每名玩家的 `RotateCounterClockwise` 键产生该玩家的 `Confirm`、`RotateClockwise` 键产生该玩家的 `Back`，且只归属该玩家；手柄 South 为 `Confirm`、East 为 `Back`；改绑后新键产生 `Confirm`，原键不再产生 `Confirm` | [Confirmed] [UI 交互动作：物理绑定关系](../../development/design/ui-action-input.md#物理绑定关系)、[绑定来源表](../../development/design/ui-action-input.md#绑定来源表) |

## 风险查漏

六种 GameAction、输入上下文、玩家隔离、多来源、edge 和阈值均有直接用例；生产 schedule 与设备断连风险由集成测试稿覆盖。

