use methodfig::tools::render::scan_generated_typescript;

#[test]
fn safety_scan_accepts_local_renderer_import() {
    let code = r#"
        import { createFigureRuntime } from "/tmp/methodfig/renderer/src/runtime";
        const rt = createFigureRuntime({ outDir: "/tmp/run", style: {}, canvas: {} });
        await rt.write();
    "#;
    scan_generated_typescript(code).expect("local renderer import should be allowed");
}

#[test]
fn safety_scan_rejects_network_child_process_and_env_access() {
    for code in [
        r#"import cp from "child_process";"#,
        r#"import http from "http";"#,
        r#"const secret = process.env.METHODFIG_REASONER_API_KEY;"#,
        r#"await import("net");"#,
    ] {
        let err = scan_generated_typescript(code).expect_err("unsafe code should fail");
        assert!(err.to_string().contains("unsafe generated TypeScript"));
    }
}
