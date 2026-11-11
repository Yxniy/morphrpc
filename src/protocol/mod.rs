//! Protocol compatibility markers for the first wire specification.

const INITIAL_VERSION: u8 = 1;

/// Returns whether the version is recognized by the initial scaffold.
pub const fn supports_version(version: u8) -> bool {
    version == INITIAL_VERSION
}
