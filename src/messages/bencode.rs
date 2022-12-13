use std::str::from_utf8;
use std::{error::Error, fmt};

#[derive(Debug, PartialEq, Eq)]
pub enum DecodingError {
    UnknownError,
    MissingRequiredField,
    RequiredFieldOfWrongType,
    InvalidStringLength,
    InvalidInteger,
    UnexpectedEOF,
}

impl Error for DecodingError {
    fn description(&self) -> &str {
        use DecodingError::*;
        match *self {
            // TODO: non shitify
            UnknownError => "",
            MissingRequiredField => "",
            RequiredFieldOfWrongType => "",
            InvalidStringLength => "",
            InvalidInteger => "",
            UnexpectedEOF => "",
        }
    }
}
impl fmt::Display for DecodingError {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(f, "{:?}", self)
    }
}

pub trait ToBencode {
    fn to_bencode(&self) -> Vec<u8>;
}

pub trait FromBencode<'a>: Sized {
    fn from_bencode(serialised: &'a [u8]) -> Result<Self, DecodingError>;
}

pub struct Bencode<'a> {
    pub buffer: &'a [u8],
}

impl<'a> Bencode<'a> {
    pub fn len(&self) -> usize {
        self.buffer.len()
    }

    pub fn as_dict(&self) -> Result<Dict<'a>, DecodingError> {
        let (dict, leftover) = self.eat_dict()?;
        if leftover.len() > 0 {
            Err(DecodingError::UnknownError)
        } else {
            Ok(dict)
        }
    }

    pub fn eat_integer(&self) -> Result<(&'a [u8], Bencode<'a>), DecodingError> {
        // TODO: Should be errors
        assert!(self.buffer.len() >= 3);
        assert_eq!(self.peek(), Some('i'));
        let mut tokens = self.buffer.splitn(2, |x| *x == b'e');
        let int = tokens.next().ok_or(DecodingError::UnexpectedEOF)?;
        let rest_of_buffer = tokens.next().ok_or(DecodingError::UnexpectedEOF)?;
        Ok((
            &int[1..],
            Bencode {
                buffer: rest_of_buffer,
            },
        ))
    }

    pub fn eat_dict(&self) -> Result<(Dict<'a>, Bencode<'a>), DecodingError> {
        // TODO: Should be errors
        assert!(self.buffer.len() >= 2);
        assert_eq!(self.peek(), Some('d'));

        let mut iter = Dict {
            string: Bencode {
                buffer: &self.buffer[1..],
            },
        };
        while iter.next().is_some() {}
        if iter.string.peek() == Some('e') {
            Ok((
                Dict {
                    string: Bencode {
                        buffer: &self.buffer[1..],
                    },
                },
                Bencode {
                    buffer: &(iter.string.buffer)[1..],
                },
            ))
        } else {
            Err(DecodingError::UnknownError)
        }
    }

    pub fn eat_list(&self) -> Result<(List<'a>, Bencode<'a>), DecodingError> {
        // TODO: Should be errors
        assert!(self.buffer.len() >= 2);
        assert_eq!(self.peek(), Some('l'));

        let mut iter = List {
            string: Bencode {
                buffer: &self.buffer[1..],
            },
        };
        while iter.next().is_some() {}
        if iter.string.peek() == Some('e') {
            Ok((
                List {
                    string: Bencode {
                        buffer: &self.buffer[1..],
                    },
                },
                Bencode {
                    buffer: &(iter.string.buffer)[1..],
                },
            ))
        } else {
            Err(DecodingError::UnknownError)
        }
    }

    pub fn eat_str(&self) -> Result<(&'a [u8], Bencode<'a>), DecodingError> {
        let mut tokens = self.buffer.splitn(2, |x| *x == b':');
        let key_len = tokens.next().ok_or(DecodingError::UnexpectedEOF)?;
        let rest_of_key = tokens.next().ok_or(DecodingError::UnexpectedEOF)?;
        let len_string = from_utf8(key_len)
            .ok()
            .ok_or(DecodingError::InvalidStringLength)?;
        let string_len: usize = len_string
            .parse()
            .ok()
            .ok_or(DecodingError::InvalidStringLength)?;
        let (key, rest_of_buffer) = rest_of_key.split_at(string_len);

        Ok((
            key,
            Bencode {
                buffer: rest_of_buffer,
            },
        ))
    }

    pub fn eat_any(&self) -> Result<(Value<'a>, Bencode<'a>), DecodingError> {
        match self.peek() {
            Some('d') => {
                let (d, b) = self.eat_dict()?;
                Ok((Value::Dict(d), b))
            }
            Some('0'..='9') => {
                let (e, b) = self.eat_str()?;
                Ok((Value::String(e), b))
            }
            Some('l') => {
                let (l, b) = self.eat_list()?;
                Ok((Value::List(l), b))
            }
            Some('i') => {
                let (i, b) = self.eat_integer()?;
                let int_string = from_utf8(i).ok().ok_or(DecodingError::InvalidInteger)?;
                Ok((
                    Value::Integer(
                        int_string
                            .parse()
                            .ok()
                            .ok_or(DecodingError::InvalidInteger)?,
                    ),
                    b,
                ))
            }
            _ => Err(DecodingError::UnknownError),
        }
    }

    pub fn peek(&self) -> Option<char> {
        self.buffer.first().map(|x| *x as char)
    }
}

pub enum Value<'a> {
    String(&'a [u8]),
    Dict(Dict<'a>),
    List(List<'a>),
    Integer(i64),
}

impl<'a> fmt::Debug for Value<'a> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Integer(x) => write!(f, "{}", x),
            Self::String(bytes) => {
                if let Ok(s) = std::str::from_utf8(bytes) {
                    s.fmt(f)
                } else {
                    write!(f, "{:?}", bytes)
                }
            }
            Self::Dict(d) => d.fmt(f),
            Self::List(l) => l.fmt(f),
        }
    }
}
#[derive(Debug)]
pub struct DictKVPair<'a> {
    pub key: &'a [u8],
    pub value: Value<'a>,
}

pub struct Dict<'a> {
    string: Bencode<'a>,
}

impl<'a> fmt::Debug for Dict<'a> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let copy = Dict {
            string: Bencode {
                buffer: self.string.buffer,
            },
        };
        let mut builder = f.debug_struct("");
        for kv in copy {
            let key = format!("{:?}", Value::String(kv.key));
            builder.field(&key, &kv.value);
        }
        builder.finish()
    }
}

impl<'a> Iterator for Dict<'a> {
    type Item = DictKVPair<'a>;
    fn next(&mut self) -> Option<Self::Item> {
        if self.string.peek().map(|x| x == 'e').unwrap_or(true) {
            return None;
        }

        let (key, partial1) = match self.string.eat_str() {
            Ok(t) => t,
            Err(_) => return None,
        };
        let (value, partial2) = match partial1.eat_any() {
            Ok(t) => t,
            Err(_) => return None,
        };
        self.string = partial2;
        Some(DictKVPair { key, value })
    }
}

pub struct List<'a> {
    string: Bencode<'a>,
}

impl<'a> fmt::Debug for List<'a> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let copy = List {
            string: Bencode {
                buffer: self.string.buffer,
            },
        };

        f.debug_list().entries(copy).finish()
    }
}
impl<'a> Iterator for List<'a> {
    type Item = Value<'a>;
    fn next(&mut self) -> Option<Self::Item> {
        if self.string.peek().map(|x| x == 'e').unwrap_or(true) {
            return None;
        }

        let (value, partial) = match self.string.eat_any() {
            Ok(t) => t,
            Err(_) => return None,
        };
        self.string = partial;
        Some(value)
    }
}
