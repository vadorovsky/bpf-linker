use std::{env, fs::File, path::Path, process::Command};

use anyhow::{bail, Context};
use clap::Parser;
use serde::Deserialize;
use zip::ZipArchive;

#[derive(Debug, Parser)]
pub struct Options {
    /// Build the release target
    #[clap(long)]
    pub release: bool,
}

pub fn build(opts: Options) -> anyhow::Result<()> {
    let llvm_path = download_and_extract_llvm_artifact()?;
    println!("llvm_path: {}", llvm_path);

    let mut args = vec!["build"];
    if opts.release {
        args.push("--release");
    }
    let status = Command::new("cargo")
        .args(&args)
        .status()
        .context("failed to build bpf-linker")?;
    if !status.success() {
        bail!("failed to build bpf-linker, status: {}", status);
    }
    Ok(())
}

#[derive(Debug, Deserialize)]
struct Artifact {
    // name: String,
    url: String,
}

#[derive(Debug, Deserialize)]
struct ArtifactResponse {
    artifacts: Vec<Artifact>,
}

pub fn download_and_extract_llvm_artifact() -> anyhow::Result<String> {
    // Define GitHub API URL to fetch the latest run artifacts
    let url = "https://api.github.com/repos/aya-rs/llvm-project/actions/artifacts";

    // Define headers including the GitHub token for authentication
    let client = reqwest::blocking::Client::new();
    let mut response = client
        .get(url)
        // .header("Authorization", format!("token {}", token))
        .header("User-Agent", "curl/8.2.1")
        .header("Accept", "*/*")
        .send()
        .context("failed to fetch artifacts from GitHub")?;

    println!("response: {:?}", response);

    // Deserialize the JSON response
    let artifact_response: ArtifactResponse =
        response.json().context("failed to parse response")?;

    println!("artifacts: {:?}", artifact_response);

    // Find the first artifact with the name "llvm-artifacts"
    let artifact_url = artifact_response.artifacts[0]
        // .iter()
        // .find(|artifact| artifact.name == "llvm-artifacts")
        // .context("failed to find llvm-artifacts")?
        .url
        .clone();

    println!("artifact_url: {}", artifact_url);

    // Download the artifact
    let mut response = client
        .get(format!("{}/zip", artifact_url))
        // .header("Authorization", format!("token {}", token))
        .header("User-Agent", "curl/8.2.1")
        .header("Accept", "*/*")
        .send()
        .context("failed to download the artifact")?;

    let tmp_path = Path::new("llvm-artifacts.zip");
    let mut file = File::create(&tmp_path).context("failed to create zip file")?;
    response
        .copy_to(&mut file)
        .context("failed to copy content to zip file")?;

    // Extract the artifact to the desired directory
    let destination_path = Path::new("llvm-install");
    let reader = File::open(&tmp_path).context("failed to open zip file")?;
    let mut archive = ZipArchive::new(reader).context("failed to read zip archive")?;
    archive
        .extract(&destination_path)
        .context("failed to extract zip archive")?;

    // Set the environment variable to the extracted path
    let llvm_path = destination_path
        .to_str()
        .context("failed to convert path to string")?;
    env::set_var("LLVM_SYS_170_PREFIX", llvm_path);

    Ok(llvm_path.to_string())
}
