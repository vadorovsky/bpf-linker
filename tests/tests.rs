use std::{
    env,
    ffi::{OsStr, OsString},
    fs,
    os::unix::ffi::OsStringExt,
    path::{Path, PathBuf},
    process::Command,
};

fn build_stdlib(target: &str) {
    let cargo = env::var_os("CARGO").expect("could not determine the cargo binary to use");
    let cargo = PathBuf::from(cargo);
    // Cargo doesn't provide any environment variable with the `rustc` path,
    // but since we know it should be present in the same directory as `cargo`,
    // let's.
    let rustc = cargo
        .parent()
        .expect("cargo path should have a parent")
        .join("rustc");

    let output = Command::new(rustc)
        .arg("--print")
        .arg("sysroot")
        .output()
        .expect("failed to execute rustc");
    if !output.status.success() {
        panic!(
            "rustc failed with code {:?}\nstdout: {}\nstderr: {}",
            output.status.code(),
            String::from_utf8_lossy(&output.stdout),
            String::from_utf8_lossy(&output.stderr)
        );
    }
    let sysroot = output.stdout.trim_ascii();

    let corelib = PathBuf::from(OsString::from_vec(sysroot.to_vec()))
        .join("lib/rustlib/src/rust/library/core");
    println!("{corelib:?}");
    let output = Command::new(cargo)
        .arg("build")
        .arg("--target")
        .arg(target)
        .current_dir(&corelib)
        .env("CARGO_TARGET_DIR", "/tmp/bpf-linker-stdlib")
        .output()
        .expect("failed to execute cargo");
    if !output.status.success() {
        panic!(
            "cargo failed with code {:?}\nstdout: {}\nstderr: {}",
            output.status.code(),
            String::from_utf8_lossy(&output.stdout),
            String::from_utf8_lossy(&output.stderr)
        );
    }
}

fn find_binary(binary_re_str: &str) -> PathBuf {
    let binary_re = regex::Regex::new(binary_re_str).unwrap();
    let mut binary = which::which_re(binary_re).expect(binary_re_str);
    binary
        .next()
        .unwrap_or_else(|| panic!("could not find {binary_re_str}"))
}

fn run_mode<F: Fn(&mut compiletest_rs::Config)>(target: &str, mode: &str, cfg: Option<F>) {
    let target_rustcflags = format!("-C linker={} ", env!("CARGO_BIN_EXE_bpf-linker"));

    let llvm_filecheck = Some(find_binary(r"^FileCheck(-\d+)?$"));

    let mode = mode.parse().expect("Invalid mode");
    let mut config = compiletest_rs::Config {
        target: target.to_owned(),
        target_rustcflags: Some(target_rustcflags),
        llvm_filecheck,
        mode,
        src_base: PathBuf::from(format!("tests/{}", mode)),
        ..Default::default()
    };
    config.link_deps();

    if let Some(cfg) = cfg {
        cfg(&mut config);
    }

    compiletest_rs::run_tests(&config);
}

/// Builds LLVM bitcode files from LLVM IR files located in a specified directory.
fn build_bitcode<P>(src_dir: P, dst_dir: P)
where
    P: AsRef<Path>,
{
    fs::create_dir_all(dst_dir.as_ref()).expect("failed to create a build directory for bitcode");
    for entry in fs::read_dir(src_dir.as_ref()).expect("failed to read the directory") {
        let entry = entry.expect("failed to read the entry");
        let path = entry.path();

        if path.is_file() && path.extension() == Some(OsStr::new("c")) {
            let bc_dst = dst_dir
                .as_ref()
                .join(path.with_extension("bc").file_name().unwrap());
            clang_build(path, bc_dst);
        }
    }
}

/// Compiles C code into an LLVM bitcode file.
fn clang_build<P>(src: P, dst: P)
where
    P: AsRef<Path>,
{
    let clang = find_binary(r"^clang(-\d+)?$");
    let output = Command::new(clang)
        .arg("-target")
        .arg("bpf")
        .arg("-g")
        .arg("-c")
        .arg("-emit-llvm")
        .arg("-o")
        .arg(dst.as_ref())
        .arg(src.as_ref())
        .output()
        .expect("failed to execute clang");

    if !output.status.success() {
        panic!(
            "clang failed with code {:?}\nstdout: {}\nstderr: {}",
            output.status.code(),
            String::from_utf8_lossy(&output.stdout),
            String::from_utf8_lossy(&output.stderr)
        );
    }
}

fn btf_dump(src: &Path, dst: &Path) {
    let dst = std::fs::File::create(dst)
        .unwrap_or_else(|err| panic!("could not open btf dump file '{}': {err}", dst.display()));
    let mut btf = Command::new("btf");
    btf.arg("dump").arg(src).stdout(dst);
    let status = btf
        .status()
        .unwrap_or_else(|err| panic!("could not run {btf:?}: {err}"));
    assert_eq!(status.code(), Some(0), "{btf:?} failed");
}

#[test]
fn compile_test() {
    let target = "bpfel-unknown-none";

    build_stdlib(&target);

    let root_dir = env::var_os("CARGO_MANIFEST_DIR")
        .expect("could not determine the root directory of the project");
    let root_dir = Path::new(&root_dir);

    build_bitcode(root_dir.join("tests/c"), root_dir.join("target/bitcode"));

    run_mode(target, "assembly", None::<fn(&mut compiletest_rs::Config)>);
    run_mode(
        target,
        "assembly",
        Some(|cfg: &mut compiletest_rs::Config| {
            cfg.src_base = PathBuf::from("tests/btf");
            cfg.llvm_filecheck_preprocess = Some(btf_dump);
        }),
    );
}
