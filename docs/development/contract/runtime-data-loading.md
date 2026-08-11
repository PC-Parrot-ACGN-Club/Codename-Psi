# 版本化运行数据加载 Contract

**状态：** Confirmed
**主分类：** Component Integration
**相关模块：** `client::data`、Bevy Asset、`game_core::config`、`assets/data`、`assets/i18n`
**关联文档：** [Issue #11](https://github.com/PC-Parrot-ACGN-Club/Codename-Psi/issues/11)、[TDD §5](../../TDD.md)、[assets/README.md](../../../assets/README.md)

## 目的

建立项目数据从 `assets/` 进入客户端、经过解析和版本检查、最终形成可用 typed data 的统一路径。

## 参与者与职责

| 参与者 | 提供 | 依赖 |
| --- | --- | --- |
| `client::data` | 资源路径、加载生命周期、错误上下文、fallback 和运行时注册 | Bevy Asset、对应解析器 |
| Bevy Asset | 资源读取和生命周期基础设施 | `assets/` 路径 |
| `game_core::config` | 从内存文本/字节解析 game_core typed model、schema/version 校验 | `serde`、RON/JSON |
| client 专用解析器 | 解析本地化等 client 数据 | 对应 source text/bytes |
| 功能消费者 | 使用已解析或 fallback 的 typed data | 数据已经 resolved |

## 数据契约

| 数据 | 生产方 | 消费方                 | 语义 |
| --- | --- |------------------------| --- |
| asset path | `client::data` | Bevy Asset             | `assets/data/...`、`assets/i18n/...` 的稳定路径 |
| source text/bytes | Bevy Asset | parser                 | 已读取的原始资源内容 |
| `schema_version` | 数据文件 | parser                 | 版本化数据的 schema 版本 |
| typed data | parser | `client::data`         | 通过解析与版本检查的数据 |
| `DataLoadError` | data/parser | 诊断 / 数据消费者      | 带资源路径、类别和底层 typed error |
| resolution | `client::data` | 消费者 / App lifecycle | `Loaded` 或 `Fallback` |

推荐结果模型：

```rust
enum DataResolution<T> {
    Loaded(T),
    Fallback {
        value: T,
        error: DataLoadError,
    },
}
```

两种结果都表示该数据类别已经 resolved，可以向消费者提供值。

## 协作时序

1. `client::data` 请求目标资源。
2. Bevy Asset 从项目 `assets/` 路径读取 source text/bytes。
3. source 交给 `game_core::config` 或对应 client parser。
4. parser 反序列化并检查 `schema_version` 与已定义语义约束。
5. 成功时生成 `Loaded(typed_data)`。
6. 失败时生成 typed `DataLoadError`，并由 `client::data` 选择该数据类别的内置默认值，形成 `Fallback { value, error }`。
7. 客户端注册 resolved data。
8. 消费者只读取 resolved 后的数据值；消费者与诊断系统仍可以读取 fallback 的原始错误信息。

## 错误语义

`DataLoadError` 至少区分：

- `Io`：资源读取失败；
- `Parse`：RON / JSON 无法解析；
- `UnsupportedSchema`：schema 版本不受支持；
- `InvalidData`：结构可解析但违反已确认语义约束。

`game_core::config` 返回领域解析错误；`client::data` 增加：

- resource path；
- data category；
- underlying typed error。

## fallback

运行数据读取、解析、版本或语义验证失败时使用对应内置默认值。

fallback 不吞掉错误：

```text
failure
→ built-in default
→ Fallback { value, error }
```

因此客户端可以继续运行，同时保留：

- 原始失败原因；
- 资源路径；
- fallback provenance；
- 原始加载错误信息。

规则数据发生 UnsupportedSchema 时，fallback 结果继续保留该错误，使后续消费者能够依据原始加载状态决定自身行为。

## 双方承诺

- `client::data`：拥有 asset 路径、加载状态、fallback 和错误上下文。
- Bevy Asset：提供客户端运行时资源读取能力。
- `game_core::config`：只处理调用方传入的内存内容，不访问文件系统或 Bevy。
- 数据模型方：版本化数据包含明确 schema 版本。
- 消费者：只使用 resolved typed data。
- fallback：始终使用内置默认值，并保留原始错误。
- 共同约束：错误包含足以定位资源和失败原因的诊断信息。

## schema 边界

Issue #11 只固定：

- 版本头；
- asset 读取边界；
- parser 边界；
- typed error；
- fallback；
- resolved data 语义。

以下完整数据模型由玩法实现任务定义：

- `RuleProfile`；
- 角色定义；
- Fever 题面；
- 其它玩法配置字段和语义约束。

## 验收条件

- 客户端可以通过 Bevy Asset 从 `assets/data/` 读取 RON 数据。
- 客户端可以通过 Bevy Asset 从 `assets/i18n/` 读取 JSON 数据。
- 支持 schema 得到 `Loaded(typed_data)`。
- `Io`、`Parse`、`UnsupportedSchema`、`InvalidData` 均可得到带资源上下文的 `DataLoadError`。
- 任一加载失败可以使用对应内置默认值形成 `Fallback`。
- fallback 后消费者仍能取得 typed data，同时诊断可以读取原始错误。
- `game_core` 解析器可以完全基于内存输入测试。
- 规则、角色和 Fever 的完整 schema 可以在后续任务中沿用同一加载边界。

## Test Basis

- [Confirmed] Issue #11：要求规则、角色、本地化数据加载路径、版本检查和可理解加载错误。
- [Confirmed] TDD §2：`game_core` 不依赖文件系统，`client` 负责客户端运行时。
- [Confirmed] TDD §5：定义 RON/JSON 路径、schema、本地化、设置和加载错误方向。
- [Confirmed] assets/README.md：每个版本化数据文件包含 `schema_version`，loader 拒绝未知/不支持版本；stub 仅作解析 fixture。
- [Confirmed] 当前审核结论：使用 Bevy Asset；`DataLoadError` 增加资源上下文；加载失败统一使用内置默认值 fallback；完整玩法 schema 延后定义。
