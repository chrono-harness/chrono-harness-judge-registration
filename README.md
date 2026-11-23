# chrono-harness-judge-registration

验证项目登记、执行方法与消费合同。

使用 `cargo build --locked --manifest-path crates/judge-registration/Cargo.toml` 构建。安装 Verus 0.2026.04.24.f8e1704，运行 `VERUS=/path/to/verus ./check.sh` 检查格式、构建、测试及证明。

源码来自 [固定版本的原实现](https://github.com/ChronoAIProject/chrono-harness/tree/5f6892dfa2f73675a84e4ebdc3b06eb83b824d6e/crates/judge-registration)。

`chrono-judge-registration execution-methods` 从标准输入读取一份项目和脚本登记 JSON，沿用判官的 64 MiB 输入上限。成功时输出从操作 ID 到全部方法的 JSON 映射，包含 owner、operation、tool、argv 和已登记的 output_contract。相同操作 ID 的不同声明分别保留，供发布配方检查歧义。成功退出 0，读取、解析、登记或写出失败退出 2，错误发往标准错误。

`chrono-judge-registration test-owners` 接受同一输入，按测试绑定规则验证登记，输出每个所有者对应的测试 ID 列表，包括显式测试组。宿主的发布预览读取这个映射以选择受影响单元。这些查询只读标准输入。

`test-bindings-v1` 读取同一份项目登记，按测试 ID 输出全部绑定。每条绑定包含 owner、action 和 operation。CI 用它按登记的终止操作执行测试，并把受影响的测试关联到所有者。

`scoped-test-bindings-v1` 用于 scoped CI，输入和输出与上面的绑定查询相同。项目登记 v2 先通过完整 schema 校验，再返回绑定。v1 沿用 scoped CI 读取旧登记时的绑定规则。错误写入 stderr 并退出 2。

`chrono-judge-registration filemap-views` 接受一个封存的 chrono-judge/v1 请求。在两个固定端点上按判官协议核对登记，保留历史转换和已有视图的绑定检查。成功时输出 request_id、base、candidate，每端包含 FILEMAP 登记路径、解释后的文档和节点 ID。登记拒绝时输出相同 request_id 和原始 error。启用 Git 观察时两种结果都带 git_facts。读取、解码或写出失败时退出 2。成本判官据此读取两端的成本声明。

`chrono-judge-registration worktree-inputs-v1` 从 stdin 读取一个 JSON 对象。成功时 stdout 返回 JSON 并退出 0，拒绝输入时在 stderr 给出原因并退出 2。工作树提供已读取的宿主数据，保留自己的 Git 读取收据。

| operation | 输入 | 输出 |
| --- | --- | --- |
| snapshot | entry 和按路径索引的固定 documents | 完整登记检查后的 effective_config 路径及该配置选定的 filemap |
| custody-config | config | 通过宿主配置校验的 filemap_path |
| custody-filemap | filemap | FILEMAP 格式通过时返回 valid 为 true |
| nonartifact-paths | config 和 paths | 保留顺序的未登记为产物的路径 |

`chrono-judge-registration classification-views-v1` 从 stdin 读取封存的 chrono-judge/v1 请求。它校验请求身份，按相同固定端点和历史转换规则解释登记，返回 request_id、base、candidate。每端的 endpoint 给出 workflow 路径、变更路径在 FILEMAP 的 surfaces 和 semantic_fields；documents 包含此次变更涉及的已解释登记文档，warning_code 取自本端工作流。mixed 可据此选择所需文档，外部文档仍从指定提交读取。登记拒绝返回原始 error，绑定 Git 的请求同时返回 git_facts。解码或传输失败退出 2。

`chrono-judge-registration capture-plan-v1` 接受 config（配置入口路径）、documents（调用方从固定提交读取的登记文档）和可选 unit。它执行登记校验并返回 effective_config、selection、schema 及该次捕获所需的输入 ID 集合 required。成功退出 0，未知单元或无效登记退出 2 并写出原错误。实际环境、文件和输出快照由 inputs 负责。

`collection-inputs-v1` 从标准输入读取 `config`、`base`、`candidate` 和 `selected`。两端的登记文档用路径到 JSON 对象的映射表示。命令按选中的测试返回两端收集所需的输入 ID，保留基线独有或候选新增执行单元的处理。成功写出 `base` 和 `candidate` 数组，错误写入 stderr 并退出 2。

`impact-views-v1` 供 FILEMAP 解释两个固定端点的登记。stdin 中的 `originals` 按基线、候选顺序给出两个按仓内路径索引的 UTF-8 原文表，保留换行和空白。`operation.mode` 为 `endpoints` 时提供 `base`、`candidate`、`config` 和可选的 `scheduling`。为 `request` 或 `collection` 时提供已封存的 `request`，收集时还可提供按原单元绑定的 `migration` 原件。

成功响应包含两个端点的配置、登记文档、原始 schema、节点定义、现役测试别名和调度冲突，以及已修复的基线歧义。执行请求还携带输入环境的判定和请求身份。FILEMAP 负责读取原文、计算 DELTA、图闭包及最终影响。登记工具仅在需要额外读取时返回自己的 Git 观察。注册拒绝通过请求响应的 `error` 或普通端点命令的 stderr 返回，解码与进程失败退出 2。

工作流判官调用 `workflow-views-v1`，传入封存请求和两个固定 Git 提交的原始登记文档。登记判官返回该请求对应的配置、方法、收集操作、有效输入和退役声明。语言、方法执行、旧成本及交付凭证由相应使用方核验。传入 `replay: true` 时只重检原登记绑定。两种请求均返回请求 ID、提交对及实际产生的 Git 观察。

`test-retirements-v1` 供 routes 核对本次涉及的退役测试。stdin 中的 `judge` 指定查询方，`retired_tests` 列出待核对的原测试 ID。`endpoints.mode` 为 `documents` 时传入 `config` 和按路径索引的 `base`、`candidate`。为 `collection` 时从候选检出执行，提供封存的 `request`，历史转换按需使用绑定原单元的 `migration`。stdout 的 `retirements` 按原测试 ID 返回替代测试、共同退役（`Ok: null`）或原始错误。`downstream` 指明是否有登记的后续判官，`request_id` 在收集时绑定请求，在文档模式为 null。端点或传输失败退出 2，错误写入 stderr。

`execution-declarations-v1` 接受含 `filemap` 和 `projects` 的 JSON 对象。它验证完整的测试计划和各操作的限制，再输出 `plans` 映射及 `methods` 的判定。方法成功时 `Ok` 包含所有声明（含重名），失败时 `Err` 保留原始错误，供 routes 在执行单元归属检查后使用。计划不合法时退出 2 并把原因写入 stderr。

普通 routes 判官从原启动目录向 `route-views-v1` 提交原封存请求。登记判官按请求中的候选检出路径读取两个固定提交，解释历史转换，返回请求 ID、提交对及原 Git 观察。成功的 `result.Ok` 包含两端配置、FILEMAP、项目文档、登记视图及候选的计划、方法、测试操作、调度和可复用工具。登记拒绝保留在 `result.Err`。

routes 取得 FILEMAP 的退役测试后，可以用 `test-retirements-v1` 的 `view` 模式传入这个登记视图和配置路径。选定测试后向 `route-inputs-v1` 提供同一 `request`、`view` 和 `selected`，取得绑定请求 ID 的 `effective_inputs`。调用方负责原样传递已核验的视图。收集时按选中的执行单元核对保留输入，其他范围使用对应的原输入判法。请求、端点、登记摘要或上下文不符时拒绝。

`scoped-execution-v1` 读取一个固定端点的 `filemap`、`projects` 和 `artifacts`。FILEMAP 可只提供 `schema_version`、`execution_plans`、`execution_scheduling` 和 `reference_syntax`。命令校验项目、计划限额、调度声明与引用语法，成功后输出与 `scoped-test-bindings-v1` 相同的绑定表。v1 使用宿主声明的计划。无效登记写入 stderr 并退出 2。
