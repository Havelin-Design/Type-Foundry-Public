//! Embeds the window icon in the Windows executable.

use std::env;
use std::fs;
use std::path::PathBuf;

fn main() {
    println!("cargo:rerun-if-changed=assets/type-foundry-icon.png");
    let png = fs::read("assets/type-foundry-icon.png").expect("the app icon png is in assets/");
    let ico = png_as_ico(&png);
    let out = PathBuf::from(env::var("OUT_DIR").expect("OUT_DIR")).join("type-foundry.ico");
    fs::write(&out, ico).expect("write the app icon");

    #[cfg(windows)]
    {
        let mut res = winresource::WindowsResource::new();
        res.set_icon(out.to_str().expect("icon path"));
        res.compile().expect("embed the app icon");
    }
}

/// A Vista icon whose only image is the PNG. Width and height bytes of 0 mean 256.
fn png_as_ico(png: &[u8]) -> Vec<u8> {
    let mut ico = Vec::with_capacity(22 + png.len());
    ico.extend_from_slice(&0u16.to_le_bytes());
    ico.extend_from_slice(&1u16.to_le_bytes());
    ico.extend_from_slice(&1u16.to_le_bytes());
    ico.extend_from_slice(&[0, 0, 0, 0]);
    ico.extend_from_slice(&1u16.to_le_bytes());
    ico.extend_from_slice(&32u16.to_le_bytes());
    ico.extend_from_slice(&(png.len() as u32).to_le_bytes());
    ico.extend_from_slice(&22u32.to_le_bytes());
    ico.extend_from_slice(png);
    ico
}
