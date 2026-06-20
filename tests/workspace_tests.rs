use methodfig::tools::workspace::{
    AgentWorkspace, WorkspaceFile, WorkspaceFileFormat, WorkspaceManifest,
};

#[test]
fn workspace_manifest_rejects_env_parent_and_absolute_paths() {
    for bad_path in [
        ".env",
        "readable/.env",
        "../goal.md",
        "readable/../../.env",
        "/tmp/methodfig/secret.json",
        "writable/../figure.ts",
    ] {
        let manifest = WorkspaceManifest {
            readable: vec![entry(bad_path)],
            writable: vec![],
        };

        let error = manifest
            .validate()
            .unwrap_err_or_else(|| panic!("manifest path {bad_path} should be rejected"));
        assert!(
            error.to_string().contains("unsafe workspace path"),
            "{error}"
        );
    }
}

#[test]
fn workspace_writes_only_declared_writable_files() {
    let temp = tempfile::tempdir().expect("tempdir");
    let manifest = WorkspaceManifest {
        readable: vec![entry("readable/input.md")],
        writable: vec![entry("writable/draw_plan.json")],
    };
    let workspace = AgentWorkspace::create(temp.path(), manifest).expect("workspace should create");

    workspace
        .write_declared("writable/draw_plan.json", br#"{"version":"0.2"}"#)
        .expect("declared writable file should be accepted");

    assert!(temp
        .path()
        .join("workspace/writable/draw_plan.json")
        .exists());
    let error = workspace
        .write_declared("writable/figure.ts", b"unsafe")
        .expect_err("undeclared writable file should be rejected");
    assert!(error.to_string().contains("not declared writable"));
}

#[test]
fn workspace_allows_declared_generated_code_files() {
    let temp = tempfile::tempdir().expect("tempdir");
    let manifest = WorkspaceManifest {
        readable: vec![entry("readable/input.md")],
        writable: vec![
            WorkspaceFile {
                path: "writable/code/figure.ts".to_string(),
                purpose: "generated renderer entrypoint".to_string(),
                format: WorkspaceFileFormat::Typescript,
                max_bytes: 256_000,
            },
            WorkspaceFile {
                path: "writable/code/helpers.ts".to_string(),
                purpose: "generated renderer helper module".to_string(),
                format: WorkspaceFileFormat::Typescript,
                max_bytes: 256_000,
            },
        ],
    };
    let workspace = AgentWorkspace::create(temp.path(), manifest).expect("workspace should create");

    workspace
        .write_declared("writable/code/figure.ts", b"export const figure = 1;\n")
        .expect("declared figure.ts should be writable");
    workspace
        .write_declared("writable/code/helpers.ts", b"export const helper = 1;\n")
        .expect("declared helpers.ts should be writable");

    assert!(temp
        .path()
        .join("workspace/writable/code/figure.ts")
        .exists());
    assert!(temp
        .path()
        .join("workspace/writable/code/helpers.ts")
        .exists());
}

#[test]
fn workspace_manifest_is_written_inside_workspace() {
    let temp = tempfile::tempdir().expect("tempdir");
    let manifest = WorkspaceManifest {
        readable: vec![entry("readable/input.md")],
        writable: vec![entry("writable/design_brief.md")],
    };

    AgentWorkspace::create(temp.path(), manifest).expect("workspace should create");

    let manifest_path = temp.path().join("workspace/manifest.json");
    assert!(manifest_path.exists());
    let manifest_json: serde_json::Value =
        serde_json::from_slice(&std::fs::read(manifest_path).expect("manifest should exist"))
            .expect("manifest should parse");
    assert_eq!(
        manifest_json["writable"][0]["path"],
        "writable/design_brief.md"
    );
}

fn entry(path: &str) -> WorkspaceFile {
    WorkspaceFile {
        path: path.to_string(),
        purpose: "test fixture".to_string(),
        format: WorkspaceFileFormat::Json,
        max_bytes: 4096,
    }
}

trait ExpectNone<T, E> {
    fn unwrap_err_or_else(self, fallback: impl FnOnce() -> E) -> E;
}

impl<T, E> ExpectNone<T, E> for Result<T, E> {
    fn unwrap_err_or_else(self, fallback: impl FnOnce() -> E) -> E {
        match self {
            Ok(_) => fallback(),
            Err(error) => error,
        }
    }
}
