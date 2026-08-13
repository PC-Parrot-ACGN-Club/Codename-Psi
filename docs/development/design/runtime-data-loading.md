# 版本化运行数据加载

**相关模块：** `client::data`、Bevy Asset、`game_core::config`、`assets/data`、`assets/i18n`
**关联文档：** [本地化运行时](localization-runtime.md)、[本机用户设置](user-settings.md)、[TDD §5](../../TDD.md)、[assets/README.md](../../../assets/README.md)

## 目标

建立项目数据从 `assets/` 进入客户端、经过解析和版本检查、最终形成可用 typed data 的统一路径。

## 数据模型

| 数据 | 生产方 | 消费方 | 语义 |
| --- | --- | --- | --- |
| asset path | `client::data` | Bevy Asset | `assets/data/...`、`assets/i18n/...` 的稳定路径 |
| source text/bytes | Bevy Asset | parser | 已读取的原始资源内容 |
| `schema_version` | 数据文件 | parser | 版本化数据的 schema 版本 |
| typed data | parser | `client::data` | 通过解析与版本检查的数据 |
| `DataLoadError` | data/parser | 诊断 / 数据消费者 | 带资源路径、类别和底层 typed error |
| resolution | `client::data` | 消费者 / App lifecycle | `Loaded` 或 `Fallback` |

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

## 协作

| 参与者 | 提供 | 依赖 |
| --- | --- | --- |
| `client::data` | 资源路径、加载生命周期、错误上下文、fallback 和运行时注册 | Bevy Asset、对应解析器 |
| Bevy Asset | 资源读取和生命周期基础设施 | `assets/` 路径 |
| `game_core::config` | 从内存文本/字节解析 game_core typed model、schema/version 校验 | `serde`、RON/JSON |
| client 专用解析器 | 解析本地化等 client 数据 | 对应 source text/bytes |
| 功能消费者 | 使用已解析或 fallback 的 typed data | 数据已经 resolved |

### 资产根

Bevy Asset 按 `BEVY_ASSET_ROOT`、`CARGO_MANIFEST_DIR`、可执行文件所在目录的顺序确定 `assets/` 的父目录，工作目录不参与解析。由此得到两条使用约束：

- 发布形态把 `assets/` 与二进制放在同一目录。
- 经 `cargo run` 启动时 `CARGO_MANIFEST_DIR` 指向 `crates/client`，需要以 `BEVY_ASSET_ROOT` 指向仓库根才能读到项目 `assets/`。

资产根解析失败时全部数据类别进入 fallback，客户端照常运行，因此运行是否真正读到项目数据由加载诊断判定，不由启动成败判定。

### 协作时序

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

`game_core::config` 返回领域解析错误；`client::data` 增加 resource path、data category 与 underlying typed error，使错误足以定位资源和失败原因。

## fallback

运行数据读取、解析、版本或语义验证失败时使用对应内置默认值，并保留错误：

```text
failure
→ built-in default
→ Fallback { value, error }
```

因此客户端可以继续运行，同时保留原始失败原因、资源路径、fallback provenance 与原始加载错误信息。规则数据发生 `UnsupportedSchema` 时，fallback 结果同样保留该错误，使消费者能够依据原始加载状态决定自身行为。

## 边界

- 本文不定义 `RuleProfile`、角色定义、Fever 题面及其它玩法配置的完整字段与语义约束（见[玩法设计](../../gameplay.md)）。本文只固定版本头、asset 读取边界、parser 边界、typed error、fallback 与 resolved data 语义。
- `game_core::config` 只处理调用方传入的内存内容，不访问文件系统或 Bevy。
- 消费者只使用 resolved typed data。

## Test Basis

- [Issue #11](https://github.com/PC-Parrot-ACGN-Club/Codename-Psi/issues/11)：要求规则、角色、本地化数据加载路径、版本检查和可理解加载错误。
- [TDD §2](../../TDD.md)：`game_core` 不依赖文件系统，`client` 负责客户端运行时。
- [TDD §5](../../TDD.md)：定义 RON/JSON 路径、schema、本地化、设置和加载错误方向。
- [assets/README.md](../../../assets/README.md)：每个版本化数据文件包含 `schema_version`，loader 拒绝未知/不支持版本；stub 仅作解析 fixture。
