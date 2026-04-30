use clap::Parser;

mod ci;

#[derive(Parser)]
enum Task {
    /// Format all code
    Fmt,
    /// Run clippy with deny-warnings
    Lint,
    /// Run all tests
    Test,
    /// Run e2e tests
    E2e,
    /// Full CI pipeline: fmt → lint → test
    Ci,
}

fn main() -> anyhow::Result<()> {
    match Task::parse() {
        Task::Fmt => ci::fmt(),
        Task::Lint => ci::lint(),
        Task::Test => ci::test(),
        Task::E2e => ci::e2e(),
        Task::Ci => ci::ci(),
    }
}
