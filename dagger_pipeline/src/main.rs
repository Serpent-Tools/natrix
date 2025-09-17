//! Run the CI pipeline using dagger

mod base_images;
mod fix;
mod report;
mod targets;

use clap::Parser;
use eyre::eyre;
use prelude::*;

use crate::targets::TestStatus;

/// Generate a test report, or simply run the tests
#[derive(clap::Parser)]
struct TestCommand {
    /// How many jobs to run in parallel.
    /// If not specified defaults to 1
    #[arg(short, long)]
    jobs: Option<usize>,
    /// Output to a tui instead
    #[arg(short, long)]
    tui: bool,
    /// Generate the allure report but just export it to the host.
    #[arg(long)]
    export: bool,
}

/// Run a dagger pipeline too generate test reports.
#[derive(clap::Parser)]
enum Cli {
    /// Generate a test report, or simply run the tests
    Tests(TestCommand),
    /// Apply various fixes
    Fix,
    /// Build and open the mdbook
    Book,
    /// Run benchmark
    Bench,
}

/// Common items
mod prelude {
    use dagger_sdk::HostDirectoryOpts;
    pub use dagger_sdk::{Container, ContainerWithExecOptsBuilder, Directory, Query, ReturnType};
    pub use eyre::Result;

    /// Result of executing a command in a container
    #[derive(Debug)]
    pub struct ExecutionResult {
        /// Exit code of the command
        pub exit_code: isize,
        /// Standard output of the command
        pub stdout: String,
        /// Standard error of the command
        pub stderr: String,
    }

    /// Extension trait for dagger containers
    pub trait ContainerExtension {
        /// Copy in the workspace and setup target cache
        fn with_workspace(&self, client: &Query) -> Container;

        /// Execute a command with default options (`ReturnType::Any`)
        fn with_exec_any(&self, args: Vec<impl Into<String>>) -> Result<Container>;

        /// Get the full execution result (exit code, stdout, stderr) from the current container
        async fn get_result(&self) -> Result<ExecutionResult>;
    }

    impl ContainerExtension for Container {
        fn with_workspace(&self, client: &Query) -> Container {
            let workspace = client.host().directory_opts(
                ".",
                HostDirectoryOpts {
                    exclude: Some(vec!["target", "*/dist", "docs/book", ".jj"]),
                    include: None,
                    no_cache: None,
                },
            );
            self.with_mounted_directory("/app", workspace)
                .with_workdir("/app")
                .with_mounted_cache("/app/target", client.cache_volume("rust-target"))
        }

        fn with_exec_any(&self, args: Vec<impl Into<String>>) -> Result<Container> {
            Ok(self.with_exec_opts(
                args,
                ContainerWithExecOptsBuilder::default()
                    .expect(ReturnType::Any)
                    .build()?,
            ))
        }

        async fn get_result(&self) -> Result<ExecutionResult> {
            let (exit_code, stdout, stderr) =
                tokio::try_join!(self.exit_code(), self.stdout(), self.stderr(),)?;

            Ok(ExecutionResult {
                exit_code,
                stdout,
                stderr,
            })
        }
    }
}

#[tokio::main]
async fn main() -> Result<()> {
    let arguments = Cli::parse();

    dagger_sdk::connect(async move |client| {
        match arguments {
            Cli::Tests(arguments) => {
                if cfg!(feature = "ci") {
                    base_images::base(&client)
                        .with_workspace(&client)
                        .with_mounted_directory("/load_cache", client.host().directory("./cache"))
                        .with_exec(vec![
                            "sh",
                            "-c",
                            "mv /load_cache/target/* /app/target/* || true",
                        ])
                        .sync()
                        .await?;
                }

                let reports = report::run_all_tests(&client, &arguments).await?;

                if cfg!(feature = "ci") {
                    base_images::base(&client)
                        .with_workspace(&client)
                        .with_exec(vec!["mkdir", "/store_cache"])
                        .with_exec(vec!["sh", "-c", "cp -r /app/target /store_cache"])
                        .directory("/store_cache")
                        .export("./cache")
                        .await?;
                }

                if arguments.tui {
                    let total = reports.len();
                    let mut pass = 0u16;
                    let mut skip = 0u16;
                    let mut fail = 0u16;

                    for report in reports {
                        match report.status {
                            TestStatus::Passed => {
                                pass = pass.saturating_add(1);
                            }
                            TestStatus::Skipped => {
                                skip = skip.saturating_add(1);
                            }
                            TestStatus::Failed => {
                                fail = fail.saturating_add(1);
                                println!("FAIL {}", report.name);

                                if let Some(message) = report.status_details {
                                    println!("{}", message.message);
                                    println!("{}", message.trace.unwrap_or_default());
                                }
                            }
                        }
                    }

                    println!("RESULTS: {pass}/{total} ({fail} failed, {skip} skipped)");

                    if fail != 0 {
                        return Err(eyre!("Tests failed"));
                    }
                } else {
                    let report = report::generate_allure_report(&client, &reports).await?;

                    if arguments.export {
                        report.export("./allure_report/").await?;
                        let tests_failed = reports
                            .into_iter()
                            .any(|report| matches!(report.status, TestStatus::Failed));
                        if tests_failed {
                            return Err(eyre!("Tests failed"));
                        }
                    } else {
                        report::serve_dist(&client, report).await?;
                    }
                }
            }
            Cli::Fix => {
                let source = client.host().directory_opts(
                    ".",
                    dagger_sdk::HostDirectoryOpts {
                        exclude: Some(vec!["target", "*/dist", "docs/book", ".jj"]),
                        include: None,
                        no_cache: None,
                    },
                );
                let source = fix::typos(&client, source)?;
                let source = fix::fmt(&client, source);
                let source = fix::snapshots(&client, source);
                source.export(".").await?;
            }
            Cli::Book => {
                let book = base_images::book(&client)
                    .with_workspace(&client)
                    .with_workdir("./docs")
                    .with_exec(vec!["mdbook", "build"])
                    .directory("./book");
                report::serve_dist(&client, book).await?;
            }
            Cli::Bench => {
                let result = targets::benchmark(&client).await?;
                print!("{result}");
            }
        }
        Ok(())
    })
    .await
    .map_err(Into::into)
}
