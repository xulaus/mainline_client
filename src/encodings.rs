use std::mem;

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

    let b = hex.as_bytes().chunks(2).map(|val| hex_to_byte(val[0], val[1])).collect::<Result<Vec<u8>, _>>()?;
    Ok(b.try_into().unwrap())
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

    let mut out: [u8; LEN] = unsafe {
        let mut out_unsafe: mem::MaybeUninit<[u8; LEN]> = mem::MaybeUninit::uninit();
        for i in 0..LEN {
            (*out_unsafe.as_mut_ptr())[i] = 0;
        }
        out_unsafe.assume_init()
    };

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
    for i in first_pad..enc.len() {
        // != '='
        if bytes[i] != 0x3D {
            return Err(InvalidHashCharacter);
        }
    }

    Ok(out)
}

mod tests {
    use super::*;
    use EncodingError::*;

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
    fn test_bytes_from_base32() {
        // Basic Decode
        let ff = bytes_from_base32::<1>("74======");
        assert_eq!(Ok([0xFF]), ff);

        // Case sensitivity
        let ac1 = bytes_from_base32::<1>("Ai======");
        let ac2 = bytes_from_base32::<1>("aI======");
        assert_eq!(ac1, ac2);
        assert_eq!(Ok([0x02]), ac2);

        // Invalid 1 char code
        let invalid1 = bytes_from_base32::<1>("Ab======");
        assert_eq!(invalid1.err(), Some(InvalidHashCharacter));

        // 2 bytes (Must read more than one byte sucessfully)
        let ac2 = bytes_from_base32::<2>("abCQ====");
        assert_eq!(Ok([0x00, 0x45]), ac2);

        // Invalid 2 char code
        let invalid2 = bytes_from_base32::<1>("ABC1====");
        assert_eq!(invalid2.err(), Some(InvalidHashCharacter));

        // 5 bytes (Must read full chunk sucessfullt)

        let ac3 = bytes_from_base32::<5>("77777777");
        assert_eq!(Ok([0xFF, 0xFF, 0xFF, 0xFF, 0xFF]), ac3);
        let ac4 = bytes_from_base32::<5>("GLASda73");
        assert_eq!(Ok([0x32, 0xc1, 0x21, 0x83, 0xfb]), ac4);

        // 6 bytes (Must read over one full chunks sucessfully)
        let ac5 = bytes_from_base32::<6>("GL3Sda7y2A======");
        assert_eq!(Ok([0x32, 0xf7, 0x21, 0x83, 0xf8, 0xd0]), ac5);

        // 11 bytes (Must read over two full chunks sucessfully)
        let ac5 = bytes_from_base32::<11>("77777777GL3Sda7y2A======");
        assert_eq!(
            Ok([0xFF, 0xFF, 0xFF, 0xFF, 0xFF, 0x32, 0xf7, 0x21, 0x83, 0xf8, 0xd0]),
            ac5
        );

        // Length is too Short
        let short1 = bytes_from_base32::<2>("ABC3===");
        assert_eq!(short1.err(), Some(InvalidHashLength));
        let short2 = bytes_from_base32::<6>("ABC7===========");
        assert_eq!(short2.err(), Some(InvalidHashLength));

        // Length is too long
        let long1 = bytes_from_base32::<2>("ABC3=====");
        assert_eq!(long1.err(), Some(InvalidHashLength));
        let long2 = bytes_from_base32::<6>("ABC3=============");
        assert_eq!(long2.err(), Some(InvalidHashLength));
    }

    #[test]
    fn test_hex_to_nibble() {
        assert_eq!(Ok(0x0), hex_to_nibble("0".as_bytes()[0]));
        assert_eq!(Ok(0x1), hex_to_nibble("1".as_bytes()[0]));
        assert_eq!(Ok(0x2), hex_to_nibble("2".as_bytes()[0]));
        assert_eq!(Ok(0x3), hex_to_nibble("3".as_bytes()[0]));
        assert_eq!(Ok(0x4), hex_to_nibble("4".as_bytes()[0]));
        assert_eq!(Ok(0x5), hex_to_nibble("5".as_bytes()[0]));
        assert_eq!(Ok(0x6), hex_to_nibble("6".as_bytes()[0]));
        assert_eq!(Ok(0x8), hex_to_nibble("8".as_bytes()[0]));
        assert_eq!(Ok(0x9), hex_to_nibble("9".as_bytes()[0]));
        assert_eq!(Ok(0xa), hex_to_nibble("a".as_bytes()[0]));
        assert_eq!(Ok(0xb), hex_to_nibble("b".as_bytes()[0]));
        assert_eq!(Ok(0xc), hex_to_nibble("c".as_bytes()[0]));
        assert_eq!(Ok(0xd), hex_to_nibble("d".as_bytes()[0]));
        assert_eq!(Ok(0xe), hex_to_nibble("e".as_bytes()[0]));
        assert_eq!(Ok(0xf), hex_to_nibble("f".as_bytes()[0]));
        assert_eq!(Ok(0xA), hex_to_nibble("A".as_bytes()[0]));
        assert_eq!(Ok(0xB), hex_to_nibble("B".as_bytes()[0]));
        assert_eq!(Ok(0xC), hex_to_nibble("C".as_bytes()[0]));
        assert_eq!(Ok(0xD), hex_to_nibble("D".as_bytes()[0]));
        assert_eq!(Ok(0xE), hex_to_nibble("E".as_bytes()[0]));
        assert_eq!(Ok(0xF), hex_to_nibble("F".as_bytes()[0]));
        assert_eq!(
            hex_to_nibble("G".as_bytes()[0]).err(),
            Some(InvalidHashCharacter)
        );
        assert_eq!(
            hex_to_nibble("g".as_bytes()[0]).err(),
            Some(InvalidHashCharacter)
        );
    }
}