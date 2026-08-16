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

设置字段至少覆盖：language、window mode、master volume、sfx volume、P1 keyboard mapping、P2 keyboard mapping、P1/P2 gamepad mapping、vibration、animation intensity、color assist。

`PlayerInputBindings` 只保存可配置绑定。一个动作在每个设备类别下至多持有一个物理位：设置页与角落按键提示都只显示每类设备的一条绑定，第二条会静默生效而在界面上既不可见也无法删除。

两个设备类别的绑定同时保存，与该玩家当前的[输入源](local-input-sampling.md#输入源)无关：手上没有手柄的玩家照样可以先把手柄配置好。

```rust
enum AnimationIntensity {
    Reduced,
    Full,
}
```

`AnimationIntensity` 的表现语义见[表现运行时：动画强度](presentation-runtime.md#动画强度)。color assist 的表现语义见[表现与 UI 设计 §4.1](../../presentation.md)：关闭时普通球只有纯色，开启时每种颜色获得互不相同的球内符号。它只改变球体线索，不改变任何规则时序。

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
| color assist | `false` |

### 默认输入绑定

`PlayerInputBindings` 只保存四个可配置动作，其默认绑定为：

| 动作 | P1 键盘 | P2 键盘 | 手柄 |
| --- | --- | --- | --- |
| `SoftDrop` | `S` | `↓` | DPadDown |
| `HardDrop` | `W` | `↑` | DPadUp |
| `RotateClockwise` | `K` | `Numpad2` | East |
| `RotateCounterClockwise` | `J` | `Numpad1` | South |

两个旋转动作的绑定同时承担菜单的确认与返回（见[UI 交互动作：绑定来源表](ui-action-input.md#绑定来源表)），默认值按两种用途一并选取：手柄取 South / East，使手柄上的确认与返回保持该平台的惯例。

默认绑定不为空：没有设置文件时，键盘与手柄都可以直接产生全部六个规则动作，其中 `Left` / `Right` 来自同一张表的固定绑定。

下列物理位在默认绑定下被两个领域共用，由输入上下文区分：

```text
P1 W / S        Gameplay: HardDrop / SoftDrop         Menu: Up / Down
P2 ↑ / ↓        Gameplay: HardDrop / SoftDrop         Menu: Up / Down
DPadUp / Down   Gameplay: HardDrop / SoftDrop         Menu: Up / Down
P1 J / K        Gameplay: RotateCCW / RotateCW        Menu: Confirm / Back
P2 Num1 / Num2  Gameplay: RotateCCW / RotateCW        Menu: Confirm / Back
South / East    Gameplay: RotateCCW / RotateCW        Menu: Confirm / Back
```

旋转一行随玩家的绑定移动，其余各行固定。

## 行为

### 启动加载

- 输入：平台配置目录中的设置文件。
- 处理：读取 RON、检查 schema、构建 `UserSettings`，再按下节规则修复绑定表。
- 输出：当前用户偏好。
- 错误语义：
  - 文件不存在：使用内置默认设置；
  - 解析或版本异常：记录诊断并使用内置默认设置。

### 绑定表修复

解析成功的文件仍可能违反[数据模型](#数据模型)的每类设备至多一条绑定，或违反[输入绑定冲突](#输入绑定冲突)的判定范围。这类文件不是解析错误，但按其内容游玩会得到界面上看不见的行为。

- 输入：解析后的 `UserSettings`。
- 处理：按玩家槽位、动作顺序遍历绑定表，丢弃被固定绑定占用、被同一动作的同类设备已占用、或被判定范围内的其它绑定已占用的物理位。每类设备保留最先出现的一条——那是设置页一直显示的一条。
- 输出：满足全部绑定不变量的 `UserSettings`；丢弃项逐条记录诊断，并触发一次保存，使修复落到文件上。
- 错误语义：修复不失败；一份绑定全部被丢弃的动作即为未绑定动作，不产生输入。

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
  4. 无冲突时写入并结束捕获，有冲突时不写入、结束捕获并把冲突结果交给设置 UI 展示。
- 输出：更新后的 `PlayerInputBindings`，或一次未改变绑定表的捕获。
- 错误语义：捕获期间的返回输入取消本次捕获并保留原绑定；捕获态不产生 `UIAction`，也不产生规则动作。

捕获直接读取物理设备，不经过[输入源](local-input-sampling.md#输入源)：持有手柄的玩家因此仍能录入自己的键盘绑定。

取消输入取自该玩家的返回绑定，且**两个设备类别都接受**：目标设备类别之外的输入不作为候选绑定，但仍可取消。目标设备没有连接时，捕获等不到候选绑定，键盘上的取消是唯一出路。

暂停键额外充当取消：返回绑定本身可以为空（[绑定表修复](#绑定表修复)可能丢弃某个动作的全部绑定），此时它是唯一不依赖绑定表的出路。

设备未连接时该设备类别的重绑定项不可选（见[页面导航与焦点](page-navigation.md#设备可用性)），因此上述两条都是兜底而非常规路径。

写入生效后立即用于运行时采样，不等待下一次进入对局。

### 输入绑定冲突

- 输入：设置页面准备写入的新绑定（仅 `SoftDrop` / `HardDrop` / `RotateClockwise` / `RotateCounterClockwise` 四个可配置动作）。
- 处理：按下面两条依次判断该物理位是否已被占用。
- 输出：可供设置 UI 展示的冲突结果，包含被占用方——占用它的玩家与动作，或「固定绑定」。
- 错误语义：冲突时拒绝写入，绑定表保持原样，设置 UI 展示是谁占用了该物理位。占用方不会被夺走绑定：一个动作失去全部绑定就不再产生输入，而两个旋转动作同时承担菜单的确认与返回，失去绑定的玩家将无法操作设置页本身。一个物理位需要改绑到别处时，先把占用方改到其它物理位；被固定绑定占用的物理位没有这条出路，只能另选一个。

**判定范围按硬件划分，不按玩家划分。** 一块键盘由两名本地玩家共用，因此一个按键被任一玩家占用即为占用；每名玩家各持一只手柄，因此手柄按键只在该玩家自己的映射表内判定——两名玩家的手柄默认列相同即由此成立。

**固定绑定占用的物理位参与判定，判定按输入上下文。** 一个物理位能否被某个可配置动作取用，取决于它的固定含义是否与该动作在同一上下文中同时生效：

| 固定绑定 | 生效上下文 | 可被取用的可配置动作 |
| --- | --- | --- |
| 水平方向 | Gameplay 的 `Left` / `Right`，Menu 的焦点左右 | 无 |
| 暂停 | 全部 | 无 |
| 垂直方向 | 仅 Menu 的焦点上下 | `SoftDrop`、`HardDrop` |

垂直方向一行即[默认输入绑定](#默认输入绑定)中 W / S、↑ / ↓、DPadUp / DPadDown 双用的依据；两个旋转动作在菜单中承担确认与返回，与焦点上下同时生效，故不在可取用之列。

## 平台目录

设置文件使用 workspace 已引入的 `directories` 能力定位平台标准应用配置目录。

## 协作

| 设置 | 消费方 | 应用时机 |
| --- | --- | --- |
| language | [本地化运行时](localization-runtime.md#切换语言) | 立即 |
| window mode | 客户端窗口 | 立即 |
| master volume、sfx volume | 客户端音频输出 | 立即 |
| `PlayerInputBindings` | [本地输入采样](local-input-sampling.md) | 立即 |
| vibration、animation intensity、color assist | [表现运行时](presentation-runtime.md) | 立即 |

全部设置在玩家确认修改后立即生效，不需要重启，也不需要离开设置页。持久化与生效相互独立：写盘失败不回退已经生效的内存值。

## 边界

- 本文不定义固定绑定的键位（见[UI 交互动作：物理绑定关系](ui-action-input.md#物理绑定关系)）。固定绑定动作不是可配置动作，不进入绑定表，也不能作为重绑定的目标；它们占用的物理位按[输入绑定冲突](#输入绑定冲突)参与判定。
- 本文不定义设置页面的焦点顺序、层级与控件布局（见[页面导航与焦点：设置页层级](page-navigation.md#设置页层级)）。
- 本文不定义一名玩家当前由哪个设备驱动（见[本地输入采样：输入源](local-input-sampling.md#输入源)）。绑定表两个设备类别都保存，与谁正在生效无关。
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
