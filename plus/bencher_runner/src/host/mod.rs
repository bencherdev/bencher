//! What the runner reads of its host at the idle decision point, never during a Job.

pub(crate) mod health;
pub(crate) mod md;
pub(crate) mod nvme;

pub(crate) const SYSFS: &str = "/sys";
