pub fn isolated_git() -> bool {
    chrono_test_process::isolated_git_with_observation(
        std::path::Path::new(env!("CHRONO_TEST_GIT")),
        None,
        |_| Ok(false),
        false,
    )
}
