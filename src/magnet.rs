use std::{borrow::Cow, collections::HashMap, error::Error, fmt, mem, str::FromStr};

#[derive(Debug, PartialEq)]
pub enum MagnetURIError {
    InvalidHashCharacter,
    InvalidHashLength,
    InvalidURIScheme,
    InvalidStartCharacter,
    UnknownHashFunction,
    InvalidUseOfReservedChar,
    NotImplemented,
}
impl Error for MagnetURIError {
    fn description(&self) -> &str {
        use MagnetURIError::*;
        match *self {
            InvalidHashCharacter => "Invalid character in hex string",
            InvalidHashLength => "Hex string was an inappropriate size",
            InvalidURIScheme => "URI scheme must be \"magnet:\"",
            InvalidStartCharacter => "Magnet URI must start with \"?\"",
            UnknownHashFunction => "URN hash function unknown",
            InvalidUseOfReservedChar => "Invalid use of reserved character in query string",
            NotImplemented => "Soz lol",
        }
    }
}
impl fmt::Display for MagnetURIError {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(f, "{}", self.to_string())
    }
}

#[inline]
fn hex_to_nibble(h: u8) -> Result<u8, MagnetURIError> {
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
    return Err(MagnetURIError::InvalidHashCharacter);
}

#[inline]
fn hex_to_byte(b1: u8, b2: u8) -> Result<u8, MagnetURIError> {
    Ok((hex_to_nibble(b1)? << 4) | (hex_to_nibble(b2)?))
}

fn bytes_from_hex<const LEN: usize>(hex: &str) -> Result<[u8; LEN], MagnetURIError> {
    if hex.len() != (2 * LEN) {
        return Err(MagnetURIError::InvalidHashLength);
    }

    unsafe {
        let mut bytes: mem::MaybeUninit<[u8; LEN]> = mem::MaybeUninit::uninit();
        for (i, val) in hex.as_bytes().chunks(2).enumerate() {
            (*bytes.as_mut_ptr())[i] = hex_to_byte(val[0], val[1])?
        }
        Ok(bytes.assume_init())
    }
}

#[inline]
fn base32_decode_char(h: u8) -> Result<u8, MagnetURIError> {
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
    return Err(MagnetURIError::InvalidHashCharacter);
}

fn bytes_from_base32<const LEN: usize>(enc: &str) -> Result<[u8; LEN], MagnetURIError> {
    use MagnetURIError::*;

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
    fn destructure_byte(offset_in_chunk: u32, byte: u8) -> Result<(u8, u8), MagnetURIError> {
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

fn uri_decode_value(value: &str) -> Result<Cow<str>, MagnetURIError> {
    use MagnetURIError::*;
    const INVALID: [char; 3] = ['#', '?', '&'];
    if INVALID.iter().any(|v| value.contains(*v)) {
        return Err(InvalidUseOfReservedChar);
    }

    let s: Cow<str> = match value.find('+') {
        Some(_) => Cow::Owned(value.replace('+', " ")),
        None => Cow::Borrowed(value),
    };

    match (*s).find('%') {
        Some(first_percent) => {
            let mut ret = Vec::<u8>::with_capacity(s.bytes().len());
            ret.extend(s[..first_percent].bytes());
            for segment in s[(first_percent + 1)..].split("%") {
                if segment.len() < 2 {
                    return Err(InvalidUseOfReservedChar);
                }
                let bytes = segment.as_bytes();
                ret.push(hex_to_byte(bytes[0], bytes[1])?);
                ret.extend_from_slice(&bytes[2..]);
            }
            match String::from_utf8(ret) {
                Ok(string) => Ok(Cow::Owned(string)),
                Err(_) => Err(NotImplemented),
            }
        }
        None => Ok(s),
    }
}

#[derive(Debug, PartialEq)]
pub enum MagnetHash {
    SHA1([u8; 20]),
    MD5([u8; 16]),
    BTIH([u8; 20]),
    INVALID,
}

impl FromStr for MagnetHash {
    type Err = MagnetURIError;

    fn from_str(s: &str) -> Result<MagnetHash, Self::Err> {
        if s.starts_with("urn:sha1:") {
            Ok(MagnetHash::SHA1(bytes_from_base32(&s[9..])?))
        } else if s.starts_with("urn:md5:") {
            Ok(MagnetHash::MD5(bytes_from_hex(&s[8..])?))
        } else if s.starts_with("urn:btih:") {
            if s.len() == 49 {
                Ok(MagnetHash::BTIH(bytes_from_hex(&s[9..])?))
            } else {
                Ok(MagnetHash::BTIH(bytes_from_base32(&s[9..])?))
            }
        } else {
            Err(MagnetURIError::UnknownHashFunction)
        }
    }
}

#[derive(Debug, PartialEq)]
pub struct MagnetFile {
    hash: MagnetHash,
    display_name: String,
}

impl Default for MagnetFile {
    fn default() -> Self {
        MagnetFile {
            hash: MagnetHash::INVALID,
            display_name: "".to_string(),
        }
    }
}

#[derive(Debug, PartialEq)]
pub struct MagnetFiles {
    files: Vec<MagnetFile>,
}

impl FromStr for MagnetFiles {
    type Err = MagnetURIError;

    fn from_str(s: &str) -> Result<MagnetFiles, Self::Err> {
        use MagnetURIError::*;

        if !s.starts_with("magnet:") {
            return Err(InvalidURIScheme);
        }
        if !s[7..].starts_with("?") {
            return Err(InvalidStartCharacter);
        }

        let mut files: HashMap<&str, MagnetFile> = HashMap::new();
        for serialised_pair in s[8..].split("&") {
            match serialised_pair.find("=") {
                Some(split_at) => {
                    let key = &serialised_pair[0..split_at];
                    let value = uri_decode_value(&serialised_pair[(split_at + 1)..])?;

                    if key == "xt" || key.starts_with("xt.") {
                        let file_key = if key.len() < 3 { &"1" } else { &key[3..] };
                        let hash = MagnetHash::from_str(&value)?;
                        match files.get_mut(file_key) {
                            Some(file) => {
                                file.hash = hash;
                            }
                            None => {
                                let mut file: MagnetFile = Default::default();
                                file.hash = hash;
                                files.insert(file_key, file);
                            }
                        };
                    } else if key == "dn" || key.starts_with("dn.") {
                        let file_key = if key.len() < 3 { &"1" } else { &key[3..] };
                        match files.get_mut(file_key) {
                            Some(file) => {
                                file.display_name = (*value).to_string();
                            }
                            None => {
                                let mut file: MagnetFile = Default::default();
                                file.display_name = (*value).to_string();
                            }
                        };
                    }
                }
                None => {
                    // TODO: log warning
                }
            };
        }

        Ok(MagnetFiles {
            files: files.into_iter().map(|kv_pair| kv_pair.1).collect(),
        })
    }
}

pub fn parse(s: &str) -> Result<MagnetFiles, MagnetURIError> {
    MagnetFiles::from_str(s)
}

#[cfg(test)]
mod tests {
    use super::*;
    use MagnetURIError::*;

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
    fn hash_from_str() {
        let magnet1 = MagnetHash::from_str("urn:md5:c12fe1c06bba254a9dc9f519b335aa7c");
        assert_eq!(
            magnet1,
            Ok(MagnetHash::MD5([
                193, 47, 225, 192, 107, 186, 37, 74, 157, 201, 245, 25, 179, 53, 170, 124
            ]))
        );

        // let magnet2 = MagnetHash::from_str("urn:sha1:TXGCZQTH26NL6OUQAJJPFALHG2LTGBC7");
        // assert_eq!(
        //     magnet2,
        //     Ok(MagnetHash::SHA1([
        //         0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0
        //     ]))
        // );

        let magnet3 = MagnetHash::from_str("urn:btih:209c8226b299b308beaf2b9cd3fb49212dbd13ec");
        assert_eq!(
            magnet3,
            Ok(MagnetHash::BTIH([
                32, 156, 130, 38, 178, 153, 179, 8, 190, 175, 43, 156, 211, 251, 73, 33, 45, 189,
                19, 236
            ]))
        );
    }

    #[test]
    fn file_from_str() {
        let magnet1 =
            MagnetFiles::from_str("magnet:?xt=urn:md5:c12fe1c06bba254a9dc9f519b335aa7c1367a88a");
        let magnet2 = MagnetFiles::from_str(
            "magnet:?xt=urn%3amd5%3Ac12fe1c06bba254a9dc9f519b335aa7c1367a88a",
        );

        assert_eq!(magnet1, magnet2);
    }

    #[test]
    fn test_hex_to_nibble() -> Result<(), Box<dyn Error>> {
        assert_eq!(0x0, hex_to_nibble("0".as_bytes()[0])?);
        assert_eq!(0x1, hex_to_nibble("1".as_bytes()[0])?);
        assert_eq!(0x2, hex_to_nibble("2".as_bytes()[0])?);
        assert_eq!(0x3, hex_to_nibble("3".as_bytes()[0])?);
        assert_eq!(0x4, hex_to_nibble("4".as_bytes()[0])?);
        assert_eq!(0x5, hex_to_nibble("5".as_bytes()[0])?);
        assert_eq!(0x6, hex_to_nibble("6".as_bytes()[0])?);
        assert_eq!(0x8, hex_to_nibble("8".as_bytes()[0])?);
        assert_eq!(0x9, hex_to_nibble("9".as_bytes()[0])?);
        assert_eq!(0xa, hex_to_nibble("a".as_bytes()[0])?);
        assert_eq!(0xb, hex_to_nibble("b".as_bytes()[0])?);
        assert_eq!(0xc, hex_to_nibble("c".as_bytes()[0])?);
        assert_eq!(0xd, hex_to_nibble("d".as_bytes()[0])?);
        assert_eq!(0xe, hex_to_nibble("e".as_bytes()[0])?);
        assert_eq!(0xf, hex_to_nibble("f".as_bytes()[0])?);
        assert_eq!(0xA, hex_to_nibble("A".as_bytes()[0])?);
        assert_eq!(0xB, hex_to_nibble("B".as_bytes()[0])?);
        assert_eq!(0xC, hex_to_nibble("C".as_bytes()[0])?);
        assert_eq!(0xD, hex_to_nibble("D".as_bytes()[0])?);
        assert_eq!(0xE, hex_to_nibble("E".as_bytes()[0])?);
        assert_eq!(0xF, hex_to_nibble("F".as_bytes()[0])?);
        assert_eq!(
            hex_to_nibble("G".as_bytes()[0]).err(),
            Some(InvalidHashCharacter)
        );
        assert_eq!(
            hex_to_nibble("g".as_bytes()[0]).err(),
            Some(InvalidHashCharacter)
        );
        Ok(())
    }

    #[test]
    fn test_uri_decode_value() {
        let no_replace_needed = uri_decode_value("ABCD").unwrap();
        assert!(no_replace_needed.is_borrowed());
        assert_eq!(no_replace_needed, "ABCD");

        let replace_needed = uri_decode_value("%41CD").unwrap();
        assert!(replace_needed.is_owned());
        assert_eq!(replace_needed, "ACD");

        assert_eq!(uri_decode_value("%%").err(), Some(InvalidUseOfReservedChar));
        assert_eq!(
            uri_decode_value("sad#asd").err(),
            Some(InvalidUseOfReservedChar)
        );
        assert_eq!(
            uri_decode_value("asd&asd").err(),
            Some(InvalidUseOfReservedChar)
        );
        assert_eq!(
            uri_decode_value("asd?asd").err(),
            Some(InvalidUseOfReservedChar)
        );
    }
}
