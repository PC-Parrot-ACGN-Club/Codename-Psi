# 本机用户设置 Spec

**状态：** Confirmed  
**主分类：** Component  
**相关模块：** `client::settings`  
**关联文档：** [Issue #11](https://github.com/PC-Parrot-ACGN-Club/Codename-Psi/issues/11)、[PRD §5.2、§7](../../PRD.md)、[TDD §4–§5](../../TDD.md)

## 目标

保存并恢复客户端本机偏好，为窗口、音频、输入、本地化、震动和表现设置提供统一持久化来源。

`UserSettings` 提供当前用户设置，并负责持久化和恢复。

## 数据模型

| 名称 | 含义 | 约束 |
| --- | --- | --- |
| `UserSettings` | 用户当前保存的本机偏好 | RON、带 schema 版本 |
| settings path | 平台标准应用配置目录中的文件 | 通过平台目录能力解析 |
| `PlayerInputBindings` | P1/P2 的键盘与手柄绑定 | 两名玩家独立保存 |

设置字段至少覆盖：

- language；
- window mode；
- master volume；
- sfx volume；
- P1 keyboard mapping；
- P2 keyboard mapping；
- P1/P2 gamepad mapping；
- vibration；
- character performance；
- animation intensity。

## 默认值

### 通用设置

| 设置 | 默认值 |
| --- | --- |
| language | `en` |
| window mode | `Windowed` |
| master volume | `1.0` |
| sfx volume | `1.0` |
| vibration | `true` |
| character performance | `true` |
| animation intensity | `Normal` / `1.0` |

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

- 输入：设置页面准备写入的新绑定。
- 处理：判断新绑定是否与同一配置范围内已有绑定冲突。
- 输出：可供设置 UI 判断的冲突结果。
- 错误语义：冲突属于可处理业务结果，由设置 UI 决定覆盖或重新绑定。

运行时采样中的逻辑动作冲突由 `core::input` 处理，与这里的绑定编辑冲突无关。

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
- 默认语言、窗口、音量、震动和动画设置均有明确值。
- 运行时组件可以读取当前 `UserSettings`。
- 写入失败不会破坏已有正式设置文件。

## Test Basis

- [Confirmed] Issue #11：要求持久化语言、窗口模式、音量、键盘映射、手柄映射、震动、角色演出和动画强度。
- [Confirmed] PRD §5.2：P1/P2 独立映射、键盘/手柄支持和冲突提示。
- [Confirmed] PRD §7：定义需要持久化的用户设置。
- [Confirmed] TDD §4：客户端维护可序列化输入映射。
- [Confirmed] TDD §5：设置使用 RON，保存到平台标准应用配置目录；解析错误使用安全默认值并给出诊断。
- [Confirmed] 当前审核结论：`UserSettings` 只保存用户偏好；采用临时文件 replace/rename 保存；使用 `directories`；默认值按本文定义。
