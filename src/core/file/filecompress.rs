use std::fs::File;
use std::io::{self, Read};
use std::sync::mpsc;

#[derive(Debug)]
pub struct FileCompressPipeline<Input: Read, Output: Write> {
    pub input: Input,
    pub output: Output,
}

#[derive(Clone, Debug)]
enum CompressorLevel {
    CompressorBitLevel,
    CompressorByteLevel,
}

impl CompressorLevel {
    pub fn get_compressor_fn(self) -> fn(&[u8]) -> Vec<u8> {
        match self {
            CompressorLevel::CompressorBitLevel => crate::core::rlecompress::bit_level_compress,
            CompressorLevel::CompressorByteLevel => crate::core::rlecompress::byte_level_compress,
        }
    }
}

#[derive(Debug)]
pub struct FileCompressSettings {
    pub chunk_size: usize,
    pub compressor_level: CompressorLevel,
}

#[derive(Debug)]
pub struct FileCompressor {
    pub pipeline: FileCompressPipeline,
    pub settings: FileCompressSettings,  
}

impl FileCompressor {
    pub fn new(pipeline: &FileCompressPipeline, settings: &FileCompressSettings) -> Self {
        Self {
            pipeline.clone(),
            settings.clone()
        }
    }

    pub fn input_file(&self) -> &[String] {
        &self.pipeline.input;
    }

    pub fn output_file(&self) -> &[String] {
        &self.pipeline.output;
    }

    pub fn compress_input_to_output(&self) {
        let input_path = self.pipeline.input.clone();
        let output_path = self.pipeline.output;
        let chunk_size = self.settings.chunk_size;
        let compress_fn = self.settings.compressor_level.get_compressor_fn();

        let (reader_sender, reader_receiver) = mpsc::sync_channel::<Vec<u8>>(8);
        let (reader_msg_sender, reader_msg_receiver) = mpsc::sync_channel::<bool>(1);
        
        // Main reading thread.
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

            while reader_msg_receiver.try_recv().unwrap_or(false) {
                // Create zeroed buffer and read full chunk from
                // input file. Send it to another thread.
                let mut buffer = vec![0u8; chunk_size];
                match file.by_ref().take(chunk_size as u64).read_to_end(&mut buffer) {
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
                    }
                    Err(e) => {
                        eprintln!("Error while reading from file: {e}");
                        break;
                    }
                }
            }
        });

        let (compressed_sender, compressed_receiver) = mpsc::sync_channel::<Vec<u8>>(4);
        let (compressed_msg_sender, compressed_msg_receiver) = mpsc::sync_channel::<bool>(1);

        // Compressing thread.
        // We compress chunks and send them to writer thread.
        std::thread::spawn(move || {

            // Probe raw chunk buffer
            while let Ok(chunk_buffer) = reader_receiver.recv() {
                if compressed_msg_receiver.try_recv().unwrap_or(false) {
                    reader_msg_sender.send(true);
                    break;
                }

                // Compress raw chunk buffer using 'compress_fn' function
                let compressed_buffer = compress_fn(&chunk_buffer);

                if compressed_sender.send(compressed_buffer).is_err() {
                    reader_msg_sender.send(true);
                    break;
                }
            }
        });

        while let Ok(compressed_buffer) = compressed_receiver.recv() {
            if compressed_msg_receiver.try_recv().unwrap_or(false) {
                reader_msg_sender.send(true);
                break;
            }

            // Compress raw chunk buffer using 'compress_fn' function
            let compressed_buffer = compress_fn(&chunk_buffer);

            if compressed_sender.send(compressed_buffer).is_err() {
                reader_msg_sender.send(true);
                break;
            }
        }
    }
}
