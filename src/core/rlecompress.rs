/* 'byte_level_compress' and 'bit_level_compress' are functions
 *  for memory compressing.
 *  For proper file handling and compression, see file/filecompress.rs
 */

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

    let mut curr_bit = data[0] & 1;
    let mut curr_cnt: u8 = 0;

    for byte in data {
        for shf in 0..8u8 {
            let bit = (*byte >> shf) & 1;

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
}
