pub fn isolated_git() -> bool {
    chrono_test_process::isolated_git_with_observation(
        std::path::Path::new(env!("CARGO_BIN_EXE_chrono-test-git-model")),
        None,
        |_| Ok(false),
        false,
    )
}
