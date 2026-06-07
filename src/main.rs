use std::env;
use std::fs;
use std::process;
pub mod core;
use core::rlecompress::{byte_level_compress, byte_level_decompress};

fn print_stats(original_size: usize, output_size: usize) {
    println!("Rozmiar wejściowy: {} bajtów", original_size);
    println!("Rozmiar wyjściowy: {} bajtów", output_size);

    if original_size > 0 {
        let ratio = output_size as f64 / original_size as f64 * 100.0;
        println!("Rozmiar po operacji: {:.2}% rozmiaru wejściowego", ratio);
    }
}

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

    println!("Operacja zakończona powodzeniem.");
    print_stats(input_data.len(), output_data.len());
}
