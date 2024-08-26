use llvm_sys::target_machine::{LLVMDisposeTargetMachine, LLVMTargetMachineRef};
use tracing::info;

use crate::Cpu;

pub struct TargetMachine {
    target_machine_ref: LLVMTargetMachineRef,
}

impl Drop for TargetMachine {
    fn drop(&mut self) {
        unsafe { LLVMDisposeTargetMachine(self.target_machine_ref) }
    }
}

impl TargetMachine {
    pub fn new(target: Option<String>, cpu: Cpu, cpu_features: String) -> Self {
        // Here's how the output target is selected:
        //
        // 1) rustc with builtin BPF support: cargo build --target=bpf[el|eb]-unknown-none
        //      the input modules are already configured for the correct output target
        //
        // 2) rustc with no BPF support: cargo rustc -- -C linker-flavor=bpf-linker -C linker=bpf-linker -C link-arg=--target=bpf[el|eb]
        //      the input modules are configured for the *host* target, and the output target
        //      is configured with the `--target` linker argument
        //
        // 3) rustc with no BPF support: cargo rustc -- -C linker-flavor=bpf-linker -C linker=bpf-linker
        //      the input modules are configured for the *host* target, the output target isn't
        //      set via `--target`, so default to `bpf` (bpfel or bpfeb depending on the host
        //      endianness)
        let (triple, target) = match target {
            // case 1
            Some(triple) => {
                let c_triple = CString::new(triple.as_str()).unwrap();
                (triple.as_str(), unsafe {
                    llvm::target_from_triple(&c_triple)
                })
            }
            None => {
                let c_triple = unsafe { LLVMGetTarget(*module) };
                let triple = unsafe { CStr::from_ptr(c_triple) }.to_str().unwrap();
                if triple.starts_with("bpf") {
                    // case 2
                    (triple, unsafe { llvm::target_from_module(*module) })
                } else {
                    // case 3.
                    info!("detected non-bpf input target {} and no explicit output --target specified, selecting `bpf'", triple);
                    let triple = "bpf";
                    let c_triple = CString::new(triple).unwrap();
                    (triple, unsafe { llvm::target_from_triple(&c_triple) })
                }
            }
        };
    }
}
