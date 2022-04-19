use esbuild_rs::{transform_direct, Loader, TransformOptionsBuilder};
use std::path::Path;
use std::sync::mpsc::sync_channel;
use std::sync::Arc;
use std::{env, fs};

pub fn main() -> Result<(), Box<dyn std::error::Error>> {
    let input = Arc::new(tachyons::TACHYONS.as_bytes().to_vec());
    let out_dir = env::var_os("OUT_DIR").unwrap();
    let output = Path::new(&out_dir).join("tachyons.css");

    let mut options = TransformOptionsBuilder::new();
    options.loader = Loader::CSS;
    options.minify_identifiers = true;
    options.minify_syntax = true;
    options.minify_whitespace = true;
    let options = options.build();

    let (sender, receiver) = sync_channel(1);

    transform_direct(input, options, move |result| {
        let code = result.code.as_str();
        fs::write(output, code).unwrap();
        sender.send(()).unwrap();
    });

    receiver.recv().unwrap();

    println!("cargo:rerun-if-changed=build.rs");

    Ok(())
}
