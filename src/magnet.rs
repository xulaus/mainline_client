use mainline_client::encodings::{hex_to_byte, bytes_from_base32, bytes_from_hex, EncodingError};

use std::{borrow::Cow, collections::HashMap, error::Error, fmt, str::FromStr};

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
