fn main() {
    println!("cargo:rustc-link-arg=-Wl,-soname,libmerge_applier.so")
}
