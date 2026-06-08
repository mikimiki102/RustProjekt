//! File: memcompress.rs.
//! 
//! Functions for memory buffers compression.
//!
//! This module provides [`byte_level_compress`] and [`bit_level_compress`] 
//! for handling in-memory data buffers.
//!
//! For proper file handling and streaming compression, see the 
//! [`filecompress`](crate::core::file::filecompress) module.

#[macro_export]
macro_rules! get_bit_u8 {
    ($byte:expr, $pos:expr) => 
    { ((($byte) >> ($pos)) & 1u8) };
}
#[macro_export]
macro_rules! set_bit_as_u8 {
    {$byte:expr, $bit:expr, $pos:expr} => 
    { ($byte |= $bit << $pos) }
}
#[macro_export]
macro_rules! set_bit_u8 {
    {$byte:expr, $pos:expr} => 
    { set_bit_as_u8!(byte, 1, pos); }
}
#[macro_export]
macro_rules! get_repr_bit_u8 {
    ($byte:expr) => 
    { (get_bit_u8!($byte, 7u8)) };
}
#[macro_export]
macro_rules! get_bit_cnt_u8 {
    ($byte:expr) => 
    { (($byte) & 0x7fu8) };
}
#[macro_export]
macro_rules! bit_cluster_u8 {
    ($cnt:expr, $bit:expr) => 
    { (($cnt) | ($bit << 7u8)) };
}

// We save incoming bytes repeating count in entire byte.
const MAX_CNT_BYTE_LV_CLUSTER: u8 = u8::MAX;    
// We save count in lower 7-bits of a byte, that is u8::MAX / 2 maximum.
const MAX_CNT_BIT_LV_CLUSTER: u8 = u8::MAX / 2; 

/// Returns compressed memory buffer using byte compression.
pub fn byte_level_compress(buff: &[u8]) -> Vec<u8> {
    let mut result = Vec::new();

    if buff.is_empty() {
        return result;
    }

    let mut curr_byte = buff[0];
    let mut curr_cnt = 1u8;

    for &byte in &buff[1..] {
        if byte == curr_byte && 
           curr_cnt < MAX_CNT_BYTE_LV_CLUSTER {
            curr_cnt += 1;
        } 
        else {
            result.push(curr_cnt);
            result.push(curr_byte);

            curr_byte = byte;
            curr_cnt = 1;
        }
    }

    result.push(curr_cnt);
    result.push(curr_byte);

    result
}

/// Returns compressed memory buffer using bit compression.
pub fn bit_level_compress(buff: &[u8]) -> Vec<u8> {
    let mut result = Vec::new();

    if buff.is_empty() {
        return result;
    }

    let mut curr_bit = get_bit_u8!(buff[0], 0);
    let mut curr_cnt = 0u8;

    for byte in buff {
        for shf in 0..8u8 {
            let bit = get_bit_u8!(*byte, shf);

            if bit == curr_bit && curr_cnt < MAX_CNT_BIT_LV_CLUSTER {
                curr_cnt += 1;
            }
            else {
                let cluster = bit_cluster_u8!(curr_cnt, curr_bit);
                result.push(cluster);

                curr_bit = bit;
                curr_cnt = 1;
            }
        }
    }

    let cluster = bit_cluster_u8!(curr_cnt, curr_bit);
    result.push(cluster);

    result
}

/// Returns decompressed memory buffer that was compressed using byte compression.
pub fn byte_level_decompress(data: &[u8]) -> Result<Vec<u8>, &str> {
    if data.len() % 2 != 0 {
        return Err("Invalid compressed file: bytes count is not even");
    }

    let min_result_size = data.len() / 2;
    let mut result = Vec::with_capacity(min_result_size);

    if data.is_empty() {
        return Ok(result);
    }

    for pair in data.chunks(2) {
        let count = pair[0];
        let byte = pair[1];

        for _ in 0..count {
            result.push(byte);
        }
    }

    Ok(result)
}

/// Returns decompressed memory buffer that was compressed using bit compression.
pub fn bit_level_decompress(data: &[u8]) -> Result<Vec<u8>, &str> {
    let mut result = Vec::new();

    if data.is_empty() {
        return Ok(result);
    }
    
    result.push(0u8);
    let mut curr_shf = 0u8;
    let mut total_cnt = 0usize;

    for (i, byte) in data.iter().enumerate() {
        let count = get_bit_cnt_u8!(byte);
        let bit = get_repr_bit_u8!(byte);
        total_cnt += count as usize;

        for j in 0..count {
            let prev = result.last_mut().unwrap();
            set_bit_as_u8!(*prev, bit, curr_shf);
            curr_shf += 1;

            if curr_shf >= 8 {
                curr_shf = 0;
                if j < count - 1 || i < data.len() - 1 {
                    result.push(0u8);
                }
            }
        }
    }

    if total_cnt % 8 > 0 {
        return Err("Bit bufor is misaligned");
    }

    Ok(result)
}

#[cfg(test)]
mod tests {
    use super::*;
    
    #[test]
    fn test_byte_empty() {
        let input: [u8; 0] = [];
        assert_eq!(byte_level_compress(&input), Vec::<u8>::new());
    }

    #[test]
    fn test_byte_simple() {
        let input = [1, 1, 1, 2, 2, 3];
        let expected = [3, 1, 2, 2, 1, 3];
        assert_eq!(byte_level_compress(&input), expected);
    }

    #[test]
    fn test_byte_no_repeats() {
        let input = [1, 2, 3, 4];
        let expected = [1, 1, 1, 2, 1, 3, 1, 4];
        assert_eq!(byte_level_compress(&input), expected);
    }

    #[test]
    fn test_byte_overflow_counter() {
        let input = vec![0xAA; 260];
        let result = byte_level_compress(&input);
        assert_eq!(result, vec![255, 0xAA, 5, 0xAA]);
    }

    #[test]
    fn test_bit_empty() {
        let input: [u8; 0] = [];
        assert_eq!(bit_level_compress(&input), Vec::<u8>::new());
    }

    #[test]
    fn test_bit_all_zeros() {
        let input = [0b0000_0000];
        let expected = [8];
        assert_eq!(bit_level_compress(&input), expected);
    }

    #[test]
    fn test_bit_all_ones() {
        let input = [0b1111_1111];
        let expected = [136];
        assert_eq!(bit_level_compress(&input), expected);
    }

    #[test]
    fn test_bit_alternating() {
        let input = [0b1010_1010];
        let expected = [1, 129, 1, 129, 1, 129, 1, 129];
        assert_eq!(bit_level_compress(&input), expected);
    }

    #[test]
    fn test_bit_overflow_counter() {
        let input = vec![0xFF; 16];
        let result = bit_level_compress(&input);
        assert_eq!(result, vec![255, 129]);
    }

    #[test]
    fn test_compress_and_decompress_text() {
        let input = b"AAAABBBCCDAA";

        let compressed = byte_level_compress(input);
        let decompressed = byte_level_decompress(&compressed).unwrap();

        assert_eq!(input.to_vec(), decompressed);

        let compressed = bit_level_compress(input);
        let decompressed = bit_level_decompress(&compressed).unwrap();

        assert_eq!(input.to_vec(), decompressed);
    }

    #[test]
    fn test_empty_data() {
        let input = b"";

        let compressed = byte_level_compress(input);
        let decompressed = byte_level_decompress(&compressed).unwrap();

        assert_eq!(input.to_vec(), decompressed);

        let compressed = bit_level_compress(input);
        let decompressed = bit_level_decompress(&compressed).unwrap();

        assert_eq!(input.to_vec(), decompressed);
    }

    #[test]
    fn test_no_repetitions() {
        let input = b"ABCDEF";

        let compressed = byte_level_compress(input);
        let decompressed = byte_level_decompress(&compressed).unwrap();

        assert_eq!(input.to_vec(), decompressed);

        let compressed = bit_level_compress(input);
        let decompressed = bit_level_decompress(&compressed).unwrap();

        assert_eq!(input.to_vec(), decompressed);
    }

    #[test]
    fn test_long_sequence() {
        let input = vec![b'A'; 300];

        let compressed = byte_level_compress(&input);
        let decompressed = byte_level_decompress(&compressed).unwrap();

        assert_eq!(input.to_vec(), decompressed);

        let compressed = bit_level_compress(&input);
        let decompressed = bit_level_decompress(&compressed).unwrap();

        assert_eq!(input.to_vec(), decompressed);
    }
}
