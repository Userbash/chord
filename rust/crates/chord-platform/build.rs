//! Resolve MSVC native libraries from installed vcpkg metadata. Unix and MinGW
//! retain the existing system Hunspell ABI link in ffi_hunspell.rs.
fn main() -> Result<(), Box<dyn std::error::Error>> {
    for variable in [
        "VCPKG_ROOT",
        "VCPKGRS_TRIPLET",
        "VCPKGRS_DYNAMIC",
        "VCPKGRS_DISABLE",
        "VCPKGRS_NO_HUNSPELL",
        "NO_VCPKG",
    ] {
        println!("cargo:rerun-if-env-changed={variable}");
    }
    // cfg! describes the build-script host, which can differ from Cargo's target.
    if std::env::var("CARGO_CFG_TARGET_OS").as_deref() == Ok("windows")
        && std::env::var("CARGO_CFG_TARGET_ENV").as_deref() == Ok("msvc")
    {
        vcpkg::Config::new()
            .copy_dlls(false)
            .find_package("hunspell")?;
    }
    Ok(())
}
