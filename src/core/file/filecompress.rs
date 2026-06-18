//! File: filecompress.rs.
//!
//! Provides [`FileCompressor`] structure for either compressing
//! or decompressing given input file to output file.
//! Input and output files are specified via [`FileCompressPipeline`].

use crate::get_bit_cnt_u8;
use std::fs::File;
use std::io::{BufWriter, ErrorKind, Read, Seek, SeekFrom, Write};
use std::sync::mpsc;

fn get_bytes_left(file: &mut File) -> std::io::Result<u64> {
    let current_position = file.stream_position()?;
    let total_size = file.seek(SeekFrom::End(0))?;
    file.seek(SeekFrom::Start(current_position))?;
    let bytes_left = total_size - current_position;
    Ok(bytes_left)
}

/// Compressor mode specifier.
/// Compressing either bits, or bytes.
#[derive(Clone, Debug, PartialEq)]
pub enum CompressorLevel {
    CompressorBitLevel,
    CompressorByteLevel,
}

/// Differentiate between different
/// internal functions while parsing and compressing a file.
impl CompressorLevel {
    pub fn get_compressor_fn(&self) -> fn(&[u8]) -> Vec<u8> {
        match self {
            CompressorLevel::CompressorBitLevel => crate::core::memcompress::bit_level_compress,

            CompressorLevel::CompressorByteLevel => crate::core::memcompress::byte_level_compress,
        }
    }

    pub fn get_decompressor_fn(&self) -> fn(&[u8]) -> Result<Vec<u8>, &str> {
        match self {
            CompressorLevel::CompressorBitLevel => crate::core::memcompress::bit_level_decompress,

            CompressorLevel::CompressorByteLevel => crate::core::memcompress::byte_level_decompress,
        }
    }

    pub fn get_reader_fn(
        &self,
    ) -> fn(&mut File, &FileCompressSettings) -> std::io::Result<Option<Vec<u8>>> {
        match self {
            CompressorLevel::CompressorBitLevel => FileCompressor::aligned_bit_buf_reader,

            CompressorLevel::CompressorByteLevel => FileCompressor::byte_buf_reader,
        }
    }

    pub fn get_marker_value(&self) -> u8 {
        match self {
            CompressorLevel::CompressorBitLevel => 0x00u8,

            CompressorLevel::CompressorByteLevel => 0xFFu8,
        }
    }

    pub fn from_value(v: u8) -> Option<Self> {
        if v == CompressorLevel::CompressorBitLevel.get_marker_value() {
            return Some(CompressorLevel::CompressorBitLevel);
        } else if v == CompressorLevel::CompressorByteLevel.get_marker_value() {
            return Some(CompressorLevel::CompressorByteLevel);
        }

        None
    }


    pub fn to_str(&self) -> &'static str {
        match self {
            CompressorLevel::CompressorByteLevel => "byte-level",
            CompressorLevel::CompressorBitLevel => "bit-level",
        }
    }
}

/// Specify input file and output file for the compressor.
#[derive(Debug)]
pub struct FileCompressPipeline<'a> {
    pub input: &'a mut File,
    pub output: &'a mut File,
}

/// Specify compressing settings.
/// Increasing `chunk_size_hint` might increase performance
/// while compressing big files, but setting it to high value
/// might be an overkill for just small files.
#[derive(Debug, Clone)]
pub struct FileCompressSettings {
    pub chunk_size_hint: usize,
    pub compression_level: CompressorLevel,
}

/// That is core compressor.
/// It implements `compress_input_to_output`, that do the actual stuff.
#[derive(Debug, Clone)]
pub struct FileCompressor {
    pub settings: FileCompressSettings,
}

pub trait BuffReaderTrait:
    FnMut(&mut File, &FileCompressSettings) -> std::io::Result<Option<Vec<u8>>> + Send + 'static
{
}

pub trait BuffTransformTrait: Fn(&[u8]) -> Vec<u8> + Send + Sync + 'static {}

impl<T> BuffReaderTrait for T where
    T: FnMut(&mut File, &FileCompressSettings) -> std::io::Result<Option<Vec<u8>>> + Send + 'static
{
}

impl<T> BuffTransformTrait for T where T: Fn(&[u8]) -> Vec<u8> + Send + Sync + 'static {}

impl FileCompressor {
    pub fn new(settings: &FileCompressSettings) -> Self {
        if settings.chunk_size_hint < 2
            && settings.compression_level == CompressorLevel::CompressorByteLevel
        {
            panic!("Minimum chunk size for byte-level compression is 2");
        }

        Self {
            settings: settings.clone(),
        }
    }

    pub fn aligned_bit_buf_reader(
        file: &mut File,
        settings: &FileCompressSettings,
    ) -> std::io::Result<Option<Vec<u8>>> {
        let chunk_size = settings.chunk_size_hint;
        let mut buffer = Vec::with_capacity(chunk_size);
        let mut total_bit_cnt: usize = 0;
        let mut one_byte_buf = [0u8; 1];

        loop {
            match file.read_exact(&mut one_byte_buf) {
                Ok(()) => {
                    let byte = one_byte_buf[0];
                    total_bit_cnt += get_bit_cnt_u8!(byte) as usize;

                    buffer.push(byte);

                    if total_bit_cnt > 0
                        && total_bit_cnt % 8 == 0
                        && total_bit_cnt / 8 >= chunk_size
                    {
                        return Ok(Some(std::mem::take(&mut buffer)));
                    }
                }
                Err(e) => {
                    if e.kind() == ErrorKind::UnexpectedEof {
                        if !buffer.is_empty() {
                            return Ok(Some(std::mem::take(&mut buffer)));
                        }
                        return Ok(None);
                    }
                    return Err(e);
                }
            }
        }
    }

    pub fn byte_buf_reader(
        file: &mut File,
        settings: &FileCompressSettings,
    ) -> std::io::Result<Option<Vec<u8>>> {
        let bytes_to_read = get_bytes_left(file)? as usize;
        let chunk_size = settings.chunk_size_hint;
        let buffer_size = std::cmp::min(bytes_to_read, chunk_size);
        let mut buffer = Vec::with_capacity(buffer_size);

        match file.take(chunk_size as u64).read_to_end(&mut buffer) {
            Ok(bytes_read) => {
                if bytes_read == 0 {
                    return Ok(None);
                }

                if bytes_read < chunk_size {
                    buffer.truncate(bytes_read);
                }

                return Ok(Some(buffer));
            }
            Err(e) => {
                eprintln!("Error while reading from file: {e}");
                Err(e)
            }
        }
    }

    fn stream_input_to_output(
        &self,
        pipeline: FileCompressPipeline<'_>,
        mut buf_read_policy: impl BuffReaderTrait,
        transform_policy: impl BuffTransformTrait,
    ) {
        let read_chunk_channel_size = 8;

        let (reader_sender, reader_receiver) =
            mpsc::sync_channel::<Vec<u8>>(read_chunk_channel_size);
        let (reader_msg_sender, reader_msg_receiver) = mpsc::sync_channel::<bool>(1);
        
        // Here, we read chunks of data
        // and then send them to another threads.
        let settings = self.settings.clone();

        let file = pipeline.input;

        std::thread::scope(|s| {
            s.spawn(move || {
                // Sequentially read next buffers with 'buf_read_policy'
                while !reader_msg_receiver.try_recv().unwrap_or(false) {
                    match buf_read_policy(file, &settings) {
                        Ok(Some(buffer)) => {
                            if reader_sender.send(buffer).is_err() {
                                break;
                            }
                        }
                        Ok(None) => {
                            break;
                        }
                        Err(e) => {
                            eprintln!("Error while reading from file: {e}");
                            break;
                        }
                    }
                }
            }); // s.spawn

            let transform_chunk_channel_size = 8;

            let (transform_sender, transform_receiver) =
                mpsc::sync_channel::<Vec<u8>>(transform_chunk_channel_size);
            let (transform_msg_sender, transform_msg_receiver) = mpsc::sync_channel::<bool>(1);

            // Here, we transform thread according to policy function.
            s.spawn(move || {
                // Probe raw chunk buffer
                while let Ok(chunk_buffer) = reader_receiver.recv() {
                    if transform_msg_receiver.try_recv().unwrap_or(false) {
                        let _ = reader_msg_sender.send(true);
                        break;
                    }

                    // Decompress raw chunk buffer using provided policy function
                    let transform_buffer = transform_policy(&chunk_buffer);

                    if transform_sender.send(transform_buffer).is_err() {
                        let _ = reader_msg_sender.send(true);
                        break;
                    }
                }
            }); // s.spawn

            // We take care of writing to file in main thread.
            let output_file = pipeline.output;

            let mut writer = BufWriter::new(output_file);

            while let Ok(compressed_buffer) = transform_receiver.recv() {
                if let Err(e) = writer.write_all(&compressed_buffer) {
                    eprintln!("Error while writing to file: {e}");
                    break;
                }
            }

            let _ = transform_msg_sender.send(true);
            let _ = writer.flush();
        }); // std::thread::scope
    }

    pub fn compress_input_to_output(&self, pipeline: FileCompressPipeline<'_>) {
        let compress_fn = self.settings.compression_level.get_compressor_fn();
        let buf_read_fn = self.settings.compression_level.get_reader_fn();

        self.stream_input_to_output(pipeline, buf_read_fn, compress_fn);
    }

    pub fn decompress_input_to_output(&self, pipeline: FileCompressPipeline<'_>) {
        let compression_level = self.settings.compression_level.clone();
        let buf_read_fn = self.settings.compression_level.get_reader_fn();
        let compress_fn = move |buffer: &[u8]| {
            let decompress_fn = compression_level.get_decompressor_fn();
            decompress_fn(buffer).unwrap()
        };

        self.stream_input_to_output(pipeline, buf_read_fn, compress_fn);
    }
}

#[cfg(test)]
pub mod tests {
    use super::*;
    use crate::{bit_cluster_u8, get_bit_u8, get_repr_bit_u8};
    use rand::{
        Rng,
        distr::{Distribution, Uniform},
    };
    use std::fs::File;
    use std::fs::OpenOptions;
    use std::path::PathBuf;

    fn byte_compr_collapsed_eq(v0: &[u8], v1: &[u8]) -> bool {
        if v0.is_empty() ^ v1.is_empty() {
            return false;
        } else if v0.is_empty() && v1.is_empty() {
            return true;
        } else if v0.len() % 2 > 0 || v1.len() % 2 > 0 {
            panic!("v0 and v1 are required to have even length");
        }

        let mut i0: usize = 0;
        let mut i1: usize = 0;

        while i0 < v0.len() && i1 < v1.len() {
            let v0_byte = v0[i0 + 1];
            let v1_byte = v1[i1 + 1];

            if v0_byte != v1_byte {
                return false;
            }

            let mut v0_cnt: usize = 0;
            let mut v1_cnt: usize = 0;

            while i0 < v0.len() && v0[i0 + 1] == v0_byte {
                v0_cnt += v0[i0] as usize;
                i0 += 2;
            }

            while i1 < v1.len() && v1[i1 + 1] == v1_byte {
                v1_cnt += v1[i1] as usize;
                i1 += 2;
            }

            if v0_cnt != v1_cnt {
                return false;
            }
        }

        i0 == v0.len() && i1 == v1.len()
    }

    fn bit_compr_collapsed_eq(v0: &[u8], v1: &[u8]) -> bool {
        if v0.is_empty() ^ v1.is_empty() {
            return false;
        } else if v0.is_empty() && v1.is_empty() {
            return true;
        }

        let mut i0: usize = 0;
        let mut i1: usize = 0;

        while i0 < v0.len() && i1 < v1.len() {
            let v0_bit = get_repr_bit_u8!(v0[i0]);
            let v1_bit = get_repr_bit_u8!(v1[i1]);

            if v0_bit != v1_bit {
                return false;
            }

            let mut v0_cnt: usize = 0;
            let mut v1_cnt: usize = 0;

            while i0 < v0.len() && get_repr_bit_u8!(v0[i0]) == v0_bit {
                v0_cnt += get_bit_cnt_u8!(v0[i0]) as usize;
                i0 += 1;
            }

            while i1 < v1.len() && get_repr_bit_u8!(v1[i1]) == v1_bit {
                v1_cnt += get_bit_cnt_u8!(v1[i1]) as usize;
                i1 += 1;
            }

            if v0_cnt != v1_cnt {
                return false;
            }
        }

        i0 == v0.len() && i1 == v1.len()
    }

    pub fn random_byte_data_file(fp: PathBuf) -> std::io::Result<(Vec<u8>, Vec<u8>)> {
        let mut input_file = File::create(fp.clone())?;

        // Write random data to input_file
        let mut rng = rand::rng();
        let range = Uniform::new(1, 16).unwrap();

        let cluster_cnts = 4096;

        let random_cnts: Vec<u8> = (0..cluster_cnts)
            .map(|_| range.sample(&mut rng) as u8)
            .collect();
        let mut random_bytes = vec![0u8; cluster_cnts];

        rng.fill_bytes(&mut random_bytes);

        let mut compress_output: Vec<u8> = Vec::with_capacity(2 * cluster_cnts);
        let mut decompress_output = Vec::with_capacity(cluster_cnts);

        // Write data to tmp_output_file
        for i in 0..cluster_cnts {
            let byte = random_bytes[i];
            let cnt = random_cnts[i];

            let chunk = vec![byte; cnt as usize];
            input_file.write_all(&chunk)?;

            compress_output.push(cnt);
            compress_output.push(byte);

            decompress_output.extend(chunk);
        }

        input_file.flush()?;
        drop(input_file);

        Ok((compress_output, decompress_output))
    }

    fn random_bit_data_file(fp: PathBuf) -> std::io::Result<(Vec<u8>, Vec<u8>)> {
        let mut input_file = File::create(fp.clone())?;

        let mut rng = rand::rng();
        let cluster_cnts = 4096;

        let mut compress_output = vec![0u8; cluster_cnts];
        rng.fill_bytes(&mut compress_output);

        let mut total_bit_cnt = 0;
        compress_output
            .iter()
            .for_each(|&x| total_bit_cnt += get_bit_cnt_u8!(x) as usize);

        let fill_bit_cnt = 8 - (total_bit_cnt % 8) as u8;

        if fill_bit_cnt > 0 {
            // create filler cluster to assert we got
            // file data size that is multiple of a byte.
            let fill_bit = bit_cluster_u8!(fill_bit_cnt, 1);
            compress_output.push(fill_bit);
        }

        // Remove bytes where count of bits is zero
        compress_output.retain(|&x| get_bit_cnt_u8!(x) > 0);

        // Write data to tmp_input_file
        let mut decompress_output = Vec::new();
        let mut curr_byte = 0u8;
        let mut curr_shf = 0u8;

        for &cluster in &compress_output {
            let cnt = get_bit_cnt_u8!(cluster);
            let bit = get_repr_bit_u8!(cluster);

            for _ in 0..cnt {
                curr_byte |= bit << curr_shf;
                curr_shf += 1;

                if curr_shf >= 8 {
                    decompress_output.push(curr_byte);
                    curr_byte = 0u8;
                    curr_shf = 0u8;
                }
            }
        }

        if curr_shf > 0 {
            decompress_output.push(curr_byte);
        }

        input_file.write_all(&decompress_output)?;
        input_file.flush()?;

        drop(input_file);

        Ok((compress_output, decompress_output))
    }

    #[test]
    fn test_file_byte_level_compress() -> std::io::Result<()> {
        let mut tmp_input_fp = PathBuf::from("assets");
        tmp_input_fp.push("tmp_bit_input_fp");

        let mut tmp_output_fp = PathBuf::from("assets");
        tmp_output_fp.push("tmp_bit_output_fp");

        let (expect_output, _) = random_byte_data_file(tmp_input_fp.clone()).unwrap();

        let settings = FileCompressSettings {
            chunk_size_hint: 2048,
            compression_level: CompressorLevel::CompressorByteLevel,
        };

        let compressor = FileCompressor::new(&settings);

        let mut tmp_input = File::open(tmp_input_fp.clone())?;
        let mut tmp_output = OpenOptions::new()
            .read(true)
            .write(true)
            .create(true)
            .truncate(true)
            .open(&tmp_output_fp)?;

        // Actual compression
        compressor.compress_input_to_output(FileCompressPipeline {
            input: &mut tmp_input,
            output: &mut tmp_output,
        });

        // Check compression
        tmp_output.seek(SeekFrom::Start(0))?;

        let mut file_output = Vec::new();

        tmp_output.read_to_end(&mut file_output)?;

        drop(tmp_input);
        drop(tmp_output);

        // Delete temporary files
        let _ = std::fs::remove_file(&tmp_input_fp)?;
        let _ = std::fs::remove_file(&tmp_output_fp)?;

        assert!(
            byte_compr_collapsed_eq(&file_output, &expect_output),
            "Byte-level compression is invalid"
        );

        Ok(())
    }

    #[test]
    fn test_file_bit_level_compress() -> std::io::Result<()> {
        let mut tmp_input_fp = PathBuf::from("assets");
        tmp_input_fp.push("tmp_byte_input_fp");

        let mut tmp_output_fp = PathBuf::from("assets");
        tmp_output_fp.push("tmp_byte_output_fp");

        let (compress_output, _decompess_output) =
            random_bit_data_file(tmp_input_fp.clone()).unwrap();

        let settings = FileCompressSettings {
            chunk_size_hint: 2048,
            compression_level: CompressorLevel::CompressorBitLevel,
        };

        let compressor = FileCompressor::new(&settings);

        let mut tmp_input = File::open(tmp_input_fp.clone())?;
        let mut tmp_output = OpenOptions::new()
            .read(true)
            .write(true)
            .create(true)
            .truncate(true)
            .open(&tmp_output_fp)?;

        // Actual compression
        compressor.compress_input_to_output(FileCompressPipeline {
            input: &mut tmp_input,
            output: &mut tmp_output,
        });

        // Check compression
        let mut compressed_file = File::open(&tmp_output_fp)?;
        let mut file_output = Vec::new();

        compressed_file.read_to_end(&mut file_output)?;
        drop(compressed_file);

        assert!(
            bit_compr_collapsed_eq(&file_output, &compress_output),
            "Bit-level compression is invalid"
        );

        // Delete temporary files
        let _ = std::fs::remove_file(&tmp_input_fp)?;
        let _ = std::fs::remove_file(&tmp_output_fp)?;

        Ok(())
    }

    #[test]
    fn test_byte_level_compress_decompress() -> std::io::Result<()> {
        let mut tmp_input_fp = PathBuf::from("assets");
        tmp_input_fp.push("tmp_byte_input_fp_cd");

        let mut tmp_output_fp = PathBuf::from("assets");
        tmp_output_fp.push("tmp_byte_output_fp_cd");

        let mut tmp_final_output_fp = PathBuf::from("assets");
        tmp_final_output_fp.push("tmp_byte_final_output_fp_cd");

        let (_compress_output, decompress_output) =
            random_byte_data_file(tmp_input_fp.clone()).unwrap();

        let settings = FileCompressSettings {
            chunk_size_hint: 2048,
            compression_level: CompressorLevel::CompressorByteLevel,
        };

        let compressor = FileCompressor::new(&settings);

        let mut tmp_input = File::open(tmp_input_fp.clone())?;
        let mut tmp_output = OpenOptions::new()
            .read(true)
            .write(true)
            .create(true)
            .truncate(true)
            .open(tmp_output_fp.clone())?;
        let mut tmp_final_output = OpenOptions::new()
            .read(true)
            .write(true)
            .create(true)
            .truncate(true)
            .open(&tmp_final_output_fp)?;

        compressor.compress_input_to_output(FileCompressPipeline {
            input: &mut tmp_input,
            output: &mut tmp_output,
        });

        tmp_output.seek(SeekFrom::Start(0))?;

        compressor.decompress_input_to_output(FileCompressPipeline {
            input: &mut tmp_output,
            output: &mut tmp_final_output,
        });

        tmp_final_output.seek(SeekFrom::Start(0))?;
        let mut result = Vec::new();

        let _ = tmp_final_output.read_to_end(&mut result);
        drop(tmp_final_output);

        let _ = std::fs::remove_file(&tmp_input_fp);
        let _ = std::fs::remove_file(&tmp_output_fp);
        let _ = std::fs::remove_file(&tmp_final_output_fp);

        assert_eq!(result, decompress_output);

        Ok(())
    }

    #[test]
    fn test_bit_level_compress_decompress() -> std::io::Result<()> {
        let mut tmp_input_fp = PathBuf::from("assets");
        tmp_input_fp.push("tmp_bit_input_fp_cd");

        let mut tmp_output_fp = PathBuf::from("assets");
        tmp_output_fp.push("tmp_bit_output_fp_cd");

        let mut tmp_final_output_fp = PathBuf::from("assets");
        tmp_final_output_fp.push("tmp_bit_final_output_fp_cd");

        let (_compress_output, decompress_output) =
            random_bit_data_file(tmp_input_fp.clone()).unwrap();

        let settings = FileCompressSettings {
            chunk_size_hint: 2048,
            compression_level: CompressorLevel::CompressorBitLevel,
        };

        let compressor = FileCompressor::new(&settings);

        let mut tmp_input = File::open(tmp_input_fp.clone())?;
        let mut tmp_output = OpenOptions::new()
            .read(true)
            .write(true)
            .create(true)
            .truncate(true)
            .open(tmp_output_fp.clone())?;
        let mut tmp_final_output = OpenOptions::new()
            .read(true)
            .write(true)
            .create(true)
            .truncate(true)
            .open(&tmp_final_output_fp)?;

        compressor.compress_input_to_output(FileCompressPipeline {
            input: &mut tmp_input,
            output: &mut tmp_output,
        });

        tmp_output.seek(SeekFrom::Start(0))?;

        compressor.decompress_input_to_output(FileCompressPipeline {
            input: &mut tmp_output,
            output: &mut tmp_final_output,
        });

        let mut input = File::open(&tmp_final_output_fp)?;
        let mut result = Vec::new();

        let _ = input.read_to_end(&mut result);
        drop(input);

        assert_eq!(result, decompress_output);

        let _ = std::fs::remove_file(&tmp_input_fp);
        let _ = std::fs::remove_file(&tmp_output_fp);
        let _ = std::fs::remove_file(&tmp_final_output_fp);

        Ok(())
    }
}
