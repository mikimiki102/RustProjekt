use std::fs::{self, File};
use std::io::{BufWriter, Read, Write};
use std::sync::{mpsc};
use std::path::PathBuf;

#[derive(Clone, Debug)]
pub enum CompressorLevel {
    CompressorBitLevel,
    CompressorByteLevel,
}

impl CompressorLevel {
    pub fn get_compressor_fn(&self) -> fn(&[u8]) -> Vec<u8> {
        match self {
            CompressorLevel::CompressorBitLevel => crate::core::rlecompress::bit_level_compress,
            CompressorLevel::CompressorByteLevel => crate::core::rlecompress::byte_level_compress,
        }
    }
}

/* Specify input file and output file for the compressor.
 */

#[derive(Debug, Clone)]
pub struct FileCompressPipeline {
    pub input: PathBuf,
    pub output: PathBuf,
}

/* Specify compressing settings.
 * Increasing 'chunk_size' might increase performance
 * while compressing big files, but setting it to high value
 * might be an overkill for just small files.
 * 'compressor_level' stands for compression mode, either bit or byte compression.
 */

#[derive(Debug, Clone)]
pub struct FileCompressSettings {
    pub chunk_size: usize,
    pub compression_level: CompressorLevel,
}

/* That is core compressor.
 * It implements 'compress_input_to_output', that do the actual stuff.
 */

#[derive(Debug)]
pub struct FileCompressor {
    pub pipeline: FileCompressPipeline,
    pub settings: FileCompressSettings,  
}

impl FileCompressor {
    pub fn new(pipeline: &FileCompressPipeline, settings: &FileCompressSettings) -> Self {
        Self {
            pipeline: pipeline.clone(),
            settings: settings.clone()
        }
    }

    pub fn input_file(&self) -> &PathBuf {
        &self.pipeline.input
    }

    pub fn output_file(&self) -> &PathBuf {
        &self.pipeline.output
    }

    pub fn compress_input_to_output(&self) {
        let input_path = self.pipeline.input.clone();
        let output_path = &self.pipeline.output;
        let chunk_size = self.settings.chunk_size;
        let compress_fn = self.settings.compression_level.get_compressor_fn();

        let read_chunk_channel_size = 8;

        let (reader_sender, reader_receiver) = 
            mpsc::sync_channel::<Vec<u8>>(read_chunk_channel_size );
        let (reader_msg_sender, reader_msg_receiver) = mpsc::sync_channel::<bool>(1);
        
        let mut bytes_to_read = fs::metadata(input_path.clone()).unwrap().len() as usize;

        // Reading thread.
        // Here, we read chunks of data
        // and then send them to another thread for final compressing.
        std::thread::spawn(move || {
            
            // Open input file and handle errors
            let mut file = match File::open(&input_path) {
                Ok(f) => f,
                Err(e) => {
                    eprintln!("Couldn't open input file: {e}");
                    return;
                }
            };

            while !reader_msg_receiver.try_recv().unwrap_or(false) {
                // Create zeroed buffer and read chunk from file.
                let buffer_size = std::cmp::min(bytes_to_read, chunk_size);
                let mut buffer = Vec::with_capacity(buffer_size);

                match (&mut file).take(chunk_size as u64).read_to_end(&mut buffer) {
                    Ok(bytes_read) => {
                        if bytes_read == 0 {
                            break;
                        }

                        if bytes_read < chunk_size {
                            buffer.truncate(bytes_read);
                        }

                        if reader_sender.send(buffer).is_err() {
                            break;
                        }

                        bytes_to_read -= bytes_read;
                    }
                    Err(e) => {
                        eprintln!("Error while reading from file: {e}");
                        break;
                    }
                }
            }
        });

        let compressed_chunk_channel_size = 8;

        let (compressed_sender, compressed_receiver) = 
            mpsc::sync_channel::<Vec<u8>>(compressed_chunk_channel_size );
        let (compressed_msg_sender, compressed_msg_receiver) = mpsc::sync_channel::<bool>(1);

        // Compressing thread.
        // We compress chunks and send them to writer thread.
        std::thread::spawn(move || {

            // Probe raw chunk buffer
            while let Ok(chunk_buffer) = reader_receiver.recv() {
                if compressed_msg_receiver.try_recv().unwrap_or(false) {
                    let _ = reader_msg_sender.send(true);
                    break;
                }

                // Compress raw chunk buffer using 'compress_fn' function
                let compressed_buffer = compress_fn(&chunk_buffer);

                if compressed_sender.send(compressed_buffer).is_err() {
                    let _ = reader_msg_sender.send(true);
                    break;
                }
            }
        });

        // We take care of writing to file in main thread.

        let output_file = match File::create(&output_path) {
            Ok(f) => f,
            Err(e) => {
                eprintln!("Couldn't create output file: {e}");
                let _ = compressed_msg_sender.send(true);
                return;
            }
        };

        let mut writer = BufWriter::new(output_file);

        while let Ok(compressed_buffer) = compressed_receiver.recv() {            
            if let Err(e) = writer.write_all(&compressed_buffer) {
                eprintln!("Error while writing to file: {e}");
                break;
            }
        }

        let _ = compressed_msg_sender.send(true);
        let _ = writer.flush();
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use rand::{
        distr::{Uniform, Distribution}, 
        Rng
    };
    use crate::{get_bit, get_repr_bit, get_bit_cnt};

    fn byte_collapsed_eq(v0: &[u8], v1: &[u8]) -> bool {
        if v0.is_empty() ^ v1.is_empty() {
            return false;
        }
        else if v0.is_empty() && v1.is_empty() {
            return true;
        }
        else if v0.len() % 2 > 0 || v1.len() % 2 > 0 {
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

    fn bit_collapsed_eq(v0: &[u8], v1: &[u8]) -> bool {
        if v0.is_empty() ^ v1.is_empty() {
            return false;
        }
        else if v0.is_empty() && v1.is_empty() {
            return true;
        }

        let mut i0: usize = 0;
        let mut i1: usize = 0;

        while i0 < v0.len() && i1 < v1.len() {
            let v0_bit = get_repr_bit!(v0[i0]);
            let v1_bit = get_repr_bit!(v1[i1]);

            if v0_bit != v1_bit {
                return false;
            }

            let mut v0_cnt: usize = 0;
            let mut v1_cnt: usize = 0;

            while i0 < v0.len() && get_repr_bit!(v0[i0]) == v0_bit {
                v0_cnt += get_bit_cnt!(v0[i0]) as usize;
                i0 += 1;
            }

            while i1 < v1.len() && get_repr_bit!(v1[i1]) == v1_bit {
                v1_cnt += get_bit_cnt!(v1[i1]) as usize;
                i1 += 1;
            }

            if v0_cnt != v1_cnt {
                return false;
            }
        }

        i0 == v0.len() && i1 == v1.len()
    }

    #[test]
    fn test_file_byte_level_compress() -> std::io::Result<()> {
        let mut tmp_input_fp = PathBuf::from("assets");
        tmp_input_fp.push("tmp_input_fp");

        let mut tmp_output_fp = PathBuf::from("assets");
        tmp_output_fp.push("tmp_output_fp");

        let mut tmp_input_file = File::create(tmp_input_fp.clone())?;

        // Write random data to tmp_input_file 
        let mut rng = rand::rng();
        let range = Uniform::new(1, 16).unwrap();

        let cluster_cnts = 16384;

        let random_cnts: Vec<u8> = (0..cluster_cnts).map(|_| range.sample(&mut rng) as u8).collect();
        let mut random_bytes = vec![0u8; cluster_cnts];

        rng.fill_bytes(&mut random_bytes);

        let mut expect_output: Vec<u8> = Vec::with_capacity(2 * cluster_cnts);

        // Write data to tmp_output_file
        for i in 0..cluster_cnts {
            let byte = random_bytes[i];
            let cnt = random_cnts[i];

            let chunk = vec![byte; cnt as usize];
            tmp_input_file.write_all(&chunk)?;

            expect_output.push(cnt);
            expect_output.push(byte);
        }

        tmp_input_file.flush()?;
        drop(tmp_input_file);

        let pipeline = FileCompressPipeline {
            input: tmp_input_fp,
            output: tmp_output_fp,
        };

        let settings = FileCompressSettings {
            chunk_size: 1,
            compression_level: CompressorLevel::CompressorByteLevel,
        };

        let compressor = FileCompressor::new(&pipeline, &settings);

        // Actual compression
        compressor.compress_input_to_output();

        // Check compression
        let mut compressed_file = File::open(&pipeline.output)?;
        let mut file_output = Vec::new();

        compressed_file.read_to_end(&mut file_output)?;
        drop(compressed_file);

        assert!(byte_collapsed_eq(&file_output, &expect_output), 
                "Byte-level compression is invalid");

        // Delete temporary files
        std::fs::remove_file(&pipeline.input)?;
        std::fs::remove_file(&pipeline.output)?;

        Ok(())
    }

    #[test]
    fn test_file_bit_level_compress() -> std::io::Result<()> {
        let mut tmp_input_fp = PathBuf::from("assets");
        tmp_input_fp.push("tmp_input_fp");

        let mut tmp_output_fp = PathBuf::from("assets");
        tmp_output_fp.push("tmp_output_fp");

        let mut tmp_input_file = File::create(tmp_input_fp.clone())?;

        let mut rng = rand::rng();
        let cluster_cnts = 1024;
        
        let mut random_bytes = vec![0u8; cluster_cnts];
        rng.fill_bytes(&mut random_bytes);

        let mut total_bit_cnt = 0;
        random_bytes.iter().for_each(|&x| total_bit_cnt += get_bit_cnt!(x) as usize);

        let fill_bit_cnt = 8 - (total_bit_cnt % 8) as u8;

        if fill_bit_cnt > 0 {
            let fill_bit = fill_bit_cnt | (0x80u8);
            random_bytes.push(fill_bit);
        }

        // Remove bytes where count of bits is zero
        random_bytes.retain(|&x| get_bit_cnt!(x) > 0);

        // Write data to tmp_input_file
        let mut input_data = Vec::new();
        let mut curr_byte = 0u8;
        let mut curr_shf = 0u8;

        for &cluster in &random_bytes {
            let cnt = get_bit_cnt!(cluster);
            let bit = get_repr_bit!(cluster);

            for _ in 0..cnt {
                curr_byte |= bit << curr_shf;
                curr_shf += 1;

                if curr_shf >= 8 {
                    input_data.push(curr_byte);
                    curr_byte = 0u8;
                    curr_shf = 0u8;
                }
            }
        }

        if curr_shf > 0 {
            input_data.push(curr_byte);
        }

        tmp_input_file.write_all(&input_data)?;
        tmp_input_file.flush()?;
        drop(tmp_input_file);

        let pipeline = FileCompressPipeline {
            input: tmp_input_fp,
            output: tmp_output_fp,
        };

        let settings = FileCompressSettings {
            chunk_size: 512,
            compression_level: CompressorLevel::CompressorBitLevel,
        };

        let compressor = FileCompressor::new(&pipeline, &settings);

        // Actual compression
        compressor.compress_input_to_output();

        // Check compression
        let mut compressed_file = File::open(&pipeline.output)?;
        let mut file_output = Vec::new();

        compressed_file.read_to_end(&mut file_output)?;
        drop(compressed_file);

        assert!(bit_collapsed_eq(&file_output, &random_bytes), 
                "Bit-level compression is invalid");

        // Delete temporary files
        std::fs::remove_file(&pipeline.input)?;
        std::fs::remove_file(&pipeline.output)?;

        Ok(())
    }
}
