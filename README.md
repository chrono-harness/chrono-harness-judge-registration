# chrono-harness-judge-registration

`chrono-judge-registration input-profile-v1` 接收含 `config` 与 `inspect_execution_units` 的 JSON。返回 `credential_environment` 名称，以及需要检查时经核对的 `has_execution_units`。inputs 以此决定采集时省去哪些环境变量，以及组合快照是否需要按单元计算输入。

验证项目登记、执行方法与消费合同。

使用 `cargo build --locked --manifest-path crates/judge-registration/Cargo.toml` 构建。安装 Verus 0.2026.04.24.f8e1704，运行 `VERUS=/path/to/verus ./check.sh` 检查格式、构建、测试及证明。

登记判官的构建、测试和证明只读取本仓源码与锁定的第三方依赖。安装本仓发行后，`execution-methods` 和 `test-owners` 可直接处理 stdin。宿主执行完整登记检查时，通过 PATH 提供 mise 清单固定的 runner 与 routes。runner 通过 `record-command-v1` 返回解码器和分类选择器的原始进程记录，routes 通过 `check-unit-assignments-v1` 校验执行单元。启用滚动日志的宿主同时安装 diagnostics。

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

`configuration-edges-v1` 从标准输入读取固定端点的完整 `filemap` 和 `effective_config`。命令验证配置边登记，输出该配置需追加到 `project_edges` 的边数组。重复边或缺少必需的有效配置时退出 2。未登记配置边或选中其他配置时返回空数组。

`scoped-execution-v1` 读取一个固定端点的 `filemap`、`projects` 和 `artifacts`。FILEMAP 可只提供 `schema_version`、`execution_plans`、`execution_scheduling` 和 `reference_syntax`。命令校验项目、计划限额、调度声明与引用语法，成功后输出与 `scoped-test-bindings-v1` 相同的绑定表。v1 使用宿主声明的计划。无效登记写入 stderr 并退出 2。

`chrono-judge-registration classification-inputs-v1 SELECTOR` 从 stdin 接收封存的原生判官请求。它解释两个固定端点，将 DELTA、登记、前置 impact 和 costs 及其绑定传给调用者给出的绝对路径程序 `SELECTOR plan-classification-v1`。选择器返回两个端点所需的路径数组 `base`、`candidate` 和透传的 `context`。registration 只接受已声明且在对应 DELTA 端点存在的路径，优先使用解释后的登记文档，其余从对应提交读取。stdout 返回 runner judge-pipeline 的输入生产者响应，成功和业务失败均保留本次已执行的 Git 读取记录。

projects 使用 `project-views-v1` 解释固定端点。标准输入提供封存的 `request`、两端按路径索引的 UTF-8 原文 `originals` 和是否在当前检出核验输入的 `validate_inputs`。输出以请求 ID 和提交对绑定登记视图、实际配置路径、已选 FILEMAP、配对归属、测试操作及退役声明。历史转换产生的 Git 观察随结果返回。登记错误保留在 `error`。

`project-evidence-v1` 接收原 `request`、已解释的 `view` 和 `evidence`。`live` 模式在执行后复核现场输入，`retained` 从调用方提供的产物映射和可选运输位置读取原件，`unit` 核对原登记视图后将有效输入投影到单元。成功返回请求 ID 和有效输入，失败写入标准错误并退出 2。`check-project-registry-v1` 接收独立的项目登记文档，调用相同的结构校验，成功返回 JSON null。

CI 向 `check-documents-v1` 的 stdin 提交 JSON，`entry` 为登记入口，`documents` 按路径提供该 Git 端点的文档。程序核验所有登记后向 stdout 写入选中的有效配置路径（JSON 字符串），退出 0。输入或登记有误时向 stderr 写入原因并退出 2。

scoped CI 使用 `ci-transition-v1` 处理两个固定提交之间的登记变更。stdin 的 `config` 是登记入口，`base` 与 `candidate` 为完整提交 ID，`documents` 按基线、候选顺序提供以路径为键的登记文档，`retired` 仅列出本次选中的已移除测试。声明调度传入 `"mode":"schedule"`，现场执行传入 `"mode":{"execute":{"environment":{...}}}`。绑定 Git 的调用方先校验固定候选配置，再用 `git.config` 和不含进程列表的 `git.git_facts` 声明读取条件。历史文件按需读取，调度遇到必须执行的历史转换时拒绝。

stdout 返回 `previous`、`candidate`、本次额外产生的 `git_facts` 和 `result`。成功的 `Ok` 提供候选工具声明、转换及可复用工具，并逐项给出所选退役测试的替代关系、原定义及原成本。登记拒绝保留在 `Err`。两种业务结果都退出 0，输入解码或传输失败退出 2。

完整 CI 调度向 `ci-transition-v1` 提交 `"mode":"full-schedule"` 和空的 `retired`。程序返回两端按登记选定的配置、FILEMAP 和项目，以及候选测试的全部操作绑定。调度需要历史解码器时保留原来的拒绝结果。

完整 CI 收集向 `impact-views-v1` 提交 `operation.mode=ci-collection`、封存的 `request` 及两端保留确切 UTF-8 字节的 `originals`。响应以 `commits` 和 `request_id` 绑定请求。`outcome.ready` 提供相同的计划输入，`outcome.error` 保留登记错误。只有历史转换需要原报告时，`outcome.needed` 才列出单元。调用方取得这些原件后使用同一请求和端点文档，并在 `operation.migration` 中提供 `units` 与 `originals` 清单。收集按所选单元重新核验原报告与转换记录。

`scoped-startup-v1` 接受与 `scoped-execution-v1` 相同的单端点输入，供启用启动消费合同的 CI 使用。`bindings` 保留原绑定表，`startup` 中的 `methods`、`plans`、`owners` 分别给出方法的所有者、工具和参数，计划的操作，以及测试的所有者。三项各有独立的成功或错误结果，CI 在实际需要该项时使用。

仅当 scoped 调度声明变更时，CI 向 `scheduling-tests-v1` 提交两个端点。每端含 `scheduling` 和按测试 ID 给出的 `operations` 数组映射。全局调度参数变更时返回两端所有计划中的测试。局部资源或输出声明变更时返回直接受影响的测试，以及在任一端点上与变更操作争用资源或输出的测试。

处理封存请求的影响、项目和工作流查询时，registration 使用 PATH 中已安装的 runner 打开一次 `git-facts-v1` 会话。会话按请求核对 Git 绑定，历史解码读取两个固定提交的原字节并检查候选检出。调用者会收到实际 Git 进程记录。完整 CI 收集只在历史解码需要读 Git 时打开会话。

常规登记与首次登记通过已安装的 runner 读取实际检出根、固定提交的树和登记原件。常规登记核对基线到候选的 DELTA。首次登记核对候选没有父提交。两种检查均核对登记原字节与解码值，并将绑定的 Git 观察写回判官响应。路由、文件映射、分类和收集时的测试退役查询也通过此接口读取封存请求的端点。
