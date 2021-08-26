extern crate dirs;
extern crate toml;

use std::error::Error;
use std::path::Path;
use std::path::PathBuf;

#[derive(serde::Deserialize)]
struct CargoLock {
    package: Vec<CargoPackage>,
}

#[derive(serde::Deserialize)]
struct CargoPackage {
    name: String,
    version: String,
}

pub fn setup() -> Result<(), Box<dyn Error>> {
    if cfg!(not(target_os = "windows")) {
        return Ok(());
    }

    let build_type = std::env::var("PROFILE")?;

    let tankersdk_bin_path = get_tanker_bin_path()?;

    // copy the DLL alongside unit tests
    std::fs::copy(
        tankersdk_bin_path.join("ctanker.dll"),
        format!("target/{}/deps/ctanker.dll", build_type),
    )?;
    // and alongside the app
    std::fs::copy(
        tankersdk_bin_path.join("ctanker.dll"),
        format!("target/{}/ctanker.dll", build_type),
    )?;
    Ok(())
}

fn parse_tanker_version() -> Result<String, Box<dyn Error>> {
    let lock_content = std::fs::read_to_string("Cargo.lock")?;
    let lock: CargoLock = toml::from_str(&lock_content)?;

    Ok(lock
        .package
        .iter()
        .find(|&p| p.name == "tankersdk")
        .unwrap()
        .version
        .to_owned())
}

fn get_tanker_bin_path() -> Result<PathBuf, Box<dyn Error>> {
    let vendor_dir = Path::new("vendor");
    let target_triplet = std::env::var("TARGET")?;

    let path = if vendor_dir.exists() {
        let vendored_path_str = format!("vendor/tankersdk/native/{}", target_triplet);
        Path::new(&vendored_path_str).to_owned()
    } else {
        let tanker_registry = "gitlab.com-de4ae77755a8b2c8";
        let tanker_version = parse_tanker_version()?;
        let tankersdk_bin_path_str = format!(
            "{}/.cargo/registry/src/{}/tankersdk-{}/native/{}",
            dirs::home_dir().unwrap().as_path().display(),
            tanker_registry,
            tanker_version,
            target_triplet
        );
        Path::new(&tankersdk_bin_path_str).to_owned()
    };
    Ok(path)
}
