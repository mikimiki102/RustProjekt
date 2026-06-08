use std::env;
use std::fs;
use std::process;
pub mod core;

// TEMPORARY
use core::memcompress::{byte_level_compress, byte_level_decompress};
//use core::file::fileformat; // USE THAT

fn main() {
    let args: Vec<String> = env::args().collect();

    if args.len() != 4 {
        eprintln!("Usage: cargo run --<mode[required]> <input_file[required]> <output_file[required]>");
        eprintln!("Examples:");
        eprintln!("  cargo run --compress input output.rle");
        eprintln!("  cargo run --decompress input.rle output");
        process::exit(1);
    }

    let mode = &args[1];
    let input_path = &args[2];
    let output_path = &args[3];

    let input_data = match fs::read(input_path) {
        Ok(data) => data,
        Err(error) => {
            eprintln!("Błąd odczytu pliku: {}", error);
            process::exit(1);
        }
    };

    let output_data = match mode.as_str() {
        "compress" => byte_level_compress(&input_data),

        "decompress" => match byte_level_decompress(&input_data) {
            Ok(data) => data,
            Err(error) => {
                eprintln!("Błąd dekompresji: {}", error);
                process::exit(1);
            }
        },

        _ => {
            eprintln!("Nieznany tryb: {}", mode);
            eprintln!("Dostępne tryby: compress, decompress");
            process::exit(1);
        }
    };

    if let Err(error) = fs::write(output_path, &output_data) {
        eprintln!("Błąd zapisu pliku: {}", error);
        process::exit(1);
    }

    println!("Done.");
}
