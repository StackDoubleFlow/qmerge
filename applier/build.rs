use std::env::{self, VarError};
use std::path::PathBuf;

fn main() {
    // TODO: Find a matching unity install for every enabled il2cpp version and generate bindings for each
    let editors_path = match env::var("UNITY_EDITORS") {
        Ok(path) => PathBuf::from(path),
        Err(VarError::NotPresent) => panic!(
            "please set $UNITY_EDITORS to the path to a folder where the editors are installed"
        ),
        Err(e) => panic!("{:?}", e),
    };

    let libil2cpp_path = editors_path.join("2019.4.28f1/Editor/Data/il2cpp/libil2cpp");
    dbg!(format!("-I{}", libil2cpp_path.to_str().unwrap()));
    let bindings = bindgen::Builder::default()
        .clang_arg(format!("-I{}", libil2cpp_path.to_str().unwrap()))
        .header(
            libil2cpp_path
                .join("il2cpp-class-internals.h")
                .to_str()
                .unwrap(),
        )
        .generate()
        .expect("Unable to generate bindings");

    let out_path = PathBuf::from(env::var("OUT_DIR").unwrap());
    bindings
        .write_to_file(out_path.join("bindings_24_5.rs"))
        .expect("Couldn't write bindings!");
}
