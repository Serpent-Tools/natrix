//! Construct CI report.

use std::pin::Pin;

use dagger_sdk::{ContainerWithExecOptsBuilder, PortForward, ReturnType, ServiceUpOpts};
use futures::{StreamExt, stream};
use itertools::Itertools;

use crate::TestCommand;
use crate::prelude::*;
use crate::targets::{IntegrationTestMode, TestResult};

/// Run all tests and return a directory of allure reports
pub async fn run_all_tests(client: &Query, arguments: &TestCommand) -> Result<Vec<TestResult>> {
    // Create all possible test futures with their names
    let all_tests = all_tests(client, arguments);

    let mut tasks: Vec<Pin<Box<dyn Future<Output = _>>>> = vec![];
    let mut need_sequential: Vec<Pin<Box<dyn Future<Output = _>>>> = vec![];

    // Filter tests based on arguments
    for (future, is_sequential) in all_tests {
        if is_sequential {
            need_sequential.push(future);
        } else {
            tasks.push(future);
        }
    }

    let mut result = Vec::new();
    let mut stream = stream::iter(tasks).buffer_unordered(arguments.jobs.unwrap_or(1));
    while let Some(task) = stream.next().await {
        result.extend(task?);
    }

    for task in need_sequential {
        result.extend(task.await?);
    }

    Ok(result)
}

/// Return a vector of all tests
fn all_tests<'q>(
    client: &'q Query,
    arguments: &'q TestCommand,
) -> Vec<(
    Pin<Box<dyn Future<Output = Result<Vec<TestResult>>> + 'q>>,
    bool,
)> {
    let all_tests: Vec<(Pin<Box<dyn Future<Output = _>>>, bool)> = vec![
        // (future, is_sequential)
        // (Box::pin(crate::targets::typos(client)), false),
        // (Box::pin(crate::targets::rustfmt(client)), false),
        (Box::pin(crate::targets::native_tests(client)), false),
        // (Box::pin(crate::targets::wasm_unit_tests(client)), false),
        // (Box::pin(crate::targets::clippy_workspace(client)), false),
        // (
        //     Box::pin(crate::targets::clippy_minimal_versions(client)),
        //     false,
        // ),
        // (Box::pin(crate::targets::clippy_natrix(client)), false),
        // (Box::pin(crate::targets::test_docs(client)), false),
        // (
        //     Box::pin(crate::targets::cargo_deny(client, "./crates/natrix")),
        //     false,
        // ),
        // (
        //     Box::pin(crate::targets::cargo_deny(client, "./crates/natrix-cli")),
        //     false,
        // ),
        // (Box::pin(crate::targets::unused_deps(client)), false),
        // (
        //     Box::pin(crate::targets::test_project_gen(client, "stable")),
        //     false,
        // ),
        // (
        //     Box::pin(crate::targets::test_project_gen(client, "nightly")),
        //     false,
        // ),
        // (Box::pin(crate::targets::test_book_links(client)), true),
        // (Box::pin(crate::targets::test_book_examples(client)), true),
        // (
        //     Box::pin(crate::targets::integration_test(
        //         client,
        //         IntegrationTestMode::Dev,
        //         arguments,
        //     )),
        //     false,
        // ),
        // (
        //     Box::pin(crate::targets::integration_test(
        //         client,
        //         IntegrationTestMode::Release,
        //         arguments,
        //     )),
        //     false,
        // ),
        // (
        //     Box::pin(crate::targets::integration_test(
        //         client,
        //         IntegrationTestMode::Build,
        //         arguments,
        //     )),
        //     false,
        // ),
    ];
    all_tests
}

/// Generate the final report
pub async fn generate_allure_report(client: &Query, results: &[TestResult]) -> Result<Directory> {
    let mut reports = client.directory();
    // HACK: Dagger doesnt like you creating 100+ files in one call.
    // So we chunk it up.
    for chunk in &results.iter().chunks(50) {
        let mut chunked = client.directory();
        for result in chunk {
            chunked = chunked.with_directory(".", result.as_file(client)?);
        }

        reports = reports.with_directory(".", chunked.sync().await?);
    }

    let result = client
        .container()
        .from("andgineer/allure")
        .with_directory("/reports", reports)
        .with_mounted_cache("/history-cache/", client.cache_volume("allure-history"))
        .with_exec_opts(
            vec!["mv", "-v", "/history-cache/history", "/reports/history"],
            ContainerWithExecOptsBuilder::default()
                .expect(ReturnType::Any)
                .build()?,
        )
        .with_exec(vec![
            "allure",
            "generate",
            "--name",
            "Natrix CI",
            "/reports",
        ])
        .with_exec(vec![
            "mv",
            "-v",
            "/allure-report/history",
            "/history-cache/history",
        ])
        .directory("/allure-report");

    Ok(result)
}

/// Serve the report
pub async fn serve_dist(client: &Query, dist: Directory) -> Result<()> {
    let container = client
        .container()
        .from("nginx:alpine")
        .with_directory("/usr/share/nginx/html", dist)
        .with_exposed_port(80);
    container.sync().await?;
    let service = container.as_service();

    let server = async move {
        let res = service
            .up_opts(ServiceUpOpts {
                ports: Some(vec![PortForward {
                    backend: 80,
                    frontend: 8000,
                    protocol: dagger_sdk::NetworkProtocol::Tcp,
                }]),
                random: None,
            })
            .await;
        if res.is_err() {
            service
                .up_opts(ServiceUpOpts {
                    ports: None,
                    random: Some(true),
                })
                .await?;
        }

        Ok::<(), eyre::Report>(())
    };

    let handle = tokio::spawn(server);
    println!("=========== RESULT GENERATED ============");
    println!("VISIT ABOVE LINK TO SEE");
    let _ = handle.await;

    Ok(())
}
