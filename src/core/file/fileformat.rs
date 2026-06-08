//! File: fileformat.rs.
//! 
//! Provides [`FileFormater`] structure for final compressing
//! or decompressing an input file to output.
//! Specify data flow using [`FileFormaterPipeline`].

use super::fileanalyze::FileAnalyzer;
use super::filecompress::FileCompressor;
use std::path::PathBuf;
use std::fs::{
    File, 
    OpenOptions
};
use crate::core::file::filecompress::{
    CompressorLevel, 
    FileCompressPipeline, 
    FileCompressSettings
};
use std::io::{
    Write, 
    Read
};

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

#[derive(Debug, Clone)]
pub struct FileFormaterPipeline {
    pub input_fp: PathBuf,
    pub output_fp: PathBuf,
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

    /// That is final abstraction layer of  the compression workflow.
    /// Specify optional `compressor_hints` for custom workmode, or `None` for default behaviour.
    /// Output file: `pipeline.output` is expected to be non-existent.
    pub fn file_compress(&self, 
                         pipeline: &FileFormaterPipeline, 
                         compressor_hints: Option<FileFormaterHints>) 
    {
        let input_fp = &pipeline.input_fp;
        let output_fp = &pipeline.output_fp;

        let compressor_hints = compressor_hints.unwrap_or(FileFormaterHints::default());
        let chunk_size_hint = compressor_hints.chunk_size_hint;

        let compression_level = self.analyzer.get_suggested_compression(input_fp).unwrap().unwrap();

        let compressor = FileCompressor::new(
            &FileCompressSettings {
                chunk_size_hint: chunk_size_hint,
                compression_level: compression_level.clone()
            }
        );

        let mut output_file = match OpenOptions::new()
                                            .write(true)
                                            .append(true) 
                                            .create(true)
                                            .open(&output_fp) {
            Ok(f) => f,
            Err(e) => {
                panic!("Couldn't create output file: {e}");
            }
        };
        
        let header_byte = compression_level.get_marker_value();

        output_file.write_all(&[header_byte])
                   .expect("Failed to write header to file");

        let mut input_file = match File::open(&input_fp) {
            Ok(f) => f,
            Err(e) => {
                eprintln!("Couldn't open input file: {e}");
                return;
            }
        };

        let compressor_pipeline = FileCompressPipeline {
            input: &mut input_file,
            output: &mut output_file
        };

        compressor.compress_input_to_output(compressor_pipeline);

        drop(input_file);
        drop(output_file);
    }

    /// That is final abstraction layer of decompression workflow.
    /// Specify optional `compressor_hints` for custom workmode, or `None` for default behaviour.
    pub fn file_decompress(&self, 
                           pipeline: &FileFormaterPipeline, 
                           decompressor_hints: Option<FileFormaterHints>) 
    {
        let input_fp = &pipeline.input_fp;
        let output_fp = &pipeline.output_fp;

        let decompressor_hints = decompressor_hints.unwrap_or(FileFormaterHints::default());
        let chunk_size_hint = decompressor_hints.chunk_size_hint;

        let mut input_file = match File::open(&input_fp) {
            Ok(f) => f,
            Err(e) => {
                eprintln!("Couldn't open input file: {e}");
                return;
            }
        };
        
        let mut header_buf = [0u8; 1];
        input_file.read_exact(&mut header_buf).expect("Failed to read header byte");
        let compression_level = CompressorLevel::from_value(header_buf[0])
                                                    .expect("Invalid header markers - file is not compressed");

        let mut output_file = match OpenOptions::new()
                                            .write(true)
                                            .append(true) 
                                            .create(true)
                                            .open(&output_fp) {
            Ok(f) => f,
            Err(e) => {
                panic!("Couldn't create output file: {e}");
            }
        };

        let compressor = FileCompressor::new(
            &FileCompressSettings {
                chunk_size_hint: chunk_size_hint,
                compression_level: compression_level.clone()
            }
        );

        let compressor_pipeline = FileCompressPipeline {
            input: &mut input_file,
            output: &mut output_file
        };

        compressor.decompress_input_to_output(compressor_pipeline);
        drop(input_file);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::core::file::fileanalyze::FileAnalyzerSettings;
    use crate::core::file::filecompress::CompressorLevel;
    use std::fs;
    use crate::core::file::filecompress::tests::random_byte_data_file;
    use tempfile::NamedTempFile;
    use std::io::{
        Read, 
        SeekFrom,
        Seek
    };
    
    fn create_test_file(data: &[u8]) -> NamedTempFile {
        let mut file = NamedTempFile::new().expect("Failed to create temporary file");

        file.write_all(data).expect("Failed to write test data");
        file.flush().expect("Failed to flush test data");
        file
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

        let input = create_test_file(&input_data);
        let input_fp = input.path().to_path_buf();

        let output = NamedTempFile::new().expect("Failed to create temporary file");
        let output_fp = output.path().to_path_buf();

        let pipeline = FileFormaterPipeline {
            input_fp: input_fp.clone(),
            output_fp: output_fp.clone(),
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
    }

    #[test]
    fn test_file_compress_with_custom_hints() {
        let input_data = vec![0x01, 0x02, 0x03, 0x04, 0x05, 0x06, 0x07, 0x08];

        let input = create_test_file(&input_data);
        let input_fp = input.path().to_path_buf();

        let output = NamedTempFile::new().expect("Failed to create temporary file");
        let output_fp = output.path().to_path_buf();

        let pipeline = FileFormaterPipeline {
            input_fp: input_fp.clone(),
            output_fp: output_fp.clone(),
        };

        let formater = setup_formater(256, 1);
        let custom_hints = Some(FileFormaterHints { chunk_size_hint: 1024 });

        formater.file_compress(&pipeline, custom_hints);

        let metadata = fs::metadata(&output_fp).expect("Output file was not created");
        assert!(metadata.len() > 0);
    }

    #[test]
    #[should_panic]
    fn test_file_compress_panic_on_non_existent_input() {
        let tmp_input_file = NamedTempFile::new().unwrap();
        let tmp_output_file = NamedTempFile::new().unwrap();

        let tmp_input_fp = tmp_input_file.path().to_path_buf();
        let tmp_output_fp = tmp_output_file.path().to_path_buf();

        let pipeline = FileFormaterPipeline {
            input_fp: tmp_input_fp,
            output_fp: tmp_output_fp,
        };

        let formater = setup_formater(512, 2);
        formater.file_compress(&pipeline, None);
    }

    #[test]
    fn test_file_compress_decompress() -> std::io::Result<()> {        
        let tmp_input_file = NamedTempFile::new()?;
        let mut tmp_output_file = NamedTempFile::new()?;
        let mut tmp_final_output_file = NamedTempFile::new()?;

        let tmp_input_fp = tmp_input_file.path().to_path_buf();
        let tmp_output_fp = tmp_output_file.path().to_path_buf();
        let tmp_final_output_fp = tmp_final_output_file.path().to_path_buf();

        let (_compress_output, decompress_output)= 
            random_byte_data_file(tmp_input_fp.clone()).unwrap();
            
        let analyzer_settings = FileAnalyzerSettings {
            probe_chunk_size: 2048,
            thread_cnt: 2
        };

        let analyzer= FileAnalyzer::new(&analyzer_settings);

        let tmp_output = tmp_output_file.as_file_mut();
        let tmp_final_output = tmp_final_output_file.as_file_mut();

        let formater = FileFormater::new(&analyzer);
        let formater_hints = FileFormaterHints {
            chunk_size_hint: 2048
        };
        
        formater.file_compress(
            &FileFormaterPipeline { 
                input_fp: tmp_input_fp.clone(), 
                output_fp: tmp_output_fp.clone()
            }, 
            Some(formater_hints.clone())
        );

        tmp_output.seek(SeekFrom::Start(0))?;

        formater.file_decompress(
            &FileFormaterPipeline { 
                input_fp: tmp_output_fp.clone(), 
                output_fp: tmp_final_output_fp .clone()
            }, 
            Some(formater_hints)
        );

        tmp_final_output.seek(SeekFrom::Start(0))?;
        let mut result = Vec::new();

        let _ = tmp_final_output.read_to_end(&mut result);

        assert_eq!(result, decompress_output);

        Ok(())
    }
}
