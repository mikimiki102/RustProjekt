//! File: fileanalyze.rs.
//!
//! Provides [`FileAnalyzer`] structure for analyzing file structure
//! and finally suggesting efficient compression for input file.
//!
//! See [`FileAnalyzer::get_suggested_compression`] for obtaining suggested compression.

use super::filecompress::CompressorLevel;
use crate::get_bit_u8;
use std::fs::{self, File};
use std::io::{Read, Seek, SeekFrom};
use std::path::PathBuf;
use std::sync::atomic::{AtomicU32, Ordering};

/// Specify settings of [`FileAnalyzer`]
#[derive(Debug, Clone)]
pub struct FileAnalyzerSettings {
    pub probe_chunk_size: usize,
    pub thread_cnt: usize,
}

#[derive(Debug, Clone)]
pub struct FileAnalyzer {
    pub settings: FileAnalyzerSettings,
}

impl FileAnalyzer {
    pub fn new(settings: &FileAnalyzerSettings) -> Self {
        if settings.probe_chunk_size == 0 || settings.thread_cnt == 0 {
            panic!("Invalid settings for FileAnalyzer");
        }

        Self {
            settings: settings.clone(),
        }
    }

    fn get_byte_compression_coeff(&self, probe_chunk: &[u8]) -> f32 {
        let thread_cnt = self.settings.thread_cnt;
        let size_per_thread = probe_chunk.len() / thread_cnt;
        let total_cluster_cnt = AtomicU32::new(1);

        std::thread::scope(|s| {
            let mut handles = Vec::new();

            for i in 0..thread_cnt {
                let begin = i * size_per_thread;
                let end = std::cmp::min(i * size_per_thread + size_per_thread, probe_chunk.len());
                let chunk_slice = &probe_chunk[begin..end];

                let handle = s.spawn(|| {
                    let mut curr_byte = chunk_slice[0];

                    for i in 1..chunk_slice.len() {
                        let byte = chunk_slice[i];

                        if byte != curr_byte {
                            curr_byte = byte;
                            total_cluster_cnt.fetch_add(1, Ordering::Relaxed);
                        }
                    }
                }); // s.spawn

                handles.push(handle);
            }
        }); // std::thread::scope

        let total_cluster_cnt = total_cluster_cnt.load(Ordering::Acquire);
        probe_chunk.len() as f32 / total_cluster_cnt as f32
    }

    fn get_bit_compression_coeff(&self, probe_chunk: &[u8]) -> f32 {
        let thread_cnt = self.settings.thread_cnt;
        let size_per_thread = probe_chunk.len() / thread_cnt;
        let total_cluster_cnt = AtomicU32::new(1);

        std::thread::scope(|s| {
            let mut handles = Vec::new();

            for i in 0..thread_cnt {
                let begin = i * size_per_thread;
                let end = std::cmp::min(i * size_per_thread + size_per_thread, probe_chunk.len());
                let chunk_slice = &probe_chunk[begin..end];

                let handle = s.spawn(|| {
                    let mut curr_bit = get_bit_u8!(chunk_slice[0], 0);

                    for i in 0..chunk_slice.len() {
                        let byte = chunk_slice[i];

                        for shf in 0..8 {
                            let bit = get_bit_u8!(byte, shf);

                            if bit != curr_bit {
                                curr_bit = bit;
                                total_cluster_cnt.fetch_add(1, Ordering::Relaxed);
                            }
                        }
                    }
                }); // s.spawn

                handles.push(handle);
            }
        }); // std::thread::scope

        let total_cluster_cnt = total_cluster_cnt.load(Ordering::Acquire);
        let total_bit_cnt = probe_chunk.len() * 8;
        total_bit_cnt as f32 / total_cluster_cnt as f32
    }

    /// Returns wrapped `Some` [`CompressorLevel`] for given filepath `fp`.
    /// When error occured, returns `None`.
    pub fn get_suggested_compression(
        &self,
        fp: &PathBuf,
    ) -> std::io::Result<Option<CompressorLevel>> {
        let mut file = File::open(fp)?;

        let total_file_size = fs::metadata(fp)?.len() as usize;

        if total_file_size == 0 {
            return Ok(None);
        }

        let probe_chunk_size = 
            std::cmp::max(self.settings.probe_chunk_size, total_file_size);

        let mut probe_chunk = vec![0u8; probe_chunk_size];
        let bytes_read = file.read(&mut probe_chunk)?;
        probe_chunk.truncate(bytes_read);

        let byte_compression_coeff = self.get_byte_compression_coeff(&probe_chunk);
        let bit_compression_coeff = self.get_bit_compression_coeff(&probe_chunk);

        file.seek(SeekFrom::Start(0))?;

        if byte_compression_coeff > bit_compression_coeff {
            return Ok(Some(CompressorLevel::CompressorByteLevel));
        } else {
            return Ok(Some(CompressorLevel::CompressorBitLevel));
        }
    }

}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Write;
    use tempfile::NamedTempFile;

    fn create_test_file(data: &[u8]) -> NamedTempFile {
        let mut file = NamedTempFile::new().expect("Failed to create temporary file");

        file.write_all(data).expect("Write-all error");
        file.flush().expect("Flushing error");
        file
    }

    fn get_test_settings(probe_size: usize, threads: usize) -> FileAnalyzerSettings {
        FileAnalyzerSettings {
            probe_chunk_size: probe_size,
            thread_cnt: threads,
        }
    }

    #[test]
    fn test_analyzer_new_valid_settings() {
        let settings = get_test_settings(1024, 4);
        let analyzer = FileAnalyzer::new(&settings);
        assert_eq!(analyzer.settings.probe_chunk_size, 1024);
        assert_eq!(analyzer.settings.thread_cnt, 4);
    }

    #[test]
    #[should_panic(expected = "Invalid settings for FileAnalyzer")]
    fn test_analyzer_new_panic_on_zero_probe_size() {
        let settings = get_test_settings(0, 4);
        FileAnalyzer::new(&settings);
    }

    #[test]
    #[should_panic(expected = "Invalid settings for FileAnalyzer")]
    fn test_analyzer_new_panic_on_zero_threads() {
        let settings = get_test_settings(1024, 0);
        FileAnalyzer::new(&settings);
    }

    #[test]
    fn test_suggests_byte_level_for_long_byte_repeats() {
        let data = vec![0b1010_1010u8; 100];

        let file = create_test_file(&data);
        let fp = file.path().to_path_buf();
        let settings = get_test_settings(512, 2);
        let analyzer = FileAnalyzer::new(&settings);

        let result = analyzer.get_suggested_compression(&fp).unwrap();
        let _ = std::fs::remove_file(&fp);

        assert_eq!(result, Some(CompressorLevel::CompressorByteLevel));
    }

    #[test]
    fn test_suggests_bit_level_for_alternating_bytes_with_matching_bits() {
        let mut data = Vec::new();
        for _ in 0..50 {
            data.push(0b0000_0000);
            data.push(0b0000_1111);
        }

        let file = create_test_file(&data);
        let fp = file.path().to_path_buf();
        let settings = get_test_settings(512, 2);
        let analyzer = FileAnalyzer::new(&settings);

        let result = analyzer.get_suggested_compression(&fp).unwrap();
        let _ = std::fs::remove_file(&fp);

        assert_eq!(result, Some(CompressorLevel::CompressorBitLevel));
    }

    #[test]
    fn test_handling_empty_file() {
        let file = create_test_file(&[]);
        let fp = file.path().to_path_buf();
        let settings = get_test_settings(1024, 2);
        let analyzer = FileAnalyzer::new(&settings);

        let result = analyzer.get_suggested_compression(&fp).unwrap();
        let _ = std::fs::remove_file(&fp);

        assert_eq!(result, None);
    }

    #[test]
    fn test_file_smaller_than_probe_chunk_size() {
        let file = create_test_file(&[1, 1, 2, 2]);
        let fp = file.path().to_path_buf();
        let settings = get_test_settings(1000, 2);
        let analyzer = FileAnalyzer::new(&settings);

        let result = analyzer.get_suggested_compression(&fp);
        let _ = std::fs::remove_file(&fp);

        assert!(result.is_ok());
    }

    #[test]
    fn test_invalid_file_path_returns_error() {
        let settings = get_test_settings(1024, 2);
        let analyzer = FileAnalyzer::new(&settings);

        let result =
            analyzer.get_suggested_compression(&PathBuf::from("assets/non_existent_file_12345"));

        assert!(result.is_err());
        assert_eq!(result.unwrap_err().kind(), std::io::ErrorKind::NotFound);
    }
}
