//! Display/configuration names may change; storage format versions do not.
pub const NAME: &str = "S1Code";
pub const BIN: &str = "s1code";
pub const HOME_ENV: &str = "S1CODE_HOME";
pub const LEGACY_HOME_ENV: &str = "NERVE_HOME";
pub const LEGACY_BIN: &str = "nerve";
// All v1 executables must contend on the same workspace lock, across renames.
pub const WORKSPACE_LOCK_NAMESPACE: &str = "nerve";
pub const STORAGE_VERSION: u32 = 1;
pub const POLICY_VERSION: &str = "policy-6";
