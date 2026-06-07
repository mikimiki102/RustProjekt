/* 'byte_level_compress' and 'bit_level_compress' are functions
 *  for memory compressing.
 *  For proper file handling and compression, see file/filecompress.rs
 */

#[macro_export]
macro_rules! get_bit {
    ($byte:expr, $pos:expr) => {
        (($byte) >> ($pos)) & 1u8
    };
}

#[macro_export]
macro_rules! get_repr_bit {
    ($byte:expr) => {
        (get_bit!($byte, 7))
    };
}

#[macro_export]
macro_rules! get_bit_cnt {
    ($byte:expr) => {
        (($byte) & 0x7fu8)
    };
}

pub fn byte_level_compress(data: &[u8]) -> Vec<u8> {
    let mut result = Vec::new();

    if data.is_empty() {
        return result;
    }

    let mut curr_byte = data[0];
    let mut curr_cnt: u8 = 1;

    for &byte in &data[1..] {
        if byte == curr_byte && curr_cnt < u8::MAX {
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

pub fn bit_level_compress(data: &[u8]) -> Vec<u8> {
    let mut result = Vec::new();

    if data.is_empty() {
        return result;
    }

    let mut curr_bit = get_bit!(data[0], 0);
    let mut curr_cnt: u8 = 0;

    for byte in data {
        for shf in 0..8 {
            let bit = get_bit!(*byte, shf);

            if bit == curr_bit && curr_cnt < u8::MAX / 2 {
                curr_cnt += 1;
            }
            else {
                let cluster = curr_cnt | (curr_bit << 7);
                result.push(cluster);

                curr_bit = bit;
                curr_cnt = 1;
            }
        }
    }

    let cluster = curr_cnt | (curr_bit << 7);
    result.push(cluster);

    result
}

pub fn byte_level_decompress(data: &[u8]) -> Result<Vec<u8>, &str> {
    if data.len() % 2 != 0 {
        return Err("Invalid compressed file: bytes count is not even");
    }

    let mut result = Vec::new();

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

pub fn bit_level_decompress(data: &[u8]) -> Result<Vec<u8>, &str> {
    let mut result = Vec::new();

    if data.is_empty() {
        return Ok(result);
    }
    
    result.push(0u8);
    let mut curr_shf = 0u8;

    for (i, byte) in data.iter().enumerate() {
        let count = get_bit_cnt!(byte);
        let bit = get_repr_bit!(byte);

        for _ in 0..count {
            let prev = result.last_mut().unwrap();
            *prev |= bit << curr_shf;
            curr_shf += 1;

            if curr_shf >= 8 {
                curr_shf = 0;
                if i < data.len() - 1 {
                    result.push(0u8);
                }
            }
        }
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
