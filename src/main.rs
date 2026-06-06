use std::env;
use std::fs;
use std::process;
pub mod core;
use core::rlecompress::byte_level_compress;

fn decompress(data: &[u8]) -> Result<Vec<u8>, String> {
    if data.len() % 2 != 0 {
        return Err("Niepoprawny plik RLE: liczba bajtów nie jest parzysta".to_string());
    }

    let mut result = Vec::new();

    for pair in data.chunks(2) {
        let count = pair[0];
        let byte = pair[1];

        for _ in 0..count {
            result.push(byte);
        }
    }

    Ok(result)
}

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

        "decompress" => match decompress(&input_data) {
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_compress_and_decompress_text() {
        let input = b"AAAABBBCCDAA";

        let compressed = byte_level_compress(input);
        let decompressed = decompress(&compressed).unwrap();

        assert_eq!(input.to_vec(), decompressed);
    }

    #[test]
    fn test_empty_data() {
        let input = b"";

        let compressed = byte_level_compress(input);
        let decompressed = decompress(&compressed).unwrap();

        assert_eq!(input.to_vec(), decompressed);
    }

    #[test]
    fn test_no_repetitions() {
        let input = b"ABCDEF";

        let compressed = byte_level_compress(input);
        let decompressed = decompress(&compressed).unwrap();

        assert_eq!(input.to_vec(), decompressed);
    }

    #[test]
    fn test_long_sequence() {
        let input = vec![b'A'; 300];

        let compressed = byte_level_compress(&input);
        let decompressed = decompress(&compressed).unwrap();

        assert_eq!(input, decompressed);
    }
}
