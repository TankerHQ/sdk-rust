use std::error::Error;
use std::io::Write;
use std::os::unix::ffi::OsStrExt;
use std::path::PathBuf;

const BINDGEN_OUTPUT_FILENAME: &str = "ctanker.rs";
const TANKER_LIB_BASENAME: &str = "tanker";

fn main() -> Result<(), Box<dyn Error>> {
    let target_triple = std::env::var("TARGET")?;
    let manifest_dir = std::env::var_os("CARGO_MANIFEST_DIR").unwrap();
    let mut bindings_folder = PathBuf::from(manifest_dir);
    bindings_folder.push("native");
    bindings_folder.push(&target_triple);

    let tanker_lib_filename = &format!("lib{}.a", TANKER_LIB_BASENAME);
    if !bindings_folder.exists() {
        panic!(
            "Target platform {} is not supported ({} does not exist)",
            target_triple,
            bindings_folder.to_string_lossy()
        );
    }
    if !bindings_folder.join(tanker_lib_filename).exists() {
        panic!(
            "Couldn't find {} in {}",
            tanker_lib_filename,
            bindings_folder.to_string_lossy()
        );
    }
    if !bindings_folder.join(BINDGEN_OUTPUT_FILENAME).exists() {
        panic!(
            "Couldn't find the bindgen-generated {} in {}",
            BINDGEN_OUTPUT_FILENAME,
            bindings_folder.to_string_lossy()
        );
    }

    println!(
        "cargo:rerun-if-changed={}/{}",
        bindings_folder.to_string_lossy(),
        BINDGEN_OUTPUT_FILENAME
    );
    println!(
        "cargo:rerun-if-changed={}/{}",
        bindings_folder.to_string_lossy(),
        tanker_lib_filename
    );

    // Paths can contain anything, but env vars are a liiitle more restricted. Sanity checks!
    let bindings_folder = bindings_folder.as_os_str().as_bytes();
    assert!(!bindings_folder.contains(&b'='));
    assert!(!bindings_folder.contains(&b'\0'));
    assert!(!bindings_folder.contains(&b'\n'));

    // Export an env var so we can include ctanker.rs in the source code
    print!("cargo:rustc-env=NATIVE_BINDINGS_FOLDER=");
    std::io::stdout().write_all(bindings_folder).unwrap();
    println!();

    // Tell cargo to link with our native library
    print!("cargo:rustc-link-search=");
    std::io::stdout().write_all(bindings_folder).unwrap();
    println!();
    println!("cargo:rustc-link-lib=static={}", TANKER_LIB_BASENAME);

    Ok(())
}
