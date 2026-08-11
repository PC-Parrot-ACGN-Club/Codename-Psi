# 本地输入采样 Contract

**状态：** v1
**主分类：** Component Integration  
**相关模块：** `client::input::LocalInputSampler`、`core::input`  
**关联文档：** [本地输入采样器 Spec](../component/local-input-sampler.md)、[统一游戏动作与 Tick 输入 Spec](../component/game-action-input.md)、[TDD §3–§4](../../TDD.md)

## 目的

定义本地输入采样器如何在 fixed tick 边界生成逻辑动作，并将结果交给 `core::input` 形成规则层消费的 `PlayerActions`。

## 参与者与职责

| 参与者 | 提供 | 依赖                           |
| --- | --- |--------------------------------|
| `LocalInputSampler` | 每名本地玩家的 raw `PlayerActions` | Bevy Input、PlayerInputBinding |
| `core::input` | 动作类型与逻辑冲突归一化 | raw `PlayerActions`            |

## 数据契约

| 数据 | 生产方 | 消费方              | 语义 |
| --- | --- |---------------------| --- |
| `PlayerInputBindings` | 设置系统 | `LocalInputSampler` | 每名本地玩家的物理输入映射 |
| raw `PlayerActions` | `LocalInputSampler` | `core::input`       | 当前 fixed tick 采样到的逻辑动作，可包含互斥组合 |
| canonical `PlayerActions` | `core::input` | 后续输入装配阶段    | 应用统一冲突规则后的规则输入 |

## 协作时序

1. 普通 Update 捕获输入状态和尚未提交的一次性操作；fixed tick 根据当前绑定及采样状态生成 raw PlayerActions
2. fixed tick 到来时，采样器根据当前物理 pressed 状态、pending press edge 和当前生效绑定生成每名本地玩家的 raw `PlayerActions`。
3. 相同逻辑动作的多个物理来源在采样阶段合并。
4. raw `PlayerActions` 交给 `core::input` 进行逻辑动作归一化。

## 逻辑动作边界

采样端只报告该 tick 采样到哪些逻辑动作，不决定互斥逻辑组合的最终含义；统一归一化规则以[统一游戏动作与 Tick 输入 Spec](../component/game-action-input.md)的"逻辑动作归一化"一节为准，本 Contract 不重复定义。

相同逻辑语义的多个物理来源不属于冲突：

```text
Keyboard Left + Gamepad Left
→ Left
```

其它动作组合保持原样，由玩法规则决定同 tick 的执行顺序。

## 设置边界

- `PlayerInputBindings` 是采样器输入。
- 绑定编辑、重复物理按键提示、覆盖或重新绑定交互属于设置页面，不属于本 Contract。
- 两个不同物理输入源映射到同一逻辑动作不会被运行时采样视为冲突。

## 双方承诺

- 持续动作按 fixed tick 边界按下状态采样；
- 一次性动作每次物理按下最多提交一次，并保留 tick 间完成的按下操作。
- `core::input`：对所有来源使用相同的逻辑动作归一化规则。

## 验收条件

- 本地键盘/手柄输入可经采样与归一化形成规则层 `PlayerActions`。

## Test Basis

- [Confirmed] Issue #11：要求键盘/手柄映射、P1/P2 独立映射以及 P1、P2、AI、网络统一逻辑动作入口。
- [Confirmed] TDD §3–§4：规则核心消费 tick 动作；设备映射由 client 负责。
- [Confirmed] 当前审核结论：新增 `LocalInputSampler` Component；Contract 只描述采样输出与逻辑输入之间的关系；逻辑动作冲突统一在 `core::input` 处理；core 最多支持 8 个 participant slots。
