# 本机用户设置

**相关模块：** `client::settings`
**关联文档：** [UI 交互动作](ui-action-input.md)、[统一游戏动作与 Tick 输入](game-action-input.md)、[版本化运行数据加载](runtime-data-loading.md)、[PRD §5.2、§7](../../PRD.md)、[TDD §4–§5](../../TDD.md)

## 目标

保存并恢复客户端本机偏好，为窗口、音频、输入、本地化和震动提供统一持久化来源。

## 数据模型

| 名称 | 含义 | 约束 |
| --- | --- | --- |
| `UserSettings` | 用户当前保存的本机偏好 | RON、带 schema 版本 |
| settings path | 平台标准应用配置目录中的文件 | 通过平台目录能力解析 |
| `PlayerInputBindings` | P1/P2 的键盘与手柄绑定 | 两名玩家独立保存 |

设置字段至少覆盖：language、window mode、master volume、sfx volume、P1 keyboard mapping、P2 keyboard mapping、P1/P2 gamepad mapping、vibration、animation intensity。

`PlayerInputBindings` 只保存可配置绑定。

```rust
enum AnimationIntensity {
    Reduced,
    Full,
}
```

`AnimationIntensity` 的表现语义见[表现运行时：动画强度](presentation-runtime.md#动画强度)。

## 默认值

### 通用设置

| 设置 | 默认值 |
| --- | --- |
| language | `en` |
| window mode | `Windowed` |
| master volume | `1.0` |
| sfx volume | `1.0` |
| vibration | `true` |
| animation intensity | `Full` |

### 默认输入绑定

`PlayerInputBindings` 只保存四个可配置动作，其默认绑定为：

| 动作 | P1 键盘 | P2 键盘 | 手柄 |
| --- | --- | --- | --- |
| `SoftDrop` | `S` | `↓` | DPadDown |
| `HardDrop` | `W` | `↑` | DPadUp |
| `RotateClockwise` | `K` | `Numpad2` | South |
| `RotateCounterClockwise` | `J` | `Numpad1` | West |

默认绑定不为空：没有设置文件时，键盘与手柄都可以直接产生全部六个规则动作，其中 `Left` / `Right` 来自[UI 交互动作：固定绑定表](ui-action-input.md#固定绑定表)。

下列物理位在默认绑定下被两个领域共用，由输入上下文区分：

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

- 输入：更新后的 `UserSettings`。
- 处理：
  1. 更新内存中的 `UserSettings`；
  2. 序列化到临时文件；
  3. 完成写入后通过 replace/rename 替换正式设置文件。
- 输出：重启后可恢复的新设置。
- 错误语义：写入失败时保留当前内存值并返回可观察错误，已有正式设置文件不受破坏。

### schema 演进

设置文件的 `schema_version` 只在不兼容改动时提升：

| 改动 | `schema_version` | 已有设置文件 |
| --- | --- | --- |
| 增加字段 | 不变 | 缺失字段取该字段默认值，其余用户选择全部保留 |
| 删除字段 | 不变 | 忽略未知字段 |
| 改变已有字段的含义或取值域 | 提升 | 整体回到内置默认设置并记录诊断 |

不提供逐字段迁移：加字段由默认值补齐，不兼容改动由整体回默认处置。

### 绑定捕获

- 输入：待重绑定的可配置动作、目标玩家、目标设备类别，以及捕获期间的一次物理输入。
- 处理：
  1. 进入捕获态，暂停该设备类别的常规输入消费；
  2. 记录首个属于目标设备类别的物理输入作为候选绑定；
  3. 对候选绑定执行下节的冲突判断；
  4. 无冲突时写入，有冲突时把冲突结果交给设置 UI，由玩家选择覆盖或取消。
- 输出：更新后的 `PlayerInputBindings`，或一次被取消的捕获。
- 错误语义：捕获期间的返回输入取消本次捕获并保留原绑定；捕获态不产生 `UIAction`，也不产生规则动作。

覆盖生效后立即用于运行时采样，不等待下一次进入对局。

### 输入绑定冲突

- 输入：设置页面准备写入的新绑定（仅 `SoftDrop` / `HardDrop` / `RotateClockwise` / `RotateCounterClockwise` 四个可配置动作）。
- 处理：判断新绑定是否与同一配置范围内已有绑定冲突。配置范围是**同一玩家的同一设备类别**：P1 键盘与 P2 键盘互不冲突，同一玩家的键盘与手柄绑定也互不冲突。
- 输出：可供设置 UI 判断的冲突结果，包含被占用的动作。
- 错误语义：冲突属于可处理业务结果，由设置 UI 决定覆盖或重新绑定。覆盖使原先占用该物理位的动作变为未绑定，该动作在重新绑定前不产生输入。

## 平台目录

设置文件使用 workspace 已引入的 `directories` 能力定位平台标准应用配置目录。

## 协作

| 设置 | 消费方 | 应用时机 |
| --- | --- | --- |
| language | [本地化运行时](localization-runtime.md#切换语言) | 立即 |
| window mode | 客户端窗口 | 立即 |
| master volume、sfx volume | 客户端音频输出 | 立即 |
| `PlayerInputBindings` | [本地输入采样](local-input-sampling.md) | 立即 |
| vibration、animation intensity | [表现运行时](presentation-runtime.md) | 立即 |

全部设置在玩家确认修改后立即生效，不需要重启，也不需要离开设置页。持久化与生效相互独立：写盘失败不回退已经生效的内存值。

## 边界

- 本文不定义固定绑定的键位（见[UI 交互动作：物理绑定关系](ui-action-input.md#物理绑定关系)）。固定绑定动作不进入绑定冲突检测范围。
- 本文不定义设置页面的焦点顺序与控件布局（见[页面导航与焦点](page-navigation.md)）。
- 本文不定义运行时采样中的逻辑动作冲突（见[统一游戏动作与 Tick 输入：逻辑动作归一化](game-action-input.md#逻辑动作归一化)），它与绑定编辑冲突无关。
- `UserSettings` 只保存用户偏好，不进入规则 fixed tick 的确定性状态。

## Test Basis

- [Issue #11](https://github.com/PC-Parrot-ACGN-Club/Codename-Psi/issues/11)：要求持久化语言、窗口模式、音量、键盘映射、手柄映射与震动。
- [PRD §5.2](../../PRD.md)：P1/P2 独立映射、键盘/手柄支持和冲突提示。
- [PRD §7](../../PRD.md)：定义需要持久化的用户设置。
- [TDD §4](../../TDD.md)：客户端维护可序列化输入映射。
- [TDD §5](../../TDD.md)：设置使用 RON，保存到平台标准应用配置目录；解析错误使用安全默认值并给出诊断。
- [Issue #13](https://github.com/PC-Parrot-ACGN-Club/Codename-Psi/issues/13)：要求设置页可修改语言、窗口模式、音量、键盘/手柄映射、震动与动画强度，按规定时机生效并在重启后恢复。
- [PRD §5.1](../../PRD.md)：设置页必需内容包含动画强度。
