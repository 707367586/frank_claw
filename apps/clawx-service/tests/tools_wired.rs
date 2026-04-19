// Smoke test: service boots with tools registered by default.
#[tokio::test]
async fn service_runtime_has_tools_registered() {
    let runtime = clawx_service::build_runtime_for_tests().await.unwrap();
    assert!(runtime.tools.is_some(), "tools must be wired by default");
    assert!(runtime.approval.is_some(), "approval gate must be wired");
    let reg = runtime.tools.as_ref().unwrap();
    let names: Vec<_> = reg.definitions().into_iter().map(|d| d.name).collect();
    for expected in ["fs_read", "fs_write", "fs_mkdir", "fs_list"] {
        assert!(
            names.iter().any(|n| n == expected),
            "missing tool: {}",
            expected
        );
    }
}
