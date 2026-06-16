use std::env;
use std::fs;
use std::path::PathBuf;
use std::process;
use std::time::Instant;

pub mod core;

use crate::core::file::fileanalyze::{FileAnalyzer, FileAnalyzerSettings};
use crate::core::file::filecompress::CompressorLevel;
use crate::core::file::fileformat::{FileFormater, FileFormaterHints, FileFormaterPipeline};

fn print_usage() {
    eprintln!("Usage:");
    eprintln!("  cargo run -- compress <input_file> <output_file.rle>");
    eprintln!("  cargo run -- decompress <input_file.rle> <output_file>");
    eprintln!("  cargo run -- analyze <input_file>");
}

fn compression_name(level: &CompressorLevel) -> &'static str {
    match level {
        CompressorLevel::CompressorByteLevel => "byte-level",
        CompressorLevel::CompressorBitLevel => "bit-level",
    }
}

fn print_size_stats(input_size: u64, output_size: u64) {
    println!("Input size:  {} bytes", input_size);
    println!("Output size: {} bytes", output_size);

    if input_size > 0 {
        let ratio = output_size as f64 / input_size as f64 * 100.0;
        let saved = 100.0 - ratio;

        println!("Output/Input ratio: {:.2}%", ratio);

        if saved >= 0.0 {
            println!("Saved space: {:.2}%", saved);
        } else {
            println!("File increased by: {:.2}%", -saved);
        }
    }
}

fn create_analyzer() -> FileAnalyzer {
    let settings = FileAnalyzerSettings {
        probe_chunk_size: 4096,
        thread_cnt: 4,
    };

    FileAnalyzer::new(&settings)
}

fn create_hints() -> FileFormaterHints {
    FileFormaterHints {
        chunk_size_hint: 4096,
    }
}

fn remove_output_if_exists(output_path: &PathBuf) {
    if output_path.exists() {
        if let Err(error) = fs::remove_file(output_path) {
            eprintln!("Could not remove existing output file: {}", error);
            process::exit(1);
        }
    }
}

fn analyze_file(input_path: &PathBuf, analyzer: &FileAnalyzer) {
    let result = match analyzer.get_suggested_compression(input_path) {
        Ok(result) => result,
        Err(error) => {
            eprintln!("Analyze error: {}", error);
            process::exit(1);
        }
    };

    match result {
        Some(level) => {
            println!("Suggested compression: {}", compression_name(&level));
        }
        None => {
            println!("File is empty. No compression suggested.");
        }
    }
}

fn compress_file(input_path: PathBuf, output_path: PathBuf, analyzer: &FileAnalyzer) {
    if !input_path.exists() {
        eprintln!("Input file does not exist: {:?}", input_path);
        process::exit(1);
    }

    let suggested = match analyzer.get_suggested_compression(&input_path) {
        Ok(Some(level)) => level,
        Ok(None) => {
            eprintln!("Input file is empty. Nothing to compress.");
            process::exit(1);
        }
        Err(error) => {
            eprintln!("Analyze error: {}", error);
            process::exit(1);
        }
    };

    remove_output_if_exists(&output_path);

    let input_size = fs::metadata(&input_path)
        .map(|metadata| metadata.len())
        .unwrap_or(0);

    let formater = FileFormater::new(analyzer);

    let pipeline = FileFormaterPipeline {
        input_fp: input_path.clone(),
        output_fp: output_path.clone(),
    };

    let hints = create_hints();

    println!("Selected compression: {}", compression_name(&suggested));

    let start = Instant::now();
    formater.file_compress(&pipeline, Some(hints));
    let duration = start.elapsed();

    let output_size = fs::metadata(&output_path)
        .map(|metadata| metadata.len())
        .unwrap_or(0);

    println!("Compression finished.");
    print_size_stats(input_size, output_size);
    println!("Time: {:.2?}", duration);
}

fn decompress_file(input_path: PathBuf, output_path: PathBuf, analyzer: &FileAnalyzer) {
    if !input_path.exists() {
        eprintln!("Input file does not exist: {:?}", input_path);
        process::exit(1);
    }

    remove_output_if_exists(&output_path);

    let input_size = fs::metadata(&input_path)
        .map(|metadata| metadata.len())
        .unwrap_or(0);

    let formater = FileFormater::new(analyzer);

    let pipeline = FileFormaterPipeline {
        input_fp: input_path.clone(),
        output_fp: output_path.clone(),
    };

    let hints = create_hints();

    let start = Instant::now();
    formater.file_decompress(&pipeline, Some(hints));
    let duration = start.elapsed();

    let output_size = fs::metadata(&output_path)
        .map(|metadata| metadata.len())
        .unwrap_or(0);

    println!("Decompression finished.");
    print_size_stats(input_size, output_size);
    println!("Time: {:.2?}", duration);
}

fn main() {
    let args: Vec<String> = env::args().collect();

    if args.len() < 3 {
        print_usage();
        process::exit(1);
    }

    let mode = &args[1];
    let analyzer = create_analyzer();

    match mode.as_str() {
        "analyze" => {
            if args.len() != 3 {
                print_usage();
                process::exit(1);
            }

            let input_path = PathBuf::from(&args[2]);
            analyze_file(&input_path, &analyzer);
        }

        "compress" => {
            if args.len() != 4 {
                print_usage();
                process::exit(1);
            }

            let input_path = PathBuf::from(&args[2]);
            let output_path = PathBuf::from(&args[3]);

            compress_file(input_path, output_path, &analyzer);
        }

        "decompress" => {
            if args.len() != 4 {
                print_usage();
                process::exit(1);
            }

            let input_path = PathBuf::from(&args[2]);
            let output_path = PathBuf::from(&args[3]);

            decompress_file(input_path, output_path, &analyzer);
        }

        _ => {
            eprintln!("Unknown mode: {}", mode);
            print_usage();
            process::exit(1);
        }
    }
}
