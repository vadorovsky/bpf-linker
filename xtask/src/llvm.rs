use std::{ffi::OsString, path::Path};

pub enum System {
    Darwin,
    Linux,
}

impl ToString for System {
    fn to_string(&self) -> String {
        match self {
            Self::Darwin => "Darwin".to_owned(),
            Self::Linux => "Linux".to_owned(),
        }
    }
}

pub enum Processor {
    Aarch64,
    Riscv64,
    X86_64,
}

impl ToString for Processor {
    fn to_string(&self) -> String {
        match self {
            Self::Aarch64 => "aarch64".to_owned(),
            Self::Riscv64 => "riscv64".to_owned(),
            Self::X86_64 => "x86_64".to_owned(),
        }
    }
}

pub struct LlvmBuildConfig {
    pub c_compiler: String,
    pub cxx_compiler: String,
    pub compiler_target: Option<String>,
    pub cxxflags: Option<String>,
    pub ldflags: Option<String>,
    pub install_prefix: OsString,
    pub skip_install_rpath: bool,
    pub system: System,
    pub processor: Processor,
    pub target_triple: String,
}

impl LlvmBuildConfig {
    pub fn cmake_args(&self) -> Vec<OsString> {
        let LlvmBuildConfig {
            c_compiler,
            cxx_compiler,
            compiler_target,
            cxxflags,
            ldflags,
            install_prefix,
            skip_install_rpath,
            system,
            processor,
            target_triple,
        } = self;

        // NOTE(vadorovsky): I wish there was a `format!` equivalent for
        // `OsString`...
        let mut install_arg = OsString::from("-DCMAKE_INSTALL_PREFIX=");
        install_arg.push(install_prefix);
        let mut rpath_arg = OsString::from("-DCMAKE_INSTALL_RPATH=");
        rpath_arg.push(Path::new(install_prefix).join("lib"));

        let mut args = vec![
            OsString::from("-S"),
            OsString::from("llvm"),
            OsString::from("-B"),
            OsString::from(format!("aya-build-{}", target_triple)),
            OsString::from("-DCMAKE_BUILD_TYPE=RelWithDebInfo"),
            OsString::from(format!("-DCMAKE_ASM_COMPILER={c_compiler}")),
            OsString::from("-DCMAKE_BUILD_WITH_INSTALL_RPATH=ON"),
            OsString::from(format!("-DCMAKE_C_COMPILER={c_compiler}")),
            OsString::from(format!("-DCMAKE_CXX_COMPILER={cxx_compiler}")),
            install_arg,
            rpath_arg,
            OsString::from(format!("-DCMAKE_SYSTEM_NAME={}", system.to_string())),
            OsString::from(format!(
                "-DCMAKE_SYSTEM_PROCESSOR={}",
                processor.to_string()
            )),
            OsString::from("-DLLVM_BUILD_EXAMPLES=OFF"),
            OsString::from("-DLLVM_BUILD_STATIC=ON"),
            OsString::from("-DLLVM_ENABLE_ASSERTIONS=ON"),
            OsString::from("-DLLVM_ENABLE_LIBCXX=ON"),
            OsString::from("-DLLVM_ENABLE_LIBXML2=OFF"),
            OsString::from("-DLLVM_ENABLE_PROJECTS="),
            OsString::from("-DLLVM_ENABLE_RUNTIMES="),
            OsString::from(format!("-DLLVM_HOST_TRIPLE={target_triple}")),
            OsString::from("-DLLVM_INCLUDE_TESTS=OFF"),
            OsString::from("-DLLVM_INCLUDE_TOOLS=OFF"),
            OsString::from("-DLLVM_INCLUDE_UTILS=OFF"),
            OsString::from("-DLLVM_TARGETS_TO_BUILD=BPF"),
            OsString::from("-DLLVM_USE_LINKER=lld"),
        ];

        if let Some(compiler_target) = compiler_target {
            args.push(OsString::from(format!(
                "-DCMAKE_ASM_COMPILER_TARGET={compiler_target}"
            )));
            args.push(OsString::from(format!(
                "-DCMAKE_C_COMPILER_TARGET={compiler_target}"
            )));
            args.push(OsString::from(format!(
                "-DCMAKE_CXX_COMPILER_TARGET={compiler_target}"
            )));
        }
        if let Some(cxxflags) = cxxflags {
            args.push(OsString::from(format!("-DCMAKE_CXX_FLAGS='{cxxflags}'")));
        }
        if let Some(ldflags) = ldflags {
            args.push(OsString::from(format!(
                "-DCMAKE_EXE_LINKER_FLAGS='{ldflags}'"
            )));
            args.push(OsString::from(format!(
                "-DCMAKE_SHARED_LINKER_FLAGS='{ldflags}"
            )));
        }
        if *skip_install_rpath {
            args.push(OsString::from("-DCMAKE_SKIP_INSTALL_RPATH=ON".to_owned()));
        }

        args
    }
}
