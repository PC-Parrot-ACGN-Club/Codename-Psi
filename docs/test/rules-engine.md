# 规则引擎测试索引

**关联需求：** [Issue #12](https://github.com/PC-Parrot-ACGN-Club/Codename-Psi/issues/12)

**关联设计：** [模块设计](../development/design/)、[设计决策](../development/decision/)

本索引组织确定性规则引擎的测试设计。在 `game_core` 内存中即可完整证明的行为按被测能力收录在 `component/`，需要聚合根或跨 crate 协作的行为收录在 `integration-system/`；具体断言只在对应测试稿中定义。

## Component 测试

| 测试稿 | 覆盖主题 |
| --- | --- |
| [规则配置与开局规格冻结](component/rule-configuration.md) | 剖面与内容库解析、三层语义校验、摘要树、冻结入口与连锁强度曲线。 |
| [盘面与活动组操控](component/falling-group-control.md) | 供给与出生、动作顺序、旋转判定、锁定与分裂、出生失败。 |
| [连锁结算](component/chain-resolution.md) | 连通组扫描、相邻垃圾清除、隐藏行排除、重力与阶段提交。 |
| [得分、攻击与垃圾攻防](component/scoring-and-attack.md) | 计分公式、余数携带、抵消、单次上限与列顺分支。 |
| [Fever 循环](component/fever-mode.md) | 量表、时间奖励、题面等级、全消组合与双通道冻结合并。 |

## Component Integration 测试

| 测试稿 | 覆盖主题 |
| --- | --- |
| [小局、BO3 与安全点](integration-system/match-and-round.md) | 安全点仲裁、失败与同时失败、小局与 BO3 推进、双方攻防矩阵。 |
| [确定性、快照与状态校验](integration-system/determinism-and-snapshot.md) | 随机流派生、快照恢复、状态校验和与验证日志。 |
| [AI 参与者](integration-system/ai-player.md) | 合法动作、候选评价、执行收尾与 AI 对局可复现性。 |

## 与基础设施测试的关系

规则数据的读取路径、schema 版本检查与失败分级属于[游戏基础设施测试](game-infrastructure.md)；本索引下的测试稿从已解析的内存数据开始，不访问文件系统与 Bevy。
