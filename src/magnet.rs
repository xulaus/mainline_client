use mainline_client::encodings::{hex_to_byte, bytes_from_base32, bytes_from_hex, EncodingError};

use std::{borrow::Cow, collections::HashMap, error::Error, fmt, str::FromStr};

#[derive(Debug, PartialEq, Eq)]
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
        write!(f, "{:?}", self)
    }
}

impl From<EncodingError> for MagnetURIError {
    fn from(err: EncodingError) -> MagnetURIError {
        match err {
            EncodingError::InvalidHashCharacter => Self::InvalidHashCharacter,
            EncodingError::InvalidHashLength => Self::InvalidHashLength,
        }
    }
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
            for segment in s[(first_percent + 1)..].split('%') {
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

#[derive(Debug, PartialEq, Eq)]
pub enum MagnetHash {
    SHA1([u8; 20]),
    MD5([u8; 16]),
    BTIH([u8; 20]),
    Invalid,
}

impl FromStr for MagnetHash {
    type Err = MagnetURIError;

    fn from_str(s: &str) -> Result<MagnetHash, Self::Err> {
        if let Some(stripped) = s.strip_prefix("urn:sha1:") {
            Ok(MagnetHash::SHA1(bytes_from_base32(stripped)?))
        } else if let Some(stripped) = s.strip_prefix("urn:md5:") {
            Ok(MagnetHash::MD5(bytes_from_hex(stripped)?))
        } else if let Some(stripped) = s.strip_prefix("urn:btih:") {
            if stripped.len() == 40 {
                Ok(MagnetHash::BTIH(bytes_from_hex(stripped)?))
            } else {
                Ok(MagnetHash::BTIH(bytes_from_base32(stripped)?))
            }
        } else {
            Err(MagnetURIError::UnknownHashFunction)
        }
    }
}

#[derive(Debug, PartialEq, Eq)]
pub struct MagnetFile {
    hash: MagnetHash,
    display_name: String,
}

impl Default for MagnetFile {
    fn default() -> Self {
        MagnetFile {
            hash: MagnetHash::Invalid,
            display_name: "".to_string(),
        }
    }
}

#[derive(Debug, PartialEq, Eq)]
pub struct MagnetFiles {
    files: Vec<MagnetFile>,
}

impl FromStr for MagnetFiles {
    type Err = MagnetURIError;

    fn from_str(s: &str) -> Result<MagnetFiles, Self::Err> {
        use MagnetURIError::*;

        if let Some(data) = s.strip_prefix("magnet:?") {
            let mut files: HashMap<&str, MagnetFile> = HashMap::new();
            for serialised_pair in data.split('&') {
                if let Some((key, encoded_value)) = serialised_pair.split_once('=') {
                    let value = uri_decode_value(encoded_value)?;
                    if key.starts_with("xt") {
                        let file_key = key.strip_prefix("xt.").unwrap_or("1");
                        files.entry(file_key).or_default().hash = MagnetHash::from_str(&value)?;
                    } else if key.starts_with("dn") {
                        let file_key = key.strip_prefix("dn.").unwrap_or("1");
                        files.entry(file_key).or_default().display_name = (*value).to_string();
                    }
                } else {
                    todo!("need to log a warning here")
                };
            }

            Ok(MagnetFiles {
                files: files.into_iter().map(|kv_pair| kv_pair.1).collect(),
            })
        } else if !s.starts_with("magnet:") {
            Err(InvalidURIScheme)
        } else {
            Err(InvalidStartCharacter)
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use MagnetURIError::*;
    use test_case::test_case;

    #[test_case(
        "urn:md5:c12fe1c06bba254a9dc9f519b335aa7c",
        MagnetHash::MD5([193, 47, 225, 192, 107, 186, 37, 74, 157, 201, 245, 25, 179, 53, 170, 124]);
        "MD5"
    )]
    // #[test_case(
    //     "urn:sha1:209c8226b299b308beaf2b9cd3fb49212dbd13ec",
    //     MagnetHash::SHA1([32, 156, 130, 38, 178, 153, 179, 8, 190, 175, 43, 156, 211, 251, 73, 33, 45, 189, 19, 236]);
    //     "SHA1"
    // )]
    #[test_case(
        "urn:btih:209c8226b299b308beaf2b9cd3fb49212dbd13ec",
        MagnetHash::BTIH([32, 156, 130, 38, 178, 153, 179, 8, 190, 175, 43, 156, 211, 251, 73, 33, 45, 189, 19, 236]);
        "BTIH"
    )]
    fn hash_from_str(s: &str, expected: MagnetHash) {
        assert_eq!(MagnetHash::from_str(s), Ok(expected));
    }

    #[test]
    fn file_from_str() {
        let magnet1 =
            MagnetFiles::from_str("magnet:?xt=urn:md5:c12fe1c06bba254a9dc9f519b335aa7c1367a88a");
        let magnet2 = MagnetFiles::from_str(
            "magnet:?xt=urn%3Amd5%3Ac12fe1c06bba254a9dc9f519b335aa7c1367a88a",
        );

        assert_eq!(magnet1, magnet2); // Errors

        let magnet1 =
            MagnetFiles::from_str("magnet:?xt.abc=urn:md5:c12fe1c06bba254a9dc9f519b335aa7c");
        let magnet2 = MagnetFiles::from_str(
            "magnet:?xt.abc=urn%3amd5%3Ac12fe1c06bba254a9dc9f519b335aa7c",
        );

        assert_eq!(magnet1, magnet2);
    }

    #[test]
    fn test_uri_decode_value() {
        let no_replace_needed = uri_decode_value("ABCD").unwrap();
        assert!(no_replace_needed.is_borrowed());
        assert_eq!(no_replace_needed, "ABCD");

        let replace_needed = uri_decode_value("%41CD").unwrap();
        assert!(replace_needed.is_owned());
        assert_eq!(replace_needed, "ACD");
    }

    #[test_case("%%"; "Percent Sign")]
    #[test_case("sad#asd"; "Hash Symbol")]
    #[test_case("asd&asd"; "Amperstand")]
    #[test_case("asd?asd"; "Question Mark")]
    fn test_uri_decode_value_invalid(s: &str) {
        assert_eq!(uri_decode_value(s), Err(InvalidUseOfReservedChar));
    }
}
