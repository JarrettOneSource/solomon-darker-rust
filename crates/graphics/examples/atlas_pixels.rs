//! Export exact PNG texels for browser GPU comparisons without a Canvas round trip.

#[cfg(not(target_arch = "wasm32"))]
fn main() -> Result<(), Box<dyn std::error::Error>> {
    use std::path::PathBuf;

    let mut arguments = std::env::args_os().skip(1);
    let output = PathBuf::from(arguments.next().ok_or("Usage: atlas_pixels OUTPUT PNG...")?);
    let inputs: Vec<PathBuf> = arguments.map(PathBuf::from).collect();
    if inputs.is_empty() {
        return Err("Usage: atlas_pixels OUTPUT PNG...".into());
    }
    std::fs::create_dir_all(&output)?;
    for input in inputs {
        let name = input.file_stem().ok_or("PNG path needs a file name")?;
        let pixels = image::open(&input)?.into_rgba8();
        let mut destination = output.join(name);
        destination.set_extension("rgba");
        std::fs::write(destination, pixels.as_raw())?;
        println!("{}: {} x {} RGBA", input.display(), pixels.width(), pixels.height());
    }
    Ok(())
}

#[cfg(target_arch = "wasm32")]
fn main() {}
