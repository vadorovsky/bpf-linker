use std::ffi::OsStr;

use crate::cargo::{run_cargo, CargoArgs};

pub fn build(args: CargoArgs) -> anyhow::Result<()> {
    run_cargo(args, OsStr::new("build"))
}
