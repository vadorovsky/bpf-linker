use std::{
    ffi::OsString,
    fs,
    path::{self, PathBuf},
    process::Command,
};

use anyhow::{Context as _, Result};
use rustc_build_sysroot::{BuildMode, SysrootConfig, SysrootStatus};
use walkdir::WalkDir;

#[derive(Clone, clap::ValueEnum)]
enum StdTarget {
    BpfebUnknownNone,
    BpfelUnknownNone,
}

impl StdTarget {
    fn as_str(&self) -> &'static str {
        match self {
            Self::BpfebUnknownNone => "bpfeb-unknown-none",
            Self::BpfelUnknownNone => "bpfel-unknown-none",
        }
    }
}

#[derive(clap::Parser)]
struct BuildStd {
    #[arg(long)]
    rustc_src: PathBuf,

    #[arg(long)]
    sysroot_dir: PathBuf,

    #[arg(long, value_enum)]
    target: StdTarget,
}

#[derive(clap::Parser)]
struct BuildLlvm {
    /// Source directory.
    #[arg(long)]
    src_dir: PathBuf,
    /// Build directory.
    #[arg(long)]
    build_dir: PathBuf,
    /// Target.
    #[arg(long)]
    target: Option<String>,
    /// Use github.com/exein-io/icedragon.
    #[arg(long)]
    icedragon: bool,
    /// Directory in which the built LLVM artifacts are installed.
    #[arg(long)]
    install_prefix: PathBuf,
}

#[derive(clap::Subcommand)]
enum XtaskSubcommand {
    /// Builds the Rust standard library for the given target in the current
    /// toolchain's sysroot.
    BuildStd(BuildStd),
    /// Manages and builds LLVM.
    BuildLlvm(BuildLlvm),
}

/// Additional build commands for bpf-linker.
#[derive(clap::Parser)]
struct CommandLine {
    #[command(subcommand)]
    subcommand: XtaskSubcommand,
}

fn build_std(options: BuildStd) -> Result<()> {
    let BuildStd {
        rustc_src,
        sysroot_dir,
        target,
    } = options;

    let target = target.as_str();
    let sysroot_status =
        match rustc_build_sysroot::SysrootBuilder::new(sysroot_dir.as_path(), target)
            // Do a full sysroot build.
            .build_mode(BuildMode::Build)
            // We want only `core`, not `std`.
            .sysroot_config(SysrootConfig::NoStd)
            // Include debug symbols in order to generate correct BTF types for
            // the core types as well.
            .rustflag("-Cdebuginfo=2")
            .build_from_source(&rustc_src)?
        {
            SysrootStatus::AlreadyCached => "was already built",
            SysrootStatus::SysrootBuilt => "built successfully",
        };
    println!(
        "Standard library for target {target} {sysroot_status}: {}",
        sysroot_dir.display()
    );
    Ok(())
}

fn build_llvm(options: BuildLlvm) -> Result<()> {
    let BuildLlvm {
        src_dir,
        build_dir,
        target,
        icedragon,
        install_prefix,
    } = options;

    let build_dir = path::absolute(&build_dir).with_context(|| {
        format!(
            "failed to make `build_dir` {} absolute",
            build_dir.display()
        )
    })?;
    let install_prefix = path::absolute(&install_prefix).with_context(|| {
        format!(
            "failed to make `install_prefix` {} absolute",
            install_prefix.display()
        )
    })?;

    let mut configure_cmd = if icedragon {
        let mut configure_cmd = Command::new("icedragon");
        let _ = configure_cmd.args(["cmake"]);
        if let Some(ref target) = target {
            let _ = configure_cmd.arg("--target").arg(target);
        }
        let _ = configure_cmd.args([
            "--",
            // Directory inside icedragon's container.
            "-DCMAKE_INSTALL_PREFIX=/llvm-install",
        ]);
        configure_cmd
    } else {
        let mut configure_cmd = Command::new("cmake");

        let mut install_arg = OsString::from("-DCMAKE_INSTALL_PREFIX=");
        install_arg.push(install_prefix.as_os_str());
        let _ = configure_cmd.arg(install_arg);

        if let Some(ref target) = target {
            let _ = configure_cmd.args([
                format!("-DCMAKE_ASM_COMPILER_TARGET={target}"),
                format!("-DCMAKE_C_COMPILER_TARGET={target}"),
                format!("-DCMAKE_CXX_COMPILER_TARGET={target}"),
            ]);
        }

        configure_cmd
    };
    let configure_cmd = configure_cmd
        .arg("-S")
        .arg("llvm")
        .arg("-B")
        .arg(&build_dir)
        .args([
            "-G",
            "Ninja",
            "-DCMAKE_BUILD_TYPE=RelWithDebInfo",
            "-DCMAKE_C_COMPILER=clang",
            "-DCMAKE_CXX_COMPILER=clang++",
            "-DLLVM_BUILD_LLVM_DYLIB=ON",
            "-DLLVM_ENABLE_ASSERTIONS=ON",
            "-DLLVM_ENABLE_PROJECTS=",
            "-DLLVM_ENABLE_RUNTIMES=",
            "-DLLVM_INSTALL_UTILS=ON",
            "-DLLVM_LINK_LLVM_DYLIB=ON",
            "-DLLVM_TARGETS_TO_BUILD=BPF",
            "-DLLVM_USE_LINKER=lld",
        ])
        .current_dir(&src_dir);
    println!("Configuring LLVM with command {configure_cmd:?}");
    let status = configure_cmd.status().with_context(|| {
        format!("failed to configure LLVM build with command {configure_cmd:?}")
    })?;
    if !status.success() {
        anyhow::bail!("failed to configure LLVM build with command {configure_cmd:?}: {status}");
    }

    let mut build_cmd = if icedragon {
        if !install_prefix.exists() {
            fs::create_dir_all(&install_prefix).with_context(|| {
                format!(
                    "failed to create `install_prefix` {}",
                    install_prefix.display()
                )
            })?;
        }
        let mut build_cmd = Command::new("icedragon");
        let _ = build_cmd.args(["cmake", "--volume"]);
        let mut volume_arg = install_prefix.clone().into_os_string();
        volume_arg.push(":/llvm-install");
        let _ = build_cmd.arg(volume_arg);
        if let Some(target) = target {
            let _ = build_cmd.arg("--target").arg(target);
        }
        let _ = build_cmd.arg("--");
        build_cmd
    } else {
        Command::new("cmake")
    };
    let build_cmd = build_cmd
        .arg("--build")
        .arg(build_dir)
        .args(["--target", "install"])
        // Create symlinks rather than copies to conserve disk space,
        // especially on GitHub-hosted runners.
        //
        // Since the LLVM build creates a bunch of symlinks (and this setting
        // does not turn those into symlinks-to-symlinks), use absolute
        // symlinks so we can distinguish the two cases.
        .env("CMAKE_INSTALL_MODE", "ABS_SYMLINK");
    println!("Building LLVM with command {build_cmd:?}");
    let status = build_cmd
        .status()
        .with_context(|| format!("failed to build LLVM with command {configure_cmd:?}"))?;
    if !status.success() {
        anyhow::bail!("failed to configure LLVM build with command {configure_cmd:?}: {status}");
    }

    // Move targets over the symlinks that point to them.
    //
    // This whole dance would be simpler if CMake supported
    // `CMAKE_INSTALL_MODE=MOVE`.
    for entry in WalkDir::new(&install_prefix).follow_links(false) {
        let entry = entry.with_context(|| {
            format!(
                "failed to read filesystem entry while traversing install prefix {}",
                install_prefix.display()
            )
        })?;
        if !entry.file_type().is_symlink() {
            continue;
        }

        let link_path = entry.path();
        let target = fs::read_link(link_path)
            .with_context(|| format!("failed to read the link {}", link_path.display()))?;
        if target.is_absolute() {
            fs::rename(&target, link_path).with_context(|| {
                format!(
                    "failed to move the target file {} to the location of the symlink {}",
                    target.display(),
                    link_path.display()
                )
            })?;
        }
    }

    Ok(())
}

fn main() -> Result<()> {
    let CommandLine { subcommand } = clap::Parser::parse();
    match subcommand {
        XtaskSubcommand::BuildStd(options) => build_std(options),
        XtaskSubcommand::BuildLlvm(options) => build_llvm(options),
    }
}
