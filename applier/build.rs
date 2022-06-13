use std::env::{self, VarError};
use std::path::PathBuf;

#[cfg(target_os = "linux")]
const NDK_HOST_TAG: &str = "linux-x86_64";
#[cfg(target_os = "macos")]
const NDK_HOST_TAG: &str = "darwin-x86_64";
#[cfg(all(target_os = "windows", target_arch = "x86"))]
const NDK_HOST_TAG: &str = "windows";
#[cfg(all(target_os = "windows", target_arch = "x86_64"))]
const NDK_HOST_TAG: &str = "windows-x86_64";

fn main() {
    // TODO: Find a matching unity install for every enabled il2cpp version and generate bindings for each
    let editors_path = match env::var("UNITY_EDITORS") {
        Ok(path) => PathBuf::from(path),
        Err(VarError::NotPresent) => panic!(
            "please set $UNITY_EDITORS to the path to a folder where the editors are installed"
        ),
        Err(e) => panic!("{:?}", e),
    };

    let ndk_path = match env::var("ANDROID_NDK_HOME") {
        Ok(path) => PathBuf::from(path),
        Err(VarError::NotPresent) => {
            panic!("please set $ANDROID_NDK_HOME to the path of your ndk installation")
        }
        Err(e) => panic!("{:?}", e),
    };
    let ndk_sysroot_include = ndk_path.join(format!(
        "toolchains/llvm/prebuilt/{}/sysroot/usr/include",
        NDK_HOST_TAG
    ));

    let libil2cpp_path = editors_path.join("2019.4.28f1/Editor/Data/il2cpp/libil2cpp");
    let bindings = bindgen::Builder::default()
        .clang_arg(format!("-I{}", libil2cpp_path.to_str().unwrap()))
        .clang_arg(format!("-isystem{}", ndk_sysroot_include.to_str().unwrap()))
        .header(
            libil2cpp_path
                .join("il2cpp-class-internals.h")
                .to_str()
                .unwrap(),
        )
        // TODO: Can I derive these for specific types only?
        .derive_eq(true)
        .derive_hash(true)
        .generate()
        .expect("Unable to generate bindings");

    let out_path = PathBuf::from(env::var("OUT_DIR").unwrap());
    bindings
        .write_to_file(out_path.join("bindings_24_5.rs"))
        .expect("Couldn't write bindings!");
    println!("cargo:rustc-link-arg=-Wl,-soname,libmerge_applier.so")
}
