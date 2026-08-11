# 本地化运行时 Spec

**状态：** Confirmed  
**主分类：** Component  
**相关模块：** `client::i18n`  
**关联文档：** [Issue #11](https://github.com/PC-Parrot-ACGN-Club/Codename-Psi/issues/11)、[版本化运行数据加载 Contract](../contract/runtime-data-loading.md)、[TDD §5](../../TDD.md)、[PRD §5.3、§7](../../PRD.md)

## 目标

为玩家可见文本提供稳定 key 查询、`zh-CN` / `en` 语言选择、英文回退和开发诊断。

## 数据模型

| 名称 | 含义 | 约束 |
| --- | --- | --- |
| locale | 当前选择语言 | 首期支持 `zh-CN` 与 `en` |
| localization key | 玩家文本的稳定标识 | 页面代码通过 key 查询 |
| English catalog | 英文文本目录 | 当前语言缺失 key 时作为 fallback |
| current catalog | 当前语言文本目录 | 来自 `assets/i18n/<locale>.json` |
| missing-key diagnostic | 缺失文本的开发诊断 | 包含 locale 与 key |

`Localization` 作为客户端 Resource 提供只读查询能力。

## 存储格式

本地化 catalog 使用 JSON。

推荐结构：

```json
{
  "schema_version": 1,
  "locale": "en",
  "messages": {
    "main_menu.start": "Start",
    "main_menu.settings": "Settings"
  }
}
```

## 语义验证

catalog 完成 JSON 反序列化和 schema 版本检查后，解析器验证其中的 `locale` 是否属于客户端当前支持的 locale 集合。首期支持集合为：

- `zh-CN`；
- `en`。

catalog 的 `locale` 属于支持集合时通过该项语义验证；`locale` 不属于支持集合时，解析器返回[运行数据加载 Contract](../contract/runtime-data-loading.md)定义的 `InvalidData`。该错误保留违反的约束及实际 locale，资源加载层据此形成带资源上下文的 fallback 结果。

## 行为

### 查询文本

- 输入：稳定 localization key。
- 处理：
  1. 查询当前语言目录；
  2. 当前语言缺失时查询英文目录；
  3. 英文也缺失时使用 key 本身作为占位文本。
- 输出：最终文本。
- 错误语义：任一缺失情况均记录开发诊断。

查询顺序：

```text
current locale
    ↓ missing
en
    ↓ missing
key
```

### 切换语言

- 输入：用户设置中的目标 locale。
- 处理：根据当前语言设置更新当前 locale。
- 输出：后续文本查询使用新的 current catalog。
- 错误语义：目标 locale 资源不可用时使用默认/回退 catalog，并保留加载诊断。

### 默认语言

首次启动且没有有效用户设置时：

```text
locale = en
```

### 加载文本目录

- 输入：`assets/i18n/zh-CN.json`、`assets/i18n/en.json` 已读取内容。
- 处理：验证 schema 和 catalog 语义后构建 key/value catalog。
- 输出：可查询 catalog。
- 错误语义：解析、schema、语义和读取错误由运行数据加载 Contract 处理，并提供 fallback 结果；不受支持的 catalog locale 返回 `InvalidData`。

## 不变量

- 玩家可见文本通过稳定 key 查询。
- 首期支持 `zh-CN` 与 `en`。
- 默认 locale 为 `en`。
- 当前语言缺失 key 时优先返回英文。
- 当前语言和英文都缺失 key 时返回 key 本身。
- 缺失 key 会产生开发诊断。
- 本地化数据不进入规则确定性状态。
- 本地化 catalog 使用 JSON。
- catalog 的 `locale` 属于客户端当前支持的 locale 集合。

## 验收条件

- `zh-CN` 与 `en` catalog 可以从项目 JSON 数据构建。
- 没有有效用户语言设置时默认使用 `en`。
- 当前语言存在 key 时返回对应文本。
- 当前语言缺 key、英文存在 key 时返回英文并留下诊断。
- 两个 catalog 都缺 key 时返回 key 本身并留下诊断。
- `Localization` 可以作为客户端 Resource 被其它系统只读查询。
- malformed、unsupported 或违反 locale 语义约束的 catalog 使用运行数据加载 Contract 的 fallback 语义后仍能提供可查询文本。

## Test Basis

- [Confirmed] Issue #11：要求本地化数据加载路径。
- [Confirmed] TDD §5：`zh-CN` / `en` 使用稳定键值；缺失键回退英文，并在开发构建中记录诊断。
- [Confirmed] PRD §5.3：所有玩家文本使用键值本地化，首版提供 `zh-CN` 和 `en`，缺失键回退英文。
- [Confirmed] PRD §7：语言属于本机持久化设置。
- [Confirmed] 当前审核结论：默认语言为 `en`；双重缺失时使用 key 本身占位；`Localization` 作为 client Resource；本地化 catalog 采用 JSON；catalog locale 必须属于客户端当前支持的 locale 集合，违反时返回 `InvalidData`。
