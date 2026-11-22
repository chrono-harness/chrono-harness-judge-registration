# chrono-harness-judge-registration

验证项目登记、执行方法与消费合同。

使用 `cargo build --locked --manifest-path crates/judge-registration/Cargo.toml` 构建。安装 Verus 0.2026.04.24.f8e1704，运行 `VERUS=/path/to/verus ./check.sh` 检查格式、构建、测试及证明。

源码来自 [固定版本的原实现](https://github.com/ChronoAIProject/chrono-harness/tree/5f6892dfa2f73675a84e4ebdc3b06eb83b824d6e/crates/judge-registration)。

`chrono-judge-registration execution-methods` 从标准输入读取一份项目和脚本登记 JSON，沿用判官的 64 MiB 输入上限。成功时输出从操作 ID 到全部方法的 JSON 映射，包含 owner、operation、tool、argv 和已登记的 output_contract。相同操作 ID 的不同声明分别保留，供发布配方检查歧义。成功退出 0，读取、解析、登记或写出失败退出 2，错误发往标准错误。

`chrono-judge-registration test-owners` 接受同一输入，按测试绑定规则验证登记，输出每个所有者对应的测试 ID 列表，包括显式测试组。宿主的发布预览读取这个映射以选择受影响单元。两个查询只读标准输入。

`chrono-judge-registration filemap-views` 接受一个封存的 chrono-judge/v1 请求。在两个固定端点上按判官协议核对登记，保留历史转换和已有视图的绑定检查。成功时输出 request_id、base、candidate，每端包含 FILEMAP 登记路径、解释后的文档和节点 ID。登记拒绝时输出相同 request_id 和原始 error。启用 Git 观察时两种结果都带 git_facts。读取、解码或写出失败时退出 2。成本判官据此读取两端的成本声明。
