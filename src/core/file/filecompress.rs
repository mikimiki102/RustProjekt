use std::fs::{self, File};
use std::io::{BufWriter, Read, Write};
use std::sync::{mpsc, Arc};
use std::path::PathBuf;

use crate::get_bit_cnt;

#[derive(Clone, Debug, PartialEq)]
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

    pub fn get_decompressor_fn(&self) -> fn(&[u8]) -> Result<Vec<u8>, &str> {
        match self {
            CompressorLevel::CompressorBitLevel => crate::core::rlecompress::bit_level_decompress,
            CompressorLevel::CompressorByteLevel => crate::core::rlecompress::byte_level_decompress,
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
    pub settings: FileCompressSettings,  
}

impl FileCompressor {
    pub fn new(settings: &FileCompressSettings) -> Self {
        if settings.chunk_size < 2 && 
           settings.compression_level == CompressorLevel::CompressorByteLevel {
           panic!("Minimum chunk size for byte-level compression is 2");
        }

        Self {
            settings: settings.clone()
        }
    }

    fn stream_input_to_output(&self, 
                              pipeline: &FileCompressPipeline, 
                              policy: Arc<dyn Fn(&[u8]) -> Vec<u8> + Send + Sync + 'static>) 
    {
        let input_path = pipeline.input.clone();
        let output_path = &pipeline.output;
        let chunk_size = self.settings.chunk_size;

        let read_chunk_channel_size = 8;

        let (reader_sender, reader_receiver) = 
            mpsc::sync_channel::<Vec<u8>>(read_chunk_channel_size );
        let (reader_msg_sender, reader_msg_receiver) = 
            mpsc::sync_channel::<bool>(1);
        
        let mut bytes_to_read = fs::metadata(input_path.clone()).unwrap().len() as usize;

        // Reading thread.
        // Here, we read chunks of data
        // and then send them to another thread.
        std::thread::spawn(move || {
            /*
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
            }*/
            let mut file = match File::open(&input_path) {
                Ok(f) => f,
                Err(e) => {
                    eprintln!("Couldn't open input file: {e}");
                    return;
                }
            };

            // Tworzymy bufor akumulacyjny oraz licznik bitów
            let mut dynamic_buffer = Vec::new();
            let mut total_bits: usize = 0;
            
            // Tablica pomocnicza do odczytu pojedynczego bajtu z pliku
            let mut single_byte_buf = [0u8; 1];

            while !reader_msg_receiver.try_recv().unwrap_or(false) {
                // Czytamy dokładnie 1 bajt z pliku
                match file.read_exact(&mut single_byte_buf) {
                    Ok(()) => {
                        let byte = single_byte_buf[0];
                        
                        // Dodajemy przeczytany bajt do naszego bufora akumulacyjnego
                        dynamic_buffer.push(byte);
                        
                        // Sumujemy bity za pomocą Twojego makra
                        total_bits += get_bit_cnt!(byte) as usize;

                        // JEŻELI liczba bitów jest podzielna przez 8 i bufor nie jest pusty
                        if total_bits > 0 && total_bits % 8 == 0 {
                            // Tworzymy kopię bufora do wysłania, a istniejący czyścimy (zerujemy alokację)
                            let buffer_to_send = std::mem::take(&mut dynamic_buffer);
                            
                            if reader_sender.send(buffer_to_send).is_err() {
                                break;
                            }
                            
                            // Resetujemy licznik bitów dla nowej paczki danych
                            total_bits = 0;
                        }
                    }
                    Err(e) => {
                        // Jeśli dotarliśmy do końca pliku (EOF), to poprawnie przerywamy pętlę
                        if e.kind() == std::io::ErrorKind::UnexpectedEof {
                            break;
                        }
                        eprintln!("Error while reading from file: {e}");
                        break;
                    }
                }
            }

            // --- ZABEZPIECZENIE KOŃCÓWKI PLIKU ---
            // Jeśli plik się skończył, a w buforze zostały jeszcze jakieś bajty 
            // (bo np. cały plik nie dzielił się idealnie przez 8 bitów),
            // musimy wypchnąć tę końcówkę, aby nie zgubić danych!
            if !dynamic_buffer.is_empty() {
                let _ = reader_sender.send(dynamic_buffer);
            }
        });

        let transform_chunk_channel_size = 8;

        let (transform_sender, transform_receiver) = 
            mpsc::sync_channel::<Vec<u8>>(transform_chunk_channel_size );
        let (transform_msg_sender, transform_msg_receiver) = 
            mpsc::sync_channel::<bool>(1);

        // Here, we transform thread according to policy function.
        std::thread::spawn(move || {
            
            // Probe raw chunk buffer
            while let Ok(chunk_buffer) = reader_receiver.recv() {
                if transform_msg_receiver.try_recv().unwrap_or(false) {
                    let _ = reader_msg_sender.send(true);
                    break;
                }

                // Decompress raw chunk buffer using provided policy function
                let transform_buffer = policy(&chunk_buffer);

                if transform_sender.send(transform_buffer).is_err() {
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
                let _ = transform_msg_sender.send(true);
                return;
            }
        };

        let mut writer = BufWriter::new(output_file);

        while let Ok(compressed_buffer) = transform_receiver.recv() {            
            if let Err(e) = writer.write_all(&compressed_buffer) {
                eprintln!("Error while writing to file: {e}");
                break;
            }
        }

        let _ = transform_msg_sender.send(true);
        let _ = writer.flush();
    }

    pub fn compress_input_to_output(&self, pipeline: &FileCompressPipeline) {
        let compress_fn = self.settings.compression_level.get_compressor_fn();
        self.stream_input_to_output(pipeline, Arc::new(compress_fn));
    }

    pub fn decompress_input_to_output(&self, pipeline: &FileCompressPipeline) {
        let compression_level = self.settings.compression_level.clone();

        self.stream_input_to_output(pipeline, Arc::new(move |buffer: &[u8]| {
            let decompress_fn = compression_level.get_decompressor_fn();
            decompress_fn(buffer).unwrap()
        }));
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

    fn create_random_byte_data_file(fp: PathBuf) -> std::io::Result<(Vec<u8>, Vec<u8>)> {
        let mut input_file = File::create(fp.clone())?;

        // Write random data to input_file 
        let mut rng = rand::rng();
        let range = Uniform::new(1, 2).unwrap();

        let cluster_cnts = 2;

        let random_cnts: Vec<u8> = (0..cluster_cnts).map(|_| range.sample(&mut rng) as u8).collect();
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

    fn create_random_bit_data_file(fp: PathBuf) -> std::io::Result<(Vec<u8>, Vec<u8>)> {
        let mut input_file = File::create(fp.clone())?;

        let mut rng = rand::rng();
        let cluster_cnts = 512;
        
        let mut compress_output = vec![0u8; cluster_cnts];
        rng.fill_bytes(&mut compress_output);

        let mut total_bit_cnt = 0;
        compress_output.iter().for_each(|&x| total_bit_cnt += get_bit_cnt!(x) as usize);

        let fill_bit_cnt = 8 - (total_bit_cnt % 8) as u8;

        if fill_bit_cnt > 0 {
            let fill_bit = fill_bit_cnt | (0x80u8);
            compress_output.push(fill_bit);
        }

        // Remove bytes where count of bits is zero
        compress_output.retain(|&x| get_bit_cnt!(x) > 0);

        // Write data to tmp_input_file
        let mut decompress_output = Vec::new();
        let mut curr_byte = 0u8;
        let mut curr_shf = 0u8;

        for &cluster in &compress_output {
            let cnt = get_bit_cnt!(cluster);
            let bit = get_repr_bit!(cluster);

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

        let (expect_output, _) = create_random_byte_data_file(tmp_input_fp.clone()).unwrap();

        let pipeline = FileCompressPipeline {
            input: tmp_input_fp,
            output: tmp_output_fp,
        };

        let settings = FileCompressSettings {
            chunk_size: 512,
            compression_level: CompressorLevel::CompressorByteLevel,
        };

        let compressor = FileCompressor::new(&settings);

        // Actual compression
        compressor.compress_input_to_output(&pipeline);

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
        tmp_input_fp.push("tmp_byte_input_fp");

        let mut tmp_output_fp = PathBuf::from("assets");
        tmp_output_fp.push("tmp_byte_output_fp");

        let (compress_output, _decompess_output) = 
            create_random_bit_data_file(tmp_input_fp.clone()).unwrap();

        let pipeline = FileCompressPipeline {
            input: tmp_input_fp,
            output: tmp_output_fp,
        };

        let settings = FileCompressSettings {
            chunk_size: 512,
            compression_level: CompressorLevel::CompressorBitLevel,
        };

        let compressor = FileCompressor::new(&settings);

        // Actual compression
        compressor.compress_input_to_output(&pipeline);

        // Check compression
        let mut compressed_file = File::open(&pipeline.output)?;
        let mut file_output = Vec::new();

        compressed_file.read_to_end(&mut file_output)?;
        drop(compressed_file);

        assert!(bit_collapsed_eq(&file_output, &compress_output), 
                "Bit-level compression is invalid");

        // Delete temporary files
        let _ = std::fs::remove_file(&pipeline.input)?;
        let _ = std::fs::remove_file(&pipeline.output)?;

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

        let (_compress_output, decompress_output)= 
            create_random_byte_data_file(tmp_input_fp.clone()).unwrap();

        let pipeline_compress = FileCompressPipeline {
            input: tmp_input_fp.clone(),
            output: tmp_output_fp.clone(),
        };

        let pipeline_decompress = FileCompressPipeline {
            input: tmp_output_fp.clone(),
            output: tmp_final_output_fp.clone(),
        };

        let settings = FileCompressSettings {
            chunk_size: 8,
            compression_level: CompressorLevel::CompressorByteLevel,
        };

        let compressor = FileCompressor::new(&settings);

        compressor.compress_input_to_output(&pipeline_compress);
        compressor.decompress_input_to_output(&pipeline_decompress);

        let mut input= File::open(&tmp_final_output_fp)?;
        let mut result = Vec::new();

        let _ = input.read_to_end(&mut result);
        drop(input);

        assert_eq!(result, decompress_output);

        let _ = std::fs::remove_file(&tmp_input_fp);
        let _ = std::fs::remove_file(&tmp_output_fp);
        let _ = std::fs::remove_file(&tmp_final_output_fp);

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

        let (_compress_output, decompress_output)= 
            create_random_bit_data_file(tmp_input_fp.clone()).unwrap();

        let pipeline_compress = FileCompressPipeline {
            input: tmp_input_fp.clone(),
            output: tmp_output_fp.clone(),
        };

        let pipeline_decompress = FileCompressPipeline {
            input: tmp_output_fp.clone(),
            output: tmp_final_output_fp.clone(),
        };

        let settings = FileCompressSettings {
            chunk_size: 8,
            compression_level: CompressorLevel::CompressorBitLevel,
        };

        let compressor = FileCompressor::new(&settings);

        compressor.compress_input_to_output(&pipeline_compress);
        compressor.decompress_input_to_output(&pipeline_decompress);

        let mut input= File::open(&tmp_final_output_fp)?;
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
