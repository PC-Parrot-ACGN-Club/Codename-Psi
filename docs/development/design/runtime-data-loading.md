# 版本化运行数据加载

**相关模块：** `client::data`、Bevy Asset、`game_core::config`、`assets/data`、`assets/i18n`
**关联文档：** [本地化运行时](localization-runtime.md)、[本机用户设置](user-settings.md)、[TDD §5](../../TDD.md)、[assets/README.md](../../../assets/README.md)

## 目标

建立项目数据从 `assets/` 进入客户端、经过解析和版本检查、最终形成可用 typed data 的统一路径，并规定每类数据加载失败后游戏如何继续。

## 数据模型

| 数据 | 生产方 | 消费方 | 语义 |
| --- | --- | --- | --- |
| asset path | `client::data` | Bevy Asset | `assets/data/...`、`assets/i18n/...` 的稳定路径 |
| source text/bytes | Bevy Asset | parser | 已读取的原始资源内容 |
| `schema_version` | 数据文件 | parser | 版本化数据的 schema 版本 |
| typed data | parser | `client::data` | 通过解析与版本检查的数据 |
| `DataLoadError` | data/parser | 诊断 / 数据消费者 | 带资源路径、类别和底层 typed error |
| resolution | `client::data` | 消费者 / App lifecycle | `Loaded` 或 `Failed` |

```rust
enum DataResolution<T> {
    Loaded(T),
    Failed(DataLoadError),
}
```

失败不产生替代值。数据类别在缺失时如何继续，由下节的失败分级规定，不由加载路径提供内置默认数据。

## 失败分级

分级判据是**该类数据缺失后对游戏进程的影响**：

| 级别 | 判据 | 处理 | 适用数据 |
| --- | --- | --- | --- |
| 阻断 | 缺失后不存在权威依据，无法产生可信的对局结果 | 拒绝进入受影响流程，游戏其余部分照常可用，失败原因对玩家可见 | `assets/data/` 下的规则剖面、角色玩法数据与题面 |
| 降级 | 缺失后仍有确定的替补呈现，规则不受影响 | 继续运行并使用替补，保留诊断 | `assets/i18n/` 下的文本目录、`assets/data/presentation/` 下的角色表现数据 |

级别属于数据类别，错误类型属于失败原因，两者正交：同一个 `Parse` 错误发生在规则数据上是阻断，发生在文本目录上是降级。

阻断的作用域取自同一判据，可以小于整个流程入口：规则剖面不可用时 Match 不可达；某个角色的玩法数据不可用时该角色不可选，另一个角色照常对局。

降级级数据的加载结果无论成败都不阻止启动，因为其替补行为始终可用——文本目录的替补链在[本地化运行时](localization-runtime.md#查询文本)定义，角色表现数据的替补在[角色表现数据](character-presentation.md#查询)定义。

## 协作

| 参与者 | 提供 | 依赖 |
| --- | --- | --- |
| `client::data` | 资源路径、加载生命周期、错误上下文、分级处置和运行时注册 | Bevy Asset、对应解析器 |
| Bevy Asset | 资源读取和生命周期基础设施 | `assets/` 路径 |
| `game_core::config` | 从内存文本/字节解析 game_core typed model、schema/version 校验 | `serde`、RON/JSON |
| client 专用解析器 | 解析本地化等 client 数据 | 对应 source text/bytes |
| 功能消费者 | 使用 `Loaded` 的 typed data，或按本类别的级别处置 `Failed` | 该类别已经 resolved |

### 资产根

Bevy Asset 按 `BEVY_ASSET_ROOT`、`CARGO_MANIFEST_DIR`、可执行文件所在目录的顺序确定 `assets/` 的父目录，工作目录不参与解析。由此得到两条使用约束：

- 发布形态把 `assets/` 与二进制放在同一目录。
- 经 `cargo run` 启动时 `CARGO_MANIFEST_DIR` 指向 `crates/client`，需要以 `BEVY_ASSET_ROOT` 指向仓库根才能读到项目 `assets/`。

资产根解析失败时全部数据类别得到 `Failed`：规则数据按阻断级使 Match 不可达，文本目录按降级级显示 key。客户端照常启动，失败原因由加载诊断和受阻流程的提示共同呈现。

### 协作时序

1. `client::data` 请求目标资源。
2. Bevy Asset 从项目 `assets/` 路径读取 source text/bytes。
3. source 交给 `game_core::config` 或对应 client parser。
4. parser 反序列化并检查 `schema_version` 与已定义语义约束。
5. 成功时生成 `Loaded(typed_data)`。
6. 失败时生成 typed `DataLoadError` 并由 `client::data` 增加资源上下文，形成 `Failed(error)`。
7. 客户端注册两种 resolution。
8. 消费者读取 `Loaded` 的值，或按该类别的级别处置 `Failed`；诊断系统读取错误信息。

### 规则数据的读取顺序

规则剖面、名册与题面的路径固定；角色玩法数据的路径由数据本身决定——`data/rules/play/<profile_id>/<character_id>.ron` 的两段分别取自剖面 id 与名册中的角色 id（见 [assets/README.md](../../../assets/README.md)）。因此规则数据分两段读取：先读三份固定路径文档，名册解析成功后再按其列出的角色逐个请求玩法文件。两段共用同一份超时预算，读取总时长不因分段而变。

名册本身不可用时玩法文件无从推导，该失败按阻断级直接形成 resolution。名册列出而玩法文件读取或解析失败的角色，按上节的阻断作用域只使该角色不可选。

## 错误语义

`DataLoadError` 至少区分：

- `Io`：资源读取失败；
- `Parse`：RON / JSON 无法解析；
- `UnsupportedSchema`：schema 版本不受支持；
- `InvalidData`：结构可解析但违反已确认语义约束。

`game_core::config` 返回领域解析错误；`client::data` 增加 resource path、data category 与 underlying typed error，使错误足以定位资源和失败原因。

## 边界

- 本文不定义 `RuleProfile`、角色定义、Fever 题面及其它玩法配置的完整字段与语义约束（见[玩法设计](../../gameplay.md)）。本文只固定版本头、asset 读取边界、parser 边界、typed error、失败分级与 resolution 语义。
- 本文不定义文本目录的替补链与查询语义（见[本地化运行时](localization-runtime.md)）。
- 本文不定义角色表现数据的字段与替补内容（见[角色表现数据](character-presentation.md)）。
- 本文不定义本机用户设置的读写与首次运行初值（见[本机用户设置](user-settings.md)）。
- 本文不定义阻断级失败在界面上的呈现（见[表现与 UI 设计](../../presentation.md)）。
- `game_core::config` 只处理调用方传入的内存内容，不访问文件系统或 Bevy。

## Test Basis

- [Issue #11](https://github.com/PC-Parrot-ACGN-Club/Codename-Psi/issues/11)：要求规则、角色、本地化数据加载路径、版本检查和可理解加载错误。
- [TDD §2](../../TDD.md)：`game_core` 不依赖文件系统，`client` 负责客户端运行时。
- [TDD §5](../../TDD.md)：定义 RON/JSON 路径、schema、本地化、设置和加载错误方向。
- [assets/README.md](../../../assets/README.md)：每个版本化数据文件包含 `schema_version`，loader 拒绝未知/不支持版本。
