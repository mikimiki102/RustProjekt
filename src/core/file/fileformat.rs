use crate::core::file::filecompress::{
    FileCompressPipeline, 
    FileCompressSettings
};
use std::io::Write;
use super::fileanalyze::FileAnalyzer;
use super::filecompress::FileCompressor;
use std::fs::File;

#[derive(Debug, Clone)]
pub struct FileFormaterHints {
    pub chunk_size_hint: usize
}

impl FileFormaterHints {
    pub fn default() -> Self {
        const DEFAULT_CHUNK_SIZE: usize = 4096;

        Self {
            chunk_size_hint: DEFAULT_CHUNK_SIZE
        }
    }
}

#[derive(Debug)]
pub struct FileFormater {
    pub analyzer: FileAnalyzer
}

impl FileFormater {
    pub fn new(analyzer: &FileAnalyzer) -> Self {
        Self {
            analyzer: analyzer.clone(),
        }
    }

    pub fn file_compress(&self, 
                         pipeline: &FileCompressPipeline, 
                         compressor_hints: Option<FileFormaterHints>) 
    {
        let input_fp = &pipeline.input;
        let output_fp = &pipeline.output;

        let compressor_hints = compressor_hints.unwrap_or(FileFormaterHints::default());
        let chunk_size_hint = compressor_hints.chunk_size_hint;

        let compression_level = self.analyzer.get_suggested_compression(input_fp).unwrap().unwrap();

        let compressor = FileCompressor::new(
            &FileCompressSettings {
                chunk_size_hint: chunk_size_hint,
                compression_level: compression_level.clone()
            }
        );

        // Mark compression mode in the output file
        let mut output_file = File::create(output_fp)
                                          .expect("Couldn't create output file");
        
        let header_byte = compression_level.get_marker_value();

        output_file.write_all(&[header_byte])
                   .expect("Failed to write header to file");
        drop(output_file);

        compressor.compress_input_to_output(&pipeline);
    }

    pub fn file_decompress(&self, 
                           pipeline: &FileCompressPipeline, 
                           compressor_hits: Option<FileFormaterHints>) 
    {
        // TODO
    }

}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::core::file::fileanalyze::FileAnalyzerSettings;
    use std::fs::{self};
    use std::path::PathBuf;
    use std::io::Read;
    use crate::core::file::filecompress::CompressorLevel;

    fn create_test_file(filename: &str, data: &[u8]) -> PathBuf {
        let mut fp = PathBuf::from("assets");
        let _ = fs::create_dir_all(&fp);
        fp.push(filename);

        let mut file = File::create(&fp).expect("Failed to create test input file");
        file.write_all(data).expect("Failed to write test data");
        file.flush().expect("Failed to flush test data");
        fp
    }

    fn setup_formater(probe_size: usize, threads: usize) -> FileFormater {
        let analyzer_settings = FileAnalyzerSettings {
            probe_chunk_size: probe_size,
            thread_cnt: threads,
        };

        let analyzer = FileAnalyzer::new(&analyzer_settings);
        FileFormater::new(&analyzer)
    }

    #[test]
    fn test_file_formater_hints_default() {
        let hints = FileFormaterHints::default();
        assert_eq!(hints.chunk_size_hint, 4096);
    }

    #[test]
    fn test_file_compress_writes_correct_header_and_triggers_compression() {
        let input_data = vec![0b1010_1010u8; 100];
        
        let input_fp = create_test_file("formatter_input_byte", &input_data);
        let output_fp = PathBuf::from("assets").join("formatter_output_byte");

        let pipeline = FileCompressPipeline {
            input: input_fp.clone(),
            output: output_fp.clone(),
        };

        let formater = setup_formater(512, 2);

        formater.file_compress(&pipeline, None);

        let mut output_file = File::open(&output_fp).expect("Failed to open output file for verification");
        let mut header_buf = [0u8; 1];
        output_file.read_exact(&mut header_buf).expect("Failed to read header byte");

        let correct_mark = CompressorLevel::CompressorByteLevel.get_marker_value();

        assert_eq!(header_buf[0], correct_mark, "Invalid expected compression level");

        let mut compressed_data = Vec::new();
        output_file.read_to_end(&mut compressed_data).expect("Failed to read compressed payload");
        assert!(!compressed_data.is_empty(), "Empty file data apart from header marker");

        let _ = fs::remove_file(&input_fp);
        let _ = fs::remove_file(&output_fp);
    }

    #[test]
    fn test_file_compress_with_custom_hints() {
        let input_data = vec![0x01, 0x02, 0x03, 0x04, 0x05, 0x06, 0x07, 0x08];
        let input_fp = create_test_file("formatter_input_custom", &input_data);
        let output_fp = PathBuf::from("assets").join("formatter_output_custom");

        let pipeline = FileCompressPipeline {
            input: input_fp.clone(),
            output: output_fp.clone(),
        };

        let formater = setup_formater(256, 1);
        let custom_hints = Some(FileFormaterHints { chunk_size_hint: 1024 });

        formater.file_compress(&pipeline, custom_hints);

        let metadata = fs::metadata(&output_fp).expect("Output file was not created");
        assert!(metadata.len() > 0);

        let _ = fs::remove_file(&input_fp);
        let _ = fs::remove_file(&output_fp);
    }

    #[test]
    #[should_panic]
    fn test_file_compress_panic_on_non_existent_input() {
        let pipeline = FileCompressPipeline {
            input: PathBuf::from("assets/ghost_file_that_does_not_exist"),
            output: PathBuf::from("assets/ghost_output"),
        };

        let formater = setup_formater(512, 2);
        formater.file_compress(&pipeline, None);
    }
}
