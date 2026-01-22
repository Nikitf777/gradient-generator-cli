use anyhow::{Context, Result};
use clap::Parser;
use gradient_generator::extract_gradient_hex;
use opencv::{
	core::{self, CV_8UC3, Mat, Scalar, Vector},
	imgcodecs,
	prelude::{ColorTraitConst, MatTraitManual},
};
use std::{
	fs::{self, File, OpenOptions},
	io::{self, BufRead, BufReader, Write},
	path::Path,
};

const SEPARATOR: &str = "/"; // Use a charackter, that a filename cannot contain

#[derive(Parser)]
#[command(author, version, about, long_about = None)]
struct Args {
	/// Path to the image file
	image_path: String,

	/// Do not use cached results
	#[arg(long)]
	no_use_cache: bool,

	/// Do not save results to cache
	#[arg(long)]
	no_save_cache: bool,

	/// Generate gradient image instead of printing color values
	#[arg(long)]
	image: bool,

	/// Width of the generated image (only with --image)
	#[arg(long, default_value_t = 800, requires = "image")]
	width: u32,

	/// Height of the generated image (only with --image)
	#[arg(long, default_value_t = 600, requires = "image")]
	height: u32,

	/// Output format: png, jpg, or raw (only with --image)
	#[arg(long, default_value = "png", value_parser = ["png", "jpg", "raw"], requires = "image")]
	format: String,
}

fn main() -> Result<()> {
	let args = Args::parse();
	let image_path = Path::new(&args.image_path);
	let absolute_path = fs::canonicalize(image_path).context("Failed to canonicalize path")?;
	let image_dir = absolute_path.parent().unwrap_or(Path::new("."));
	let base_name = absolute_path.file_name().unwrap().to_str().unwrap();
	let cache_file = image_dir.join(".gradient_cache.csv");

	let (start_color, end_color, exact_angle, rounded_angle) = if !args.no_use_cache
		&& cache_file.exists()
	{
		match read_cache(&cache_file, base_name) {
			Some((start_hex, end_hex, angle_i32)) => {
				let start_vec = hex_to_vec3b(&start_hex)?;
				let end_vec = hex_to_vec3b(&end_hex)?;
				(start_vec, end_vec, angle_i32 as f64, angle_i32)
			}
			None => {
				let (start_vec, end_vec, exact_angle, rounded_angle) =
					compute_gradient(&absolute_path)?;
				if !args.no_save_cache {
					save_cache(&cache_file, base_name, &start_vec, &end_vec, rounded_angle)?;
				}
				(start_vec, end_vec, exact_angle, rounded_angle)
			}
		}
	} else {
		let (start_vec, end_vec, exact_angle, rounded_angle) = compute_gradient(&absolute_path)?;
		if !args.no_save_cache {
			save_cache(&cache_file, base_name, &start_vec, &end_vec, rounded_angle)?;
		}
		(start_vec, end_vec, exact_angle, rounded_angle)
	};

	if args.image {
		generate_image(
			start_color,
			end_color,
			exact_angle,
			args.width,
			args.height,
			&args.format,
		)?;
	} else {
		let start_hex = vec3b_to_hex(&start_color);
		let end_hex = vec3b_to_hex(&end_color);
		println!("{}", start_hex);
		println!("{}", end_hex);
		println!("{}", rounded_angle);
	}

	Ok(())
}

fn read_cache(cache_file: &Path, base_name: &str) -> Option<(String, String, i32)> {
	let file = File::open(cache_file).ok()?;
	let reader = BufReader::new(file);

	for line in reader.lines() {
		if let Ok(line) = line {
			let parts: Vec<&str> = line.split(SEPARATOR).collect();
			if parts.len() == 4 && parts[0] == base_name {
				if let Ok(angle) = parts[3].parse() {
					return Some((parts[1].to_string(), parts[2].to_string(), angle));
				}
			}
		}
	}
	None
}

fn hex_to_vec3b(hex: &str) -> Result<core::Vec3b> {
	if hex.len() != 7 || !hex.starts_with('#') {
		anyhow::bail!("Invalid hex string format: {}", hex);
	}

	let r = u8::from_str_radix(&hex[1..3], 16)?;
	let g = u8::from_str_radix(&hex[3..5], 16)?;
	let b = u8::from_str_radix(&hex[5..7], 16)?;
	Ok(core::Vec3b::from([b, g, r]))
}

fn vec3b_to_hex(vec: &core::Vec3b) -> String {
	format!("#{:02x}{:02x}{:02x}", vec[2], vec[1], vec[0])
}

fn compute_gradient(image_path: &Path) -> Result<(core::Vec3b, core::Vec3b, f64, i32)> {
	let result = extract_gradient_hex(image_path, 200, 50.0)
		.context(format!("Error processing image: {:?}", image_path))?;

	let start_vec = result.start_color.to_vec3b()?;
	let end_vec = result.end_color.to_vec3b()?;
	let exact_angle = result.angle;
	let rounded_angle = exact_angle.round() as i32;

	Ok((start_vec, end_vec, exact_angle, rounded_angle))
}

fn save_cache(
	cache_file: &Path,
	base_name: &str,
	start_vec: &core::Vec3b,
	end_vec: &core::Vec3b,
	rounded_angle: i32,
) -> Result<()> {
	let start_hex = vec3b_to_hex(start_vec);
	let end_hex = vec3b_to_hex(end_vec);

	let mut file = OpenOptions::new()
		.create(true)
		.append(true)
		.open(cache_file)
		.context("Failed to open cache file for writing")?;

	writeln!(
		file,
		"{}{}{}{}{}{}{}",
		base_name, SEPARATOR, start_hex, SEPARATOR, end_hex, SEPARATOR, rounded_angle
	)?;

	Ok(())
}

fn generate_image(
	start_color: core::Vec3b,
	end_color: core::Vec3b,
	angle_deg: f64,
	width: u32,
	height: u32,
	format: &str,
) -> Result<()> {
	let angle_rad = angle_deg.to_radians();
	let dx = angle_rad.cos();
	let dy = angle_rad.sin();

	let mut image =
		Mat::new_rows_cols_with_default(height as i32, width as i32, CV_8UC3, Scalar::all(0.0))?;

	let data = image.data_bytes_mut()?;

	if data.len() != (width * height * 3) as usize {
		anyhow::bail!(
			"Image data size mismatch: expected {} bytes, got {}",
			width * height * 3,
			data.len()
		);
	}

	let corners = [
		(0.0, 0.0),
		(width as f64 - 1.0, 0.0),
		(0.0, height as f64 - 1.0),
		(width as f64 - 1.0, height as f64 - 1.0),
	];

	let projections: Vec<f64> = corners.iter().map(|(x, y)| x * dx + y * dy).collect();

	let min_proj = projections.iter().fold(f64::INFINITY, |a, &b| a.min(b));
	let max_proj = projections.iter().fold(f64::NEG_INFINITY, |a, &b| a.max(b));
	let range = max_proj - min_proj;

	if range < 1e-5 {
		anyhow::bail!("Invalid gradient range: {}", range);
	}

	for y in 0..height {
		for x in 0..width {
			let proj = (x as f64) * dx + (y as f64) * dy;
			let t = ((proj - min_proj) / range).clamp(0.0, 1.0);

			let b = (start_color[0] as f64 * (1.0 - t) + end_color[0] as f64 * t) as u8;
			let g = (start_color[1] as f64 * (1.0 - t) + end_color[1] as f64 * t) as u8;
			let r = (start_color[2] as f64 * (1.0 - t) + end_color[2] as f64 * t) as u8;

			let idx = ((y * width + x) * 3) as usize;
			data[idx] = b;
			data[idx + 1] = g;
			data[idx + 2] = r;
		}
	}

	let mut stdout = io::stdout();
	if format == "raw" {
		stdout.write_all(&data)?;
	} else {
		let mut buf = Vector::new();
		let params = Vector::new();

		match format {
			"png" => imgcodecs::imencode(".png", &image, &mut buf, &params).map(|_| ())?,
			"jpg" => imgcodecs::imencode(".jpg", &image, &mut buf, &params).map(|_| ())?,
			_ => anyhow::bail!("Unsupported format: {}", format),
		}

		stdout.write_all(buf.as_slice())?;
	}

	Ok(())
}
