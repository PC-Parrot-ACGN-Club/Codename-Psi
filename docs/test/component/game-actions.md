# 测试用例设计：游戏动作与 Tick 输入

**关联设计：** [统一游戏动作与 Tick 输入](../../development/design/game-action-input.md)

**关联实现：** `../../../crates/game_core`

## 需求理解摘要

**功能：** 验证 `PlayerActions`、`TickInputs`、动作归一化和稳定位编码。
**测试性质：** 新功能（含既有设计的补充变更）
**本轮范围：** 脱离 Bevy 的纯游戏输入值、容量边界、冲突规则和公开解码入口。
**Test Basis：**

- [Confirmed] [统一游戏动作与 Tick 输入](../../development/design/game-action-input.md)：动作集合、位编码、参与者槽位、归一化与连续动作。

**设计基线：** 所有状态在内存中构造，直接断言值语义、slot 不变量与底层位编码。
**公共执行约束：** 见[游戏基础设施测试索引](../game-infrastructure.md#公共执行约束)。

## 范围边界

本文不定义物理设备到逻辑动作的采样（见 [客户端输入](client-input.md)），也不定义 sampler 与 fixed schedule 的协作（见 [输入与固定调度](../integration-system/input-and-fixed-tick.md)）。

## 测试点清单

- `TickInputs` 的 0、2、8、9 人边界、slot 顺序和尾部清空（TC-001～TC-004）。
- 三类冲突和合法动作组合的归一化（TC-005～TC-008）。
- 连续 tick 的值表达、复制与相等比较（TC-009～TC-010）。
- 稳定位编码、slot 访问语义与保留位拒绝（TC-011～TC-013；TC-011、TC-013 Concern: Determinism）。

## 设计方法与覆盖模型

| 方法 | 输入模型 / 风险 | 关联用例 |
| --- | --- | --- |
| 边界值分析 | 参与者数量 0、2、8、9 | TC-001～TC-004 |
| 判定表 | 水平、旋转和下落冲突组合 | TC-005～TC-008 |
| 等价类划分 | 存在且为空、参与者不存在；合法位与保留位 | TC-012～TC-013 |
| 不变量检查 | 动作位 0–5 与保留位 6–7 | TC-011、TC-013 |

## 测试用例列表

| ID | 标题 | Priority | Test Level | Concern | Domain | 前置条件 | 操作/刺激 | 测试数据 | 预期结果 | Test Basis / 证据状态 |
| --- | --- | --- | --- | --- | --- | --- | --- | --- | --- | --- |
| TC-001 | 零参与者构造空 TickInputs | P2 | Component | — | Input | `PlayerActions::EMPTY` 可用 | 用空序列构造 | `len=0` | 构造成功，`len=0`，8 个存储槽均为空 | [Confirmed] [统一游戏动作与 Tick 输入：TickInputs](../../development/design/game-action-input.md#tickinputs) |
| TC-002 | 双人输入保持 participant slot 顺序并清空尾部 | P1 | Component | — | Input | 两份不同动作集合 | 构造双人 `TickInputs` | slot 0=`Left`；slot 1=`HardDrop` | `len=2`；前两槽与输入顺序一致；槽 2～7 全部为空 | [Confirmed] [统一游戏动作与 Tick 输入：构造参与者输入](../../development/design/game-action-input.md#构造参与者输入) |
| TC-003 | 八名参与者达到容量上限时构造成功 | P1 | Component | — | Input | 8 份可区分动作集合 | 按 slot 构造 | 8 个输入，使用动作组合区分相邻槽 | `len=8`，全部 slot 保持顺序和值 | [Confirmed] [统一游戏动作与 Tick 输入：构造参与者输入](../../development/design/game-action-input.md#构造参与者输入) |
| TC-004 | 九名参与者超过容量时拒绝构造 | P2 | Component | — | Input | 9 份动作集合 | 调用构造入口 | `len=9` | 返回可判定错误，不产生截断后的 `TickInputs` | [Confirmed] [统一游戏动作与 Tick 输入：构造参与者输入](../../development/design/game-action-input.md#构造参与者输入) |
| TC-005 | 左右同时成立归一化为无水平方向 | P1 | Component | — | Input | raw actions 支持组合 | 归一化 | `Left + Right` | 结果同时清除 Left、Right | [Confirmed] [统一游戏动作与 Tick 输入：水平方向冲突](../../development/design/game-action-input.md#水平方向冲突) |
| TC-006 | 双旋转同时成立归一化为无旋转 | P1 | Component | — | Input | raw actions 支持组合 | 归一化 | `RotateClockwise + RotateCounterClockwise` | 结果同时清除两个旋转动作 | [Confirmed] [统一游戏动作与 Tick 输入：旋转方向冲突](../../development/design/game-action-input.md#旋转方向冲突) |
| TC-007 | 软降与硬降同时成立时仅保留硬降 | P1 | Component | — | Input | raw actions 支持组合 | 归一化 | `SoftDrop + HardDrop` | 结果含 HardDrop 且不含 SoftDrop | [Confirmed] [统一游戏动作与 Tick 输入：下落冲突](../../development/design/game-action-input.md#下落冲突) |
| TC-008 | 独立动作与冲突外动作在归一化后保持 | P1 | Component | — | Input | raw actions 支持六种动作 | 参数化归一化 | 单动作六组；`Left + SoftDrop + RotateClockwise`；三类冲突同时出现并附加无关动作 | 单动作和合法组合保持；冲突位按三项规则消解；无关动作不受影响；重复归一化结果相同 | [Confirmed] [统一游戏动作与 Tick 输入：逻辑动作归一化](../../development/design/game-action-input.md#逻辑动作归一化) |
| TC-009 | 连续 tick 重复动作保持相同逻辑输入值 | P1 | Component | — | Input | 三个连续 tick 输入容器 | 每个 tick 构造相同动作 | tick 100～102 均为 `SoftDrop` | 三个 tick 均含 SoftDrop，无额外 held/edge 状态影响值比较 | [Confirmed] [统一游戏动作与 Tick 输入：连续动作](../../development/design/game-action-input.md#连续动作) |
| TC-010 | PlayerActions 支持复制与相等比较 | P2 | Component | — | Input | 一份多动作 canonical 值 | 复制后比较并独立用于两个 `TickInputs` | `Left + SoftDrop` | 副本与原值相等，构造过程不改变任一值 | [Confirmed] [统一游戏动作与 Tick 输入：`PlayerActions`](../../development/design/game-action-input.md#playeractions) |
| TC-011 | `PlayerActions` 位编码为锁定的稳定格式 | P0 | Component | Determinism | Input | 可在无 Bevy 环境构造 `PlayerActions` | 分别构造六个单动作集合并读取底层 `u8` | 六个 `GameAction` 各自单独置位 | `Left`/`Right`/`SoftDrop`/`HardDrop`/`RotateClockwise`/`RotateCounterClockwise` 依次对应 bit 0–5，底层值为 `1/2/4/8/16/32`；任意动作组合的 bit 6–7 恒为 0 | [Confirmed] [统一游戏动作与 Tick 输入：位编码](../../development/design/game-action-input.md#位编码) |
| TC-012 | 槽位访问器区分「参与者不存在」与「无动作」 | P1 | Component | — | Input | 构造 `len` 为 2 的 `TickInputs`，其中 slot 1 无动作 | 分别查询 slot 1 与 slot 2 | slot 1 为 `EMPTY`；slot 2 超出 `len` | slot 1 返回存在且为空动作；slot 2 返回「无该参与者」；两者结果可区分 | [Confirmed] [统一游戏动作与 Tick 输入：`TickInputs`](../../development/design/game-action-input.md#tickinputs) |
| TC-013 | `PlayerActions` 的公开解码入口拒绝保留位 | P1 | Component | Determinism | Input | 可在无 Bevy 环境经全部公开解码入口构造 `PlayerActions` | 分别向 `from_bits` 与 Serde 解码提交合法值与保留位置位的值 | 合法：`0`、`63`；非法：`64`、`128`、`192` | 合法值解码成功且等于对应动作集合；三个非法值在每个公开入口均解码失败；不存在绕过保留位不变量的公开路径 | [Confirmed] [统一游戏动作与 Tick 输入：位编码](../../development/design/game-action-input.md#位编码) |

## 风险查漏

容量、冲突、值语义、访问器歧义和公开解码入口均有直接用例；位编码的稳定性由两个 Determinism 用例双向保护。

