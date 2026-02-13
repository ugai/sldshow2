use std::env;
use std::fs;
use std::path::Path;

fn main() {
    // Embed application icon and metadata into the Windows executable
    #[cfg(windows)]
    {
        let mut res = winresource::WindowsResource::new();
        res.set_icon("assets/icon/icon.ico");
        res.compile().unwrap();
    }

    // 1. Determine source assets directory
    let manifest_dir = env::var("CARGO_MANIFEST_DIR").expect("CARGO_MANIFEST_DIR not set");
    let src_assets = Path::new(&manifest_dir).join("assets").join("fonts");

    // 2. Determine target directory
    // This is tricky because build scripts run in OUT_DIR, which is deep inside target.
    // We want to verify where the binary will be placed.
    // Common heuristic: .../target/debug/build/pkg-xxx/out -> .../target/debug

    let profile = env::var("PROFILE").expect("PROFILE not set");
    let target_dir = Path::new(&manifest_dir).join("target").join(&profile);

    // Note: This heuristic might fail if user overrides target dir, but it covers 99% of cases.
    let dest_assets = target_dir.join("assets").join("fonts");

    println!("cargo:rerun-if-changed=assets/fonts");

    if src_assets.exists() {
        if let Err(e) = copy_dir_recursive(&src_assets, &dest_assets) {
            println!("cargo:warning=Failed to copy assets: {}", e);
        }
    } else {
        println!("cargo:warning=Assets directory not found: {:?}", src_assets);
    }
}

fn copy_dir_recursive(src: &Path, dst: &Path) -> std::io::Result<()> {
    if !dst.exists() {
        fs::create_dir_all(dst)?;
    }

    for entry in fs::read_dir(src)? {
        let entry = entry?;
        let ty = entry.file_type()?;
        let src_path = entry.path();
        let dst_path = dst.join(entry.file_name());

        if ty.is_dir() {
            copy_dir_recursive(&src_path, &dst_path)?;
        } else {
            fs::copy(&src_path, &dst_path)?;
        }
    }
    Ok(())
}
