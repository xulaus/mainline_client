#[derive(Debug, PartialEq)]
pub enum EncodingError {
    InvalidHashCharacter,
    InvalidHashLength
}
use EncodingError::*;

#[inline]
fn hex_to_nibble(h: u8) -> Result<u8, EncodingError> {
    // Decode Numbers
    if h >= 0x30 && h <= 0x39 {
        return Ok(h - 0x30);
    }
    // Decode lower case
    if h >= 0x61 && h <= 0x66 {
        return Ok(h - 0x61 + 10);
    }
    // Decode upper case
    if h >= 0x41 && h <= 0x46 {
        return Ok(h - 0x41 + 10);
    }
    return Err(InvalidHashCharacter);
}

#[inline]
pub fn hex_to_byte(b1: u8, b2: u8) -> Result<u8, EncodingError> {
    Ok((hex_to_nibble(b1)? << 4) | (hex_to_nibble(b2)?))
}

pub fn bytes_from_hex<const LEN: usize>(hex: &str) -> Result<[u8; LEN], EncodingError> {
    if hex.len() != (2 * LEN) {
        return Err(InvalidHashLength);
    }

    let mut bytes: [u8; LEN] = [0; LEN];
    for (i, val) in hex.as_bytes().chunks(2).enumerate() {
        bytes[i] = hex_to_byte(val[0], val[1])?
    }
    Ok(bytes)
}

#[inline]
fn base32_decode_char(h: u8) -> Result<u8, EncodingError> {
    // RFC 4648 base 32

    // Decode lower case
    if h >= 0x61 && h <= 0x87 {
        return Ok(h - 0x61);
    }
    // Decode upper case
    if h >= 0x41 && h <= 0x67 {
        return Ok(h - 0x41);
    }
    // Decode Numbers from 2 to 7
    if h >= 0x32 && h <= 0x37 {
        return Ok(h - 0x32 + 26);
    }
    return Err(InvalidHashCharacter);
}

pub fn bytes_from_base32<const LEN: usize>(enc: &str) -> Result<[u8; LEN], EncodingError> {

    if enc.len() != ((LEN + 4) / 5) * 8 {
        return Err(InvalidHashLength);
    }

    let mut out: [u8; LEN] = [0; LEN];
    let bytes = enc.as_bytes();
    let first_pad: usize = (LEN * 8 + 4) / 5;
    let last_byte_start: usize = ((LEN - 1) * 8 + 4) / 5; // first charicter fully inside last byte

    #[inline]
    fn destructure_byte(offset_in_chunk: u32, byte: u8) -> Result<(u8, u8), EncodingError> {
        let output_mod: u32 = (offset_in_chunk * 5) % 8;

        let val = base32_decode_char(byte)?;
        Ok((
            (val << 3).wrapping_shr(output_mod),
            val.checked_shl(3 + (8 - output_mod)).unwrap_or(0u8),
        ))
    }

    for i in 0..last_byte_start {
        let offset_in_chunk = i % 8;
        let (this, next) = destructure_byte(offset_in_chunk as u32, bytes[i])?;
        let output_location = (i * 5) / 8;

        out[output_location] |= this;
        out[output_location + 1] |= next;
    }
    // handle last not padding byte
    for i in last_byte_start..first_pad {
        let offset_in_chunk = i % 8;
        let (this, next) = destructure_byte(offset_in_chunk as u32, bytes[i])?;
        let output_location = (i * 5) / 8;

        out[output_location] |= this;
        if next != 0 {
            return Err(InvalidHashCharacter);
        }
    }
    if !bytes[first_pad..].iter().all(|&b| b == 0x3D) {
        // there was a non padding character
        return Err(InvalidHashCharacter);
    }

    Ok(out)
}

#[cfg(test)]
mod tests {
    use super::*;
    use test_case::test_case;

    #[test]
    fn test_bytes_from_hex() {
        let sha11 = bytes_from_hex("abCDef");
        let sha12 = bytes_from_hex("ABcdEF");

        assert_eq!(Ok([0xAB, 0xCD, 0xEF]), sha12);
        assert_eq!(sha11, sha12);

        let bad1 = bytes_from_hex::<1>("A");
        assert_eq!(bad1.err(), Some(InvalidHashLength));
        let bad2 = bytes_from_hex::<1>("BCD");
        assert_eq!(bad2.err(), Some(InvalidHashLength));
    }

    #[test]
    fn test_bytes_from_base32_case_insensitive() {
        let ac1 = bytes_from_base32::<1>("Ai======");
        let ac2 = bytes_from_base32::<1>("aI======");
        assert_eq!(ac1, ac2);
        assert_eq!(Ok([0x02]), ac2);
    }

    #[test_case("abCQ====", Ok([0x00, 0x45]); "Correct decoding")]
    #[test_case("ABC3===", Err(InvalidHashLength); "Encoding too short")]
    #[test_case("ABC3=====", Err(InvalidHashLength); "Encoding too long")]
    fn test_2_bytes_from_base32(s: &str, expected: Result<[u8; 2],  EncodingError>) {
        assert_eq!(bytes_from_base32::<2>(s), expected);
    }

    #[test_case("74======", Ok([0xFF]); "Correct decoding")]
    #[test_case("Ab======", Err(InvalidHashCharacter))]
    #[test_case("ABC1====", Err(InvalidHashCharacter))]
    fn test_1_bytes_from_base32(s: &str, expected: Result<[u8; 1],  EncodingError>) {
        assert_eq!(bytes_from_base32::<1>(s), expected);
    }

    #[test_case("GL3Sda7y2A======", Ok([0x32, 0xf7, 0x21, 0x83, 0xf8, 0xd0]); "Correct decoding")]
    #[test_case("ABC7===========", Err(InvalidHashLength); "Encoding too short")]
    #[test_case("ABC3=============", Err(InvalidHashLength); "Encoding too long")]
    fn test_6_bytes_from_base32(s: &str, expected: Result<[u8; 6],  EncodingError>) {
        assert_eq!(bytes_from_base32::<6>(s), expected);
    }

    #[test]
    fn test_bytes_from_base32_misc_length() {
        // 5 bytes (Must read full chunk sucessfully)
        let full_chunk = bytes_from_base32::<5>("77777777");
        assert_eq!(Ok([0xFF, 0xFF, 0xFF, 0xFF, 0xFF]), full_chunk);
        let full_chunk = bytes_from_base32::<5>("GLASda73");
        assert_eq!(Ok([0x32, 0xc1, 0x21, 0x83, 0xfb]), full_chunk);


        // 11 bytes (Must read over two full chunks sucessfully)
        let three_chunks = bytes_from_base32::<11>("77777777GL3Sda7y2A======");
        assert_eq!(
            Ok([0xFF, 0xFF, 0xFF, 0xFF, 0xFF, 0x32, 0xf7, 0x21, 0x83, 0xf8, 0xd0]),
            three_chunks
        );

        // More data after padding begins
        let good_pad = bytes_from_base32::<3>("77776===");
        assert_eq!(good_pad, Ok([255, 255, 255]));
        let bad_pad = bytes_from_base32::<3>("77777===");
        assert_eq!(bad_pad, Err(InvalidHashCharacter));
        let bad_pad = bytes_from_base32::<3>("77776=1=");
        assert_eq!(bad_pad, Err(InvalidHashCharacter));
    }


    #[test_case(b"0", Ok(0x0); "0")]
    #[test_case(b"1", Ok(0x1); "1")]
    #[test_case(b"2", Ok(0x2); "2")]
    #[test_case(b"3", Ok(0x3); "3")]
    #[test_case(b"4", Ok(0x4); "4")]
    #[test_case(b"5", Ok(0x5); "5")]
    #[test_case(b"6", Ok(0x6); "6")]
    #[test_case(b"8", Ok(0x8); "8")]
    #[test_case(b"9", Ok(0x9); "9")]
    #[test_case(b"a", Ok(0xa); "a lowercase")]
    #[test_case(b"b", Ok(0xb); "b lowercase")]
    #[test_case(b"c", Ok(0xc); "c lowercase")]
    #[test_case(b"d", Ok(0xd); "d lowercase")]
    #[test_case(b"e", Ok(0xe); "e lowercase")]
    #[test_case(b"f", Ok(0xf); "f lowercase")]
    #[test_case(b"A", Ok(0xA); "A uppercase")]
    #[test_case(b"B", Ok(0xB); "B uppercase")]
    #[test_case(b"C", Ok(0xC); "C uppercase")]
    #[test_case(b"D", Ok(0xD); "D uppercase")]
    #[test_case(b"E", Ok(0xE); "E uppercase")]
    #[test_case(b"F", Ok(0xF); "F uppercase")]
    #[test_case(b"g", Err(InvalidHashCharacter); "g lowercase")]
    #[test_case(b"G", Err(InvalidHashCharacter); "G uppercase")]
    fn test_hex_to_nibble(s: &[u8], expected: Result<u8, EncodingError>) {
        assert_eq!(expected, hex_to_nibble(s[0]));
    }
}
