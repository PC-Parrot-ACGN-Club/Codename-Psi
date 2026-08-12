# 本机用户设置 Spec

**状态：** Confirmed
**主分类：** Component
**相关模块：** `client::settings`
**关联文档：** [Issue #11](https://github.com/PC-Parrot-ACGN-Club/Codename-Psi/issues/11)、[PRD §5.2、§7](../../PRD.md)、[TDD §4–§5](../../TDD.md)、[UI 交互动作 Spec](ui-action-input.md)、[统一游戏动作与 Tick 输入 Spec](game-action-input.md)

## 目标

保存并恢复客户端本机偏好，为窗口、音频、输入、本地化和震动提供统一持久化来源。

`UserSettings` 提供当前用户设置，并负责持久化和恢复。

## 数据模型

| 名称 | 含义 | 约束 |
| --- | --- | --- |
| `UserSettings` | 用户当前保存的本机偏好 | RON、带 schema 版本 |
| settings path | 平台标准应用配置目录中的文件 | 通过平台目录能力解析 |
| `PlayerInputBindings` | P1/P2 的键盘与手柄绑定 | 两名玩家独立保存 |

`PlayerInputBindings` 只保存可配置绑定；固定绑定范围见[UI 交互动作 Spec](../component/ui-action-input.md)的“物理绑定关系”一节。

设置字段至少覆盖：

- language；
- window mode；
- master volume；
- sfx volume；
- P1 keyboard mapping；
- P2 keyboard mapping；
- P1/P2 gamepad mapping；
- vibration。

## 默认值

### 通用设置

| 设置 | 默认值 |
| --- | --- |
| language | `en` |
| window mode | `Windowed` |
| master volume | `1.0` |
| sfx volume | `1.0` |
| vibration | `true` |

### 默认输入绑定

`PlayerInputBindings` 只保存四个可配置动作，其默认绑定为：

| 动作 | P1 键盘 | P2 键盘 | 手柄 |
| --- | --- | --- | --- |
| `SoftDrop` | `S` | `↓` | DPadDown |
| `HardDrop` | `W` | `↑` | DPadUp |
| `RotateClockwise` | `K` | `Numpad2` | South |
| `RotateCounterClockwise` | `J` | `Numpad1` | West |

默认绑定不为空：没有设置文件时，键盘与手柄都可以直接产生全部六个规则动作，其中 `Left` / `Right` 来自[UI 交互动作 Spec](ui-action-input.md)定义的固定方向绑定。

下列物理位在默认绑定下被两个领域共用，由输入上下文区分，不属于绑定冲突：

```text
P1 W / S        Gameplay: HardDrop / SoftDrop    Menu: Up / Down
P2 ↑ / ↓        Gameplay: HardDrop / SoftDrop    Menu: Up / Down
DPadUp / Down   Gameplay: HardDrop / SoftDrop    Menu: Up / Down
South           Gameplay: RotateClockwise        Menu: Confirm
```

## 行为

### 启动加载

- 输入：平台配置目录中的设置文件。
- 处理：读取 RON、检查 schema、构建 `UserSettings`。
- 输出：当前用户偏好。
- 错误语义：
  - 文件不存在：使用内置默认设置；
  - 解析或版本异常：记录诊断并使用内置默认设置。

### 保存设置

- 输入：更新后的 UserSettings。
- 处理：
  1. 更新内存中的 `UserSettings`；
  2. 序列化到临时文件；
  3. 完成写入后通过 replace/rename 替换正式设置文件。
- 输出：重启后可恢复的新设置。
- 错误语义：写入失败时保留当前内存值并返回可观察错误。

### 输入绑定冲突

- 输入：设置页面准备写入的新绑定（仅 `SoftDrop` / `HardDrop` / `RotateClockwise` / `RotateCounterClockwise` 四个可配置动作）。
- 处理：判断新绑定是否与同一配置范围内已有绑定冲突。
- 输出：可供设置 UI 判断的冲突结果。
- 错误语义：冲突属于可处理业务结果，由设置 UI 决定覆盖或重新绑定。

固定绑定的动作不进入本行为的冲突检测范围。

运行时采样中的逻辑动作冲突由 `game_core::input` 处理，与这里的绑定编辑冲突无关。

## 平台目录

设置文件使用 workspace 已引入的 `directories` 能力定位平台标准应用配置目录。

## 不变量

- `UserSettings` 只保存用户偏好。
- P1 与 P2 输入映射分别持久化。
- 设置文件带 schema 版本。
- 设置存放在平台标准应用配置目录。

- 用户设置不进入规则 fixed tick 的确定性状态。
- 设置保存采用临时文件后 replace/rename 的写入策略。

## 验收条件

- 没有设置文件时使用完整默认设置启动。
- malformed/unsupported 设置文件产生诊断并回退完整默认设置。
- 保存后重启可以恢复 `UserSettings`。
- P1/P2 键盘和手柄映射可以独立保存与恢复。
- 默认语言、窗口、音量和震动均有明确值。
- 没有设置文件时，P1、P2 的四个可配置动作均已具备非空默认绑定，键盘与手柄都可直接产生全部六个规则动作。
- 运行时组件可以读取当前 `UserSettings`。
- 写入失败不会破坏已有正式设置文件。

## Test Basis

- [Confirmed] Issue #11：要求持久化语言、窗口模式、音量、键盘映射、手柄映射与震动。
- [Inferred] 待确认设计结论：移除 character performance 与 animation intensity 两项表现开关。二者在 R1 没有消费者，且各自要求表现层额外实现一条降级渲染路径；`UserSettings` 使用 `#[serde(default)]`，删除字段不需要 schema 升版或迁移，将来重新引入同样是零迁移成本。
- [Confirmed] PRD §5.2：P1/P2 独立映射、键盘/手柄支持和冲突提示。
- [Confirmed] PRD §7：定义需要持久化的用户设置。
- [Confirmed] TDD §4：客户端维护可序列化输入映射。
- [Confirmed] TDD §5：设置使用 RON，保存到平台标准应用配置目录；解析错误使用安全默认值并给出诊断。
- [Confirmed] 当前审核结论：`UserSettings` 只保存用户偏好；采用临时文件 replace/rename 保存；使用 `directories`；默认值按本文定义。
- [Confirmed] 当前审核结论：四个可配置动作具备非空默认绑定（见「默认输入绑定」）；默认绑定中与固定绑定共用的物理位由输入上下文区分，不进入绑定冲突检测。
