use anyhow::{bail, Result};
use xshell::{cmd, Shell};

fn sh() -> Result<Shell> {
    let sh = Shell::new()?;
    sh.change_dir(env!("CARGO_MANIFEST_DIR").to_string() + "/..");
    Ok(sh)
}

pub fn fmt() -> Result<()> {
    let sh = sh()?;
    cmd!(sh, "cargo fmt --all --check").run()?;
    Ok(())
}

pub fn lint() -> Result<()> {
    let sh = sh()?;
    cmd!(sh, "cargo clippy --all-targets --all-features -- -D warnings").run()?;
    Ok(())
}

pub fn test() -> Result<()> {
    let sh = sh()?;
    cmd!(sh, "cargo test --all-features").run()?;
    Ok(())
}

pub fn audit() -> Result<()> {
    let sh = sh()?;
    cmd!(sh, "cargo audit").run()?;
    Ok(())
}

pub fn e2e() -> Result<()> {
    let sh = sh()?;
    if std::env::var("DEEPSEEK_API_KEY").is_err() {
        eprintln!("Skipping e2e: DEEPSEEK_API_KEY not set");
        return Ok(());
    }
    cmd!(sh, "cargo test -p llm-proxy -- e2e").run()?;
    Ok(())
}

pub fn ci() -> Result<()> {
    let steps: Vec<(&str, fn() -> Result<()>)> =
        vec![("fmt", fmt), ("lint", lint), ("test", test), ("audit", audit)];

    for (name, step) in &steps {
        eprintln!("--- xtask ci: {} ---", name);
        if let Err(e) = step() {
            bail!("{} failed: {}", name, e);
        }
    }
    eprintln!("--- xtask ci: all passed ---");
    Ok(())
}
