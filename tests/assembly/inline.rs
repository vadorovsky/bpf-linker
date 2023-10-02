// assembly-output: bpf-linker
// compile-flags: --crate-type cdylib

// Verify that LLVM inlines the functions aggressively.
#![no_std]

// aux-build: loop-panic-handler.rs
extern crate loop_panic_handler;

#[no_mangle]
static FORBIDDEN_INODE: u64 = 0;

#[no_mangle]
static FORBIDDEN_PID: i32 = 0;

#[no_mangle]
static FORBIDDEN_NS: u32 = 0;

const EINVAL: i64 = 14;

#[repr(C)]
pub struct Task {
    last_wakee: *mut Task,
    real_parent: *mut Task,
    parent: *mut Task,
    group_leader: *mut Task,
    pi_top_task: *mut Task,
    oom_reaper_list: *mut Task,
    // pub mm: *mut Mm,
    nsproxy: *mut Nsproxy,
    // pub pid: i32,
    // pub tgid: i32,
}

impl Task {
    pub unsafe fn last_wakee(&self) -> Option<&Task> {
        self.last_wakee.as_ref()
    }

    pub unsafe fn real_parent(&self) -> Option<&Task> {
        self.real_parent.as_ref()
    }

    pub unsafe fn parent(&self) -> Option<&Task> {
        self.parent.as_ref()
    }

    pub unsafe fn group_leader(&self) -> Option<&Task> {
        self.group_leader.as_ref()
    }

    pub unsafe fn pi_top_task(&self) -> Option<&Task> {
        self.pi_top_task.as_ref()
    }

    pub unsafe fn oom_reaper_list(&self) -> Option<&Task> {
        self.oom_reaper_list.as_ref()
    }

    pub unsafe fn nsproxy(&self) -> Option<&Nsproxy> {
        self.nsproxy.as_ref()
    }
}

// #[repr(C)]
// pub struct Mm {
//     pub exe_file: *mut File,
// }
//
// #[repr(C)]
// pub struct File {
//     pub f_inode: *mut Inode,
// }
//
// #[repr(C)]
// pub struct Inode {
//     pub i_ino: u64,
// }

#[repr(C)]
pub struct Nsproxy {
    pub uts_ns: *mut NsCommon,
    pub ipc_ns: *mut NsCommon,
    pub mnt_ns: *mut NsCommon,
    pub pid_ns: *mut NsCommon,
    pub net_ns: *mut NsCommon,
    pub time_ns: *mut NsCommon,
    pub cgroup_ns: *mut NsCommon,
}

impl Nsproxy {
    pub unsafe fn uts_ns(&self) -> Option<&NsCommon> {
        self.uts_ns.as_ref()
    }

    pub unsafe fn ipc_ns(&self) -> Option<&NsCommon> {
        self.ipc_ns.as_ref()
    }

    pub unsafe fn mnt_ns(&self) -> Option<&NsCommon> {
        self.mnt_ns.as_ref()
    }

    pub unsafe fn pid_ns(&self) -> Option<&NsCommon> {
        self.pid_ns.as_ref()
    }

    pub unsafe fn net_ns(&self) -> Option<&NsCommon> {
        self.net_ns.as_ref()
    }

    pub unsafe fn time_ns(&self) -> Option<&NsCommon> {
        self.time_ns.as_ref()
    }

    pub unsafe fn cgroup_ns(&self) -> Option<&NsCommon> {
        self.cgroup_ns.as_ref()
    }
}

#[repr(C)]
pub struct NsCommon {
    pub inum: u32,
}

unsafe fn arg<'a, T>(ctx: *mut core::ffi::c_void, n: usize) -> Option<&'a T> {
    let ptr = unsafe { *(ctx as *const usize).add(n) as *const T };
    ptr.as_ref()
}

#[no_mangle]
#[link_section = "lsm/task_alloc"]
pub fn task_alloc(ctx: *mut core::ffi::c_void) -> i32 {
    match try_task_alloc(ctx) {
        Ok(ret) => ret,
        Err(_) => -1,
    }
}

fn try_task_alloc(ctx: *mut core::ffi::c_void) -> Result<i32, i64> {
    let task: &Task = unsafe { arg(ctx, 0).ok_or(-1)? };

    // check_binary(task)?;
    // check_pid(task)?;
    check_namespace_all_tasks(task)?;

    Ok(0)
}

// fn check_binary(task: *const Task) -> Result<i32, i64> {
//     let bin_ino = unsafe { (*(*(*(*task).mm).exe_file).f_inode).i_ino };
//     if bin_ino == FORBIDDEN_INODE {
//         return Ok(-1);
//     }
//
//     let bin_ino = unsafe { (*(*(*(*(*task).last_wakee).mm).exe_file).f_inode).i_ino };
//     if bin_ino == FORBIDDEN_INODE {
//         return Ok(-1);
//     }
//
//     let bin_ino = unsafe { (*(*(*(*(*task).real_parent).mm).exe_file).f_inode).i_ino };
//     if bin_ino == FORBIDDEN_INODE {
//         return Ok(-1);
//     }
//
//     let bin_ino = unsafe { (*(*(*(*(*task).parent).mm).exe_file).f_inode).i_ino };
//     if bin_ino == FORBIDDEN_INODE {
//         return Ok(-1);
//     }
//
//     Ok(0)
// }
//
// fn check_pid(task: *const Task) -> Result<i32, i64> {
//     let pid = unsafe { (*task).pid };
//     if pid == FORBIDDEN_PID {
//         return Ok(-1);
//     }
//
//     let pid = unsafe { (*(*task).last_wakee).pid };
//     if pid == FORBIDDEN_PID {
//         return Ok(-1);
//     }
//
//     let pid = unsafe { (*(*task).real_parent).pid };
//     if pid == FORBIDDEN_PID {
//         return Ok(-1);
//     }
//
//     let pid = unsafe { (*(*task).parent).pid };
//     if pid == FORBIDDEN_PID {
//         return Ok(-1);
//     }
//
//     Ok(0)
// }

fn check_namespace_all_tasks(task: &Task) -> Result<i32, i64> {
    check_namespaces_task(task)?;
    if let Some(last_wakee) = unsafe { task.last_wakee() } {
        check_namespaces_task(last_wakee)?;
    }
    if let Some(real_parent) = unsafe { task.real_parent() } {
        check_namespaces_task(real_parent)?;
    }
    if let Some(parent) = unsafe { task.parent() } {
        check_namespaces_task(parent)?;
    }

    Ok(0)
}

// #[inline(always)]
fn check_namespaces_task(task: &Task) -> Result<i32, i64> {
    if let Some(nsproxy) = unsafe { task.nsproxy() } {
        if let Some(uts_ns) = unsafe { nsproxy.uts_ns() } {
            if uts_ns.inum == FORBIDDEN_NS {
                return Ok(-1);
            }
        }

        if let Some(ipc_ns) = unsafe { nsproxy.ipc_ns() } {
            if ipc_ns.inum == FORBIDDEN_NS {
                return Ok(-1);
            }
        }

        if let Some(mnt_ns) = unsafe { nsproxy.mnt_ns() } {
            if mnt_ns.inum == FORBIDDEN_NS {
                return Ok(-1);
            }
        }

        if let Some(pid_ns) = unsafe { nsproxy.pid_ns() } {
            if pid_ns.inum == FORBIDDEN_NS {
                return Ok(-1);
            }
        }

        if let Some(net_ns) = unsafe { nsproxy.net_ns() } {
            if net_ns.inum == FORBIDDEN_NS {
                return Ok(-1);
            }
        }

        if let Some(time_ns) = unsafe { nsproxy.time_ns() } {
            if time_ns.inum == FORBIDDEN_NS {
                return Ok(-1);
            }
        }

        if let Some(cgroup_ns) = unsafe { nsproxy.cgroup_ns() } {
            if cgroup_ns.inum == FORBIDDEN_NS {
                return Ok(-1);
            }
        }
    }

    Ok(0)
}

// CHECK: -section "foobar","ax"
