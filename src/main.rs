use anyhow::{Context, Result};
use gradient_generator::extract_gradient_hex;
use opencv::prelude::ColorTraitConst;
use std::fs::File;
use std::{
	fs::{self, OpenOptions},
	io::{BufRead, BufReader, Write},
	path::Path,
};

const SEPARATOR: &str = "/";

fn main() -> Result<()> {
	let args: Vec<String> = std::env::args().collect();
	if args.len() < 2 {
		eprintln!("Usage: {} <image_path>", args[0]);
		std::process::exit(1);
	}

	let image_path = Path::new(&args[1]);
	let absolute_path = fs::canonicalize(image_path).context("Failed to canonicalize path")?;
	let image_dir = absolute_path.parent().unwrap_or(Path::new("."));
	let base_name = absolute_path.file_name().unwrap().to_str().unwrap();

	let cache_file = image_dir.join(".gradient_cache.csv");

	if cache_file.exists() {
		let file = File::open(&cache_file).context("Failed to open cache file")?;
		let reader = BufReader::new(file);

		for line in reader.lines() {
			let line = line.context("Failed to read cache line")?;
			let parts: Vec<&str> = line.split(SEPARATOR).collect();
			if parts.len() == 4 && parts[0] == base_name {
				println!("{}", parts[1]);
				println!("{}", parts[2]);
				println!("{}", parts[3]);
				return Ok(());
			}
		}
	}

	let result = extract_gradient_hex(&absolute_path)
		.context(format!("Error processing image: {:?}", absolute_path))?;

	let start_vec = result.start_color.to_vec3b()?;
	let start_hex = format!(
		"#{:02x}{:02x}{:02x}",
		start_vec[2], start_vec[1], start_vec[0]
	);
	let end_vec = result.end_color.to_vec3b()?;
	let end_hex = format!("#{:02x}{:02x}{:02x}", end_vec[2], end_vec[1], end_vec[0]);

	if let Ok(mut file) = OpenOptions::new()
		.create(true)
		.append(true)
		.open(&cache_file)
	{
		let rounded_angle = result.angle.round() as i32;
		writeln!(
			file,
			"{}{}{}{}{}{}{}",
			base_name, SEPARATOR, start_hex, SEPARATOR, end_hex, SEPARATOR, rounded_angle
		)
		.ok();
	}

	println!("{}", start_hex);
	println!("{}", end_hex);
	println!("{}", result.angle.round() as i32);

	Ok(())
}
