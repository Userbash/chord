use std::path::PathBuf;

fn main() {
    println!("cargo:rerun-if-changed=cpp/dave.h");
    println!("cargo:rerun-if-changed=cpp/dave_c_api.cpp");

    let openssl = pkg_config::Config::new()
        .probe("libcrypto")
        .expect("system OpenSSL libcrypto must be installed for libdave C++ FFI spike");

    let mut build = cc::Build::new();
    build
        .cpp(true)
        .file("cpp/dave_c_api.cpp")
        .include(PathBuf::from("cpp"))
        .flag_if_supported("-std=c++17")
        .flag_if_supported("-O2")
        .flag_if_supported("-Wall")
        .flag_if_supported("-Wextra")
        .warnings(false);

    for inc in &openssl.include_paths {
        build.include(inc);
    }

    build.compile("dave_c_spike");

    for link_path in &openssl.link_paths {
        println!("cargo:rustc-link-search=native={}", link_path.display());
    }
    for lib in &openssl.libs {
        println!("cargo:rustc-link-lib={lib}");
    }
}
