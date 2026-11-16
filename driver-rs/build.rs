//! Passes the WDK's link settings for a UMDF driver DLL to Cargo.

fn main() -> Result<(), wdk_build::ConfigError> {
    wdk_build::configure_wdk_binary_build()
}
