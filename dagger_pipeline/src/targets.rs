//! Actual targets

use std::collections::{HashMap, HashSet};

use dagger_sdk::Service;
use serde::{Deserialize, Serialize};

use crate::TestCommand;
use crate::prelude::*;

/// Unit Test suite name
const UNIT_TESTS: &str = "Unit Tests";
/// Linters test suite name
const LINTERS: &str = "Linters";
/// End to end tests suite name
const END_TO_END_TESTS: &str = "End To End Tests";

/// Test status
#[derive(Serialize)]
#[serde(rename_all = "lowercase")]
pub enum TestStatus {
    /// Test passed
    Passed,
    /// Test failed
    Failed,
    /// Test skipped
    Skipped,
}

/// Test event result types
#[derive(Deserialize, Debug)]
#[serde(rename_all = "lowercase")]
enum TestEventResult {
    /// Test passed
    Ok,
    /// Test failed
    Failed,
    /// Test ignored
    Ignored,
}

/// Test event types
#[derive(Deserialize, Debug)]
enum TestEvent {
    /// Test started (ignored in processing)
    #[serde(rename = "started")]
    Started,
    /// Test completed with result
    #[serde(untagged)]
    Result(TestEventResult),
}

/// Label types
#[derive(Serialize, Clone)]
#[serde(tag = "name", content = "value")]
enum LabelType {
    /// Suite label
    #[serde(rename = "suite")]
    Suite(String),
    /// Sub-suite label
    #[serde(rename = "subSuite")]
    SubSuite(String),
    /// The severity of the test
    #[serde(rename = "severity")]
    Severity(Severity),
}

/// The severity of the test
#[derive(Serialize, Clone)]
#[serde(rename_all = "lowercase")]
#[expect(
    dead_code,
    reason = "This represents the full range of valid values of the format."
)]
enum Severity {
    /// This is very simple to fix
    Trivial,
    /// This is just a minor issue
    Minor,
    /// This is specific feature not working
    Normal,
    /// This is a core component not working
    Critical,
    /// You cant commit without this
    Blocker,
}

/// Lib test output
#[derive(Deserialize, Debug)]
struct TestLine {
    /// Test event type
    event: TestEvent,
    /// Name
    name: String,
    /// The stdout
    stdout: Option<String>,
}

/// Status details for failed tests
#[derive(Serialize)]
pub struct StatusDetails {
    /// Error message
    pub message: String,
    /// Optional trace information
    #[serde(skip_serializing_if = "Option::is_none")]
    pub trace: Option<String>,
}

/// Test step
#[derive(Serialize)]
struct Step {
    /// Step name
    name: String,
    /// Step status
    status: TestStatus,
    /// Optional status details for failed steps
    #[serde(skip_serializing_if = "Option::is_none", rename = "statusDetails")]
    status_details: Option<StatusDetails>,
}

/// Test result for allure reporting
#[derive(Serialize)]
pub(crate) struct TestResult {
    /// Unique test identifier
    uuid: String,
    /// Test name
    pub name: String,
    /// Test status (passed/failed)
    pub status: TestStatus,
    /// Optional status details for failed tests
    #[serde(skip_serializing_if = "Option::is_none", rename = "statusDetails")]
    pub status_details: Option<StatusDetails>,
    /// Test labels
    labels: Vec<LabelType>,
    /// Optional test steps
    #[serde(skip_serializing_if = "Option::is_none")]
    steps: Option<Vec<Step>>,
}

impl TestResult {
    /// Convert test result to a directory containing the result file
    pub(crate) fn as_file(&self, client: &Query) -> Result<Directory> {
        let content = serde_json::to_string(self)?;
        let filename = format!("{}-result.json", self.uuid);
        Ok(client.directory().with_new_file(filename, content))
    }
}

/// Parse libtest JSON output and create test result directory
fn parse_libtest_json(output: &str, labels: &[LabelType], prefix: &str) -> Vec<TestResult> {
    // HACK: It seems that instead of reporting "ignored" events for skipped tests
    // nextest instead emits a "Started" event and then just never finish them.
    let mut not_finished = HashSet::new();

    let mut result = Vec::new();
    for line in output.lines() {
        if let Ok(json) = serde_json::from_str::<TestLine>(line) {
            let event_result = match json.event {
                TestEvent::Started => {
                    not_finished.insert(json.name);
                    continue;
                }
                TestEvent::Result(result) => result,
            };
            not_finished.remove(&json.name);

            let status_details = match event_result {
                TestEventResult::Ok => None,
                TestEventResult::Failed | TestEventResult::Ignored => Some(StatusDetails {
                    message: json.stdout.unwrap_or_default(),
                    trace: None,
                }),
            };

            let test_result = TestResult {
                uuid: uuid::Uuid::new_v4().to_string(),
                name: json
                    .name
                    .strip_prefix(prefix)
                    .unwrap_or(&json.name)
                    .to_string(),
                status: match event_result {
                    TestEventResult::Ok => TestStatus::Passed,
                    TestEventResult::Failed => TestStatus::Failed,
                    TestEventResult::Ignored => TestStatus::Skipped,
                },
                status_details,
                labels: labels.to_owned(),
                steps: None,
            };

            result.push(test_result);
        }
    }

    for not_finished in not_finished {
        let test_result = TestResult {
            uuid: uuid::Uuid::new_v4().to_string(),
            name: not_finished
                .strip_prefix(prefix)
                .unwrap_or(&not_finished)
                .to_string(),
            status: TestStatus::Skipped,
            labels: labels.to_owned(),
            status_details: None,
            steps: None,
        };

        result.push(test_result);
    }

    result
}

/// Generic linter configuration
struct LinterConfig {
    /// Name of the linter test
    name: String,
    /// Command to run
    command: Vec<&'static str>,
    /// Working directory to run in
    workdir: Option<&'static str>,
    /// Whether to use rust image or busybox
    use_rust_image: bool,
    /// Tools to install via binstall
    needs_binstall: Vec<&'static str>,
    /// Whether to capture stderr or stdout
    capture_stderr: bool,
}

impl Default for LinterConfig {
    fn default() -> Self {
        Self {
            name: String::new(),
            command: Vec::new(),
            workdir: None,
            use_rust_image: true,
            needs_binstall: Vec::new(),
            capture_stderr: true,
        }
    }
}

/// Run a generic linter and return test result directory
async fn run_linter(client: &Query, config: LinterConfig) -> Result<Vec<TestResult>> {
    let mut container = if config.use_rust_image {
        crate::base_images::rust(client)
    } else {
        crate::base_images::base(client)
    };

    for tool in &config.needs_binstall {
        container = container.with_directory("/bin", crate::base_images::binstall(client, tool));
    }

    container = container.with_workspace(client);

    if let Some(workdir) = config.workdir {
        container = container.with_workdir(workdir);
    }

    let result = container.with_exec_any(config.command)?;

    let exec_result = result.get_result().await?;
    let status = if exec_result.exit_code == 0 {
        None
    } else {
        let (output, other) = if config.capture_stderr {
            (exec_result.stderr, exec_result.stdout)
        } else {
            (exec_result.stdout, exec_result.stderr)
        };
        Some(StatusDetails {
            message: output,
            trace: Some(other),
        })
    };

    let test_result = TestResult {
        uuid: uuid::Uuid::new_v4().to_string(),
        name: config.name,
        status: if exec_result.exit_code == 0 {
            TestStatus::Passed
        } else {
            TestStatus::Failed
        },
        status_details: status,
        labels: vec![
            LabelType::Suite(LINTERS.to_string()),
            LabelType::Severity(Severity::Minor),
        ],
        steps: None,
    };

    Ok(vec![test_result])
}

/// Run the native nextest tests
pub async fn native_tests(client: &Query) -> Result<Vec<TestResult>> {
    let result = crate::base_images::rust(client)
        .with_directory(
            "/bin",
            crate::base_images::binstall(client, "cargo-nextest"),
        )
        .with_workspace(client)
        .with_workdir("./crates/natrix")
        .with_env_variable("NEXTEST_EXPERIMENTAL_LIBTEST_JSON", "1")
        .with_exec_any(vec![
            "cargo",
            "nextest",
            "run",
            "--color",
            "never",
            "--all-features",
            "--offline",
            "--no-fail-fast",
            "--message-format",
            "libtest-json",
            "--message-format-version",
            "0.1",
        ])?
        .stdout()
        .await?;

    let result = parse_libtest_json(
        &result,
        &[
            LabelType::Suite(UNIT_TESTS.to_string()),
            LabelType::SubSuite("Native".to_string()),
            LabelType::Severity(Severity::Normal),
        ],
        "natrix::natrix$",
    );
    Ok(result)
}

/// Wasm unit tests
pub async fn wasm_unit_tests(client: &Query) -> Result<Vec<TestResult>> {
    let output = crate::base_images::wasm(client)
        .with_workspace(client)
        .with_workdir("./crates/natrix")
        .with_exec_any(vec![
            "cargo",
            "test",
            "--tests",
            "--target",
            "wasm32-unknown-unknown",
            "--features",
            "_internal_testing",
            "--all-features",
        ])?
        .stdout()
        .await?;

    Ok(parse_wasmbindgen_test_output(&output))
}

/// Extract the result of a `wasm-pack test` run
fn parse_wasmbindgen_test_output(output: &str) -> Vec<TestResult> {
    let mut traces = HashMap::new();

    let mut hit_first = false;
    let mut current_name = String::new();
    let mut current_output = String::new();

    for line in output.lines() {
        if line.starts_with("----") {
            let parts = line.split_whitespace().collect::<Vec<_>>();
            if let [_, name, _, _] = parts[..] {
                if hit_first {
                    let mut new_name = name.to_string();
                    std::mem::swap(&mut new_name, &mut current_name);
                    traces.insert(new_name, std::mem::take(&mut current_output));
                } else {
                    current_name = name.to_string();
                    hit_first = true;
                }
            }
        }

        if line.starts_with("test result") {
            traces.insert(current_name, current_output);
            break;
        }

        if hit_first {
            current_output.push_str(line);
            current_output.push('\n');
        }
    }

    let mut test_results = Vec::new();
    for line in output.lines() {
        if line.starts_with("test ") {
            let parts = line.split_whitespace().collect::<Vec<_>>();
            if let Some(["test", name, "...", result]) = &parts.get(..4) {
                // When a test is ignored it has extra stuff at the end
                // If not its always 4 parts.
                // This check prevents false positives.
                if !result.starts_with("ignored") && parts.len() != 4 {
                    continue;
                }

                let id = uuid::Uuid::new_v4().to_string();

                let status_details = if *result == "ok" {
                    None
                } else {
                    traces.get(*name).map(|output| StatusDetails {
                        message: output.clone(),
                        trace: None,
                    })
                };

                let test_result = TestResult {
                    uuid: id,
                    name: name
                        .strip_prefix("actual_tests::")
                        .unwrap_or(name)
                        .to_string(),
                    status: if *result == "ok" {
                        TestStatus::Passed
                    } else if result.starts_with("ignored") {
                        TestStatus::Skipped
                    } else {
                        TestStatus::Failed
                    },
                    status_details,
                    labels: vec![
                        LabelType::Suite(UNIT_TESTS.to_string()),
                        LabelType::SubSuite("Web".to_string()),
                        LabelType::Severity(Severity::Normal),
                    ],
                    steps: None,
                };

                test_results.push(test_result);
            }
        }
    }

    let all_output = TestResult {
        uuid: uuid::Uuid::new_v4().to_string(),
        name: String::from("[[ALL OUTPUT]]"),
        status: TestStatus::Passed,
        status_details: Some(StatusDetails {
            message: output.into(),
            trace: None,
        }),
        labels: vec![
            LabelType::Suite(UNIT_TESTS.to_string()),
            LabelType::SubSuite("Web".to_string()),
            LabelType::Severity(Severity::Trivial),
        ],
        steps: None,
    };
    test_results.push(all_output);

    test_results
}

/// Run rustfmt on the workspace
pub async fn rustfmt(client: &Query) -> Result<Vec<TestResult>> {
    run_linter(
        client,
        LinterConfig {
            name: "Rust-fmt".to_string(),
            command: vec!["cargo", "+nightly", "fmt", "--check"],
            capture_stderr: false,
            ..Default::default()
        },
    )
    .await
}

/// Run typos on the workspace
pub async fn typos(client: &Query) -> Result<Vec<TestResult>> {
    run_linter(
        client,
        LinterConfig {
            name: "Spell Checking".to_string(),
            command: vec!["typos", "--color", "never", "--format", "brief"],
            use_rust_image: false,
            needs_binstall: vec!["typos-cli"],
            capture_stderr: false,
            ..Default::default()
        },
    )
    .await
}

/// Run clippy on the workspace
pub async fn clippy_workspace(client: &Query) -> Result<Vec<TestResult>> {
    run_linter(
        client,
        LinterConfig {
            name: "Clippy All".to_string(),
            command: vec![
                "cargo",
                "+nightly",
                "clippy",
                "--all-features",
                "--tests",
                "--",
                "-Dwarnings",
            ],
            ..Default::default()
        },
    )
    .await
}

/// Run clippy on the workspace with minimal version
pub async fn clippy_minimal_versions(client: &Query) -> Result<Vec<TestResult>> {
    run_linter(
        client,
        LinterConfig {
            name: "Clippy Minimal".to_string(),
            command: vec![
                "cargo",
                "+nightly",
                "minimal-versions",
                "--direct",
                "clippy",
                "--all-features",
                "--tests",
                "--",
                "-Dwarnings",
            ],
            needs_binstall: vec!["cargo-minimal-versions", "cargo-hack"],
            ..Default::default()
        },
    )
    .await
}

/// Run clippy hack on natrix
pub async fn clippy_natrix(client: &Query) -> Result<Vec<TestResult>> {
    run_linter(
        client,
        LinterConfig {
            name: "Clippy hack Natrix".to_string(),
            command: vec![
                "cargo",
                "hack",
                "clippy",
                "--each-feature",
                "--tests",
                "--",
                "-Dwarnings",
            ],
            workdir: Some("./crates/natrix"),
            needs_binstall: vec!["cargo-hack"],
            ..Default::default()
        },
    )
    .await
}

/// Run `cargo-deny` on the given folder
pub async fn cargo_deny(client: &Query, folder: &'static str) -> Result<Vec<TestResult>> {
    run_linter(
        client,
        LinterConfig {
            name: format!("Cargo deny ({folder})"),
            command: vec!["cargo", "deny", "check", "--exclude-dev"],
            workdir: Some(folder),
            needs_binstall: vec!["cargo-deny"],
            ..Default::default()
        },
    )
    .await
}

/// Run `cargo-udeps` on the given folder
pub async fn unused_deps(client: &Query) -> Result<Vec<TestResult>> {
    run_linter(
        client,
        LinterConfig {
            name: "Unused dependencies".to_string(),
            needs_binstall: vec!["cargo-udeps", "cargo-hack"],
            command: vec![
                "cargo",
                "+nightly",
                "hack",
                "udeps",
                "--each-feature",
                "--all-targets",
            ],
            capture_stderr: false,
            ..Default::default()
        },
    )
    .await
}

/// Build the cli
pub fn cli(client: &Query) -> Directory {
    let file = crate::base_images::rust(client)
        .with_workspace(client)
        .with_exec(vec![
            "cargo",
            "build",
            "--profile",
            "dev",
            "-p",
            "natrix-cli",
        ])
        .with_exec(vec!["mv", "./target/debug/natrix", "/tmp/natrix"])
        .file("/tmp/natrix");
    client.directory().with_file("natrix", file)
}

/// Test that the natrix project generator works
pub async fn test_project_gen(client: &Query, toolchain: &'static str) -> Result<Vec<TestResult>> {
    let container = crate::base_images::wasm(client)
        .with_exec(vec!["rustup", "default", toolchain])
        .with_directory("/bin", cli(client))
        .with_env_variable("NATRIX_PATH", "/app/crates/natrix")
        .with_exec_any(vec![
            "natrix",
            "new",
            "project",
            if toolchain == "stable" {
                "--stable"
            } else {
                "--"
            },
        ])?
        .with_workspace(client)
        .with_workdir("/project")
        .with_mounted_cache(
            "/project/target",
            client.cache_volume(format!("project-gen-test-target-{toolchain}")),
        );

    let mut success = true;
    let mut steps = Vec::with_capacity(3);

    let gen_result = container.get_result().await?;
    if gen_result.exit_code == 0 {
        steps.push(Step {
            name: "Generate project".to_string(),
            status: TestStatus::Passed,
            status_details: None,
        });

        let build = container.with_exec_any(vec!["natrix", "build"])?;
        let test = container.with_exec_any(vec!["wasm-pack", "test", "--headless", "--chrome"])?;

        let (build_result, test_result) = tokio::try_join!(build.get_result(), test.get_result())?;

        if build_result.exit_code == 0 {
            steps.push(Step {
                name: "Build project".to_string(),
                status: TestStatus::Passed,
                status_details: None,
            });
        } else {
            success = false;
            steps.push(Step {
                name: "Build project".to_string(),
                status: TestStatus::Failed,
                status_details: Some(StatusDetails {
                    message: build_result.stderr,
                    trace: Some(build_result.stdout),
                }),
            });
        }
        if test_result.exit_code == 0 {
            steps.push(Step {
                name: "Test project".to_string(),
                status: TestStatus::Passed,
                status_details: None,
            });
        } else {
            success = false;
            steps.push(Step {
                name: "Test project".to_string(),
                status: TestStatus::Failed,
                status_details: Some(StatusDetails {
                    message: test_result.stderr,
                    trace: Some(test_result.stdout),
                }),
            });
        }
    } else {
        success = false;
        steps.push(Step {
            name: "Generate project".to_string(),
            status: TestStatus::Failed,
            status_details: Some(StatusDetails {
                message: gen_result.stderr,
                trace: Some(gen_result.stdout),
            }),
        });
    }

    let test_result = TestResult {
        uuid: uuid::Uuid::new_v4().to_string(),
        name: format!("Project gen ({toolchain})"),
        status: if success {
            TestStatus::Passed
        } else {
            TestStatus::Failed
        },
        status_details: None,
        labels: vec![
            LabelType::Suite(END_TO_END_TESTS.to_string()),
            LabelType::Severity(Severity::Critical),
        ],
        steps: Some(steps),
    };

    Ok(vec![test_result])
}

/// Run the cargo doc tests
pub async fn test_docs(client: &Query) -> Result<Vec<TestResult>> {
    let output = crate::base_images::rust(client)
        .with_workspace(client)
        .with_exec_any(vec![
            "cargo",
            "+nightly",
            "test",
            "--all-features",
            "--doc",
            "--no-fail-fast",
            "--",
            "-Z",
            "unstable-options",
            "--format",
            "json",
        ])?
        .stdout()
        .await?;

    Ok(parse_libtest_json(
        &output,
        &[
            LabelType::Suite(UNIT_TESTS.to_string()),
            LabelType::SubSuite("Documentation".to_string()),
            LabelType::Severity(Severity::Minor),
        ],
        "",
    ))
}

/// Test the links in the mdbooks
/// INVARIANT: Should not run in parallel as contains timeouts
pub async fn test_book_links(client: &Query) -> Result<Vec<TestResult>> {
    let result = crate::base_images::book(client)
        .with_exec_any(vec!["mdbook", "build"])?
        .get_result()
        .await?;

    let status = if result.exit_code == 0 {
        None
    } else {
        Some(StatusDetails {
            message: result.stderr,
            trace: None,
        })
    };

    let result = TestResult {
        uuid: uuid::Uuid::new_v4().to_string(),
        status: if result.exit_code == 0 {
            TestStatus::Passed
        } else {
            TestStatus::Failed
        },
        name: "Book Links".to_string(),
        status_details: status,
        labels: vec![LabelType::Suite(LINTERS.to_string())],
        steps: None,
    };
    Ok(vec![result])
}

/// Run the example tests in the book
/// INVARIANT: This test is extremely flaky in terms of cache state.
/// So should not be run in parallel.
/// INVARIANT: `test_book_links` needs to have been run before hand to prime cache.
pub async fn test_book_examples(client: &Query) -> Result<Vec<TestResult>> {
    let result = crate::base_images::book(client)
        .with_exec_any(vec!["sh", "-c", "rm -r ../target/debug/deps/*natrix*"])?
        .with_exec_any(vec![
            "sh",
            "-c",
            "rm -r ../target/debug/deps/*wasm_bindgen_test*",
        ])?
        .with_exec(vec![
            "cargo",
            "build",
            "-p",
            "natrix",
            "--all-features",
            "--tests",
        ])
        .with_exec_any(vec!["mdbook", "test", "-L", "../target/debug/deps"])?
        .get_result()
        .await?;

    let status = if result.exit_code == 0 {
        None
    } else {
        Some(StatusDetails {
            message: result.stderr,
            trace: None,
        })
    };

    let result = TestResult {
        uuid: uuid::Uuid::new_v4().to_string(),
        status: if result.exit_code == 0 {
            TestStatus::Passed
        } else {
            TestStatus::Failed
        },
        name: "Book Examples".to_string(),
        status_details: status,
        labels: vec![
            LabelType::Suite(UNIT_TESTS.to_string()),
            LabelType::Severity(Severity::Normal),
        ],
        steps: None,
    };
    Ok(vec![result])
}

/// A service that spins up `natrix dev` in the integration tests directory.
/// The server runs on port 3000
fn natrix_dev(client: &Query, profile: &str) -> Service {
    crate::base_images::wasm(client)
        .with_directory("/bin", cli(client))
        .with_workspace(client)
        .with_workdir("./ci/integration_tests")
        .with_entrypoint(vec![
            "natrix",
            "dev",
            "--profile",
            profile,
            "--port",
            "3000",
            "--allow-external",
            "--no-reload",
        ])
        .with_exposed_port(3000)
        .as_service()
}

/// A service running a chromedriver, with the given serve mounted under the hostname `page`
/// The chromedriver runs on port 8000
fn chrome_driver_with_page(client: &Query, page: Service) -> Service {
    crate::base_images::wasm(client)
        .with_service_binding("page.local", page)
        .with_entrypoint(vec![
            "chromedriver",
            "--port=8000",
            "--allowed-ips=",
            "--allowed-origins=*",
            "--remote-debugging-pipe",
            "--user-data-dir=/tmp/chrome",
        ])
        .with_exposed_port(8000)
        .as_service()
}

/// Returns a nginx server with a natrix build
fn natrix_build(client: &Query) -> Service {
    let page = crate::base_images::wasm(client)
        .with_directory("/bin", cli(client))
        .with_workspace(client)
        .with_workdir("./ci/integration_tests")
        .with_exec(vec!["natrix", "build"])
        .directory("./dist");

    client
        .container()
        .from("nginx:alpine")
        .with_directory("/usr/share/nginx/html/dist", page)
        .with_exposed_port(80)
        .as_service()
}

/// The integration test mode
#[derive(Debug, Copy, Clone, PartialEq, Eq)]
pub enum IntegrationTestMode {
    /// `natrix dev`
    Dev,
    /// `natrix dev --release`
    Release,
    /// `natrix build`
    Build,
}

/// Run the integration tests against `natrix dev`
/// INVARIANT: This should not run in parallel as it has timeouts that could be affected by CPU
/// churn
pub async fn integration_test(
    client: &Query,
    mode: IntegrationTestMode,
    arguments: &TestCommand,
) -> Result<Vec<TestResult>> {
    let page = match mode {
        IntegrationTestMode::Dev => natrix_dev(client, "dev"),
        IntegrationTestMode::Release => natrix_dev(client, "release"),
        IntegrationTestMode::Build => natrix_build(client),
    };
    let chrome = chrome_driver_with_page(client, page.clone());

    let mut command = vec![
        "cargo".to_string(),
        "nextest".to_string(),
        "run".to_string(),
        "-j".to_string(),
        arguments.jobs.unwrap_or(1).to_string(),
        "--no-fail-fast".to_string(),
        "--message-format".to_string(),
        "libtest-json".to_string(),
        "--message-format-version".to_string(),
        "0.1".to_string(),
    ];
    if mode == IntegrationTestMode::Build {
        command.extend(["--features".to_string(), "build_test".to_string()]);
    }

    let output = crate::base_images::rust(client)
        .with_service_binding("chrome.local", chrome.clone())
        .with_directory(
            "/bin",
            crate::base_images::binstall(client, "cargo-nextest"),
        )
        .with_workspace(client)
        .with_workdir("./ci/integration_tests")
        .with_env_variable("NEXTEST_EXPERIMENTAL_LIBTEST_JSON", "1")
        .with_exec_any(command)?
        .stdout()
        .await?;

    chrome.stop().await?;
    page.stop().await?;

    Ok(parse_libtest_json(
        &output,
        &[
            LabelType::Suite(END_TO_END_TESTS.to_string()),
            LabelType::SubSuite(format!("Integration {mode:?}")),
            LabelType::Severity(Severity::Critical),
        ],
        "integration_tests::integration_tests$",
    ))
}

/// Run benchmarks
pub async fn benchmark(client: &Query) -> Result<String> {
    let result = crate::base_images::wasm(client)
        .with_exec(vec!["rustup", "default", "nightly"])
        .with_workspace(client)
        .with_workdir("./ci/benchmark")
        .with_env_variable("WASM_BINDGEN_TEST_TIMEOUT", "120")
        .with_exec(vec![
            "cargo",
            "build",
            "--tests",
            "--release",
            "--target",
            "wasm32-unknown-unknown",
        ])
        .with_exec_any(vec![
            "cargo",
            "test",
            "--release",
            "--target",
            "wasm32-unknown-unknown",
        ])?
        .get_result()
        .await?;

    Ok(result
        .stdout
        .lines()
        .map(str::trim)
        .skip_while(|line| !line.starts_with("---NATRIX_BENCHMARK_START"))
        .skip(1)
        .take_while(|line| !line.starts_with("---NATRIX_BENCHMARK_END"))
        .collect::<Vec<_>>()
        .join("\n"))
}
