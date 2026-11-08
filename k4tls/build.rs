fn main() {
    if std::env::var("CARGO_CFG_TARGET_OS").as_deref() == Ok("linux")
        && std::env::var("CARGO_CFG_TARGET_ENV").as_deref() == Ok("gnu")
    {
        // getaddrinfo may outlive its caller, so dlclose must not unmap the worker.
        println!("cargo:rustc-cdylib-link-arg=-Wl,-z,nodelete");
    }
    if std::env::var("TARGET").as_deref() == Ok("arm-unknown-linux-gnueabi") {
        println!("cargo:rustc-link-arg=-Wl,--wrap=getauxval");
        println!("cargo:rustc-link-arg=-Wl,-z,defs");
    }
}
