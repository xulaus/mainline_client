pub mod bencode;
use bencode::*;

#[derive(Debug, PartialEq)]
pub enum KRPCError {
    UnknownError(String),
    GenericError(String),
    ServerError(String),
    ProtocolError(String),
    MethodUnknown(String),
}

#[derive(Debug, PartialEq)]
pub enum KRPCQuery<'a> {
    Ping {
        id: &'a [u8; 20],
    },
    FindNode {
        id: &'a [u8; 20],
        target: &'a [u8; 20],
    },
    GetPeers {
        id: &'a [u8; 20],
        info_hash: &'a [u8; 20],
    },
}

#[derive(Debug, PartialEq)]
pub enum KRPCResponse<'a> {
    Ping { id: &'a [u8; 20] },
}

#[derive(Debug, PartialEq)]
pub enum KRPCMessageDetails<'a> {
    Error(KRPCError),
    Query(KRPCQuery<'a>),
    Response(KRPCResponse<'a>),
}

#[derive(Debug, PartialEq)]
pub struct KRPCMessage<'a> {
    transaction_id: &'a [u8],
    message: KRPCMessageDetails<'a>,
}

impl<'a> ToBencode for KRPCMessage<'a> {
    fn to_bencode(&self) -> String {
        let message_type = match self.message {
            KRPCMessageDetails::Error(_) => "e",
            KRPCMessageDetails::Query(_) => "q",
            KRPCMessageDetails::Response(_) => "r",
        };

        let message_contents = match &self.message {
            KRPCMessageDetails::Error(err) => match err {
                KRPCError::UnknownError(msg) => format!("1:eli201e{}:{}e", msg.len(), msg),
                KRPCError::GenericError(msg) => format!("1:eli201e{}:{}e", msg.len(), msg),
                KRPCError::ServerError(msg) => format!("1:eli202e{}:{}e", msg.len(), msg),
                KRPCError::ProtocolError(msg) => format!("1:eli203e{}:{}e", msg.len(), msg),
                KRPCError::MethodUnknown(msg) => format!("1:eli204e{}:{}e", msg.len(), msg),
            },
            KRPCMessageDetails::Query(q) => match q {
                _ => "".to_string(),
            },
            KRPCMessageDetails::Response(q) => match q {
                _ => "".to_string(),
            },
        };
        format!(
            "d{}1:t{}:{:x?}1:y1:{}e",
            message_contents,
            self.transaction_id.len(),
            self.transaction_id,
            message_type
        )
    }
}

fn to_20_bytes<'a>(i: &'a [u8]) -> Option<&'a [u8; 20]> {
    if i.len() == 20 {
        Some(unsafe { ::std::mem::transmute(i.as_ptr()) })
    } else {
        None
    }
}

impl<'a> FromBencode<'a> for KRPCMessage<'a> {
    fn from_bencode(serialised: &'a [u8]) -> Result<KRPCMessage, DecodingError> {
        enum MessageType {
            Query,
            Error,
            Response,
            Unknown,
        }
        enum QueryType {
            Ping,
            FindNode,
            GetPeers,
            AnnouncePeer,
            Unknown,
        };
        let mut transaction_id: Option<&[u8]> = None;
        let mut message_type = MessageType::Unknown;
        let mut query_type = QueryType::Unknown;
        let mut other_id: Option<&[u8; 20]> = None;

        let mut error_details: Option<KRPCError> = None;
        let top_level = Bencode { buffer: serialised }.as_dict()?;

        for kv in top_level {
            match kv.key {
                b"t" => match kv.value {
                    Value::String(v) => transaction_id = Some(v),
                    _ => return Err(DecodingError::RequiredFieldOfWrongType),
                },
                b"y" => match kv.value {
                    Value::String(b"e") => message_type = MessageType::Error,
                    Value::String(b"q") => message_type = MessageType::Query,
                    Value::String(b"r") => message_type = MessageType::Response,
                    _ => return Err(DecodingError::RequiredFieldOfWrongType),
                },
                b"e" => match kv.value {
                    Value::List(mut list) => {
                        let raw_code = list.next();
                        let raw_message = list.next();

                        let code: u8 = match raw_code {
                            Some(Value::Integer(v)) => v as u8,
                            _ => return Err(DecodingError::RequiredFieldOfWrongType),
                        };
                        let message: String = match raw_message {
                            Some(Value::String(v)) => format!("{:x?}", v),
                            _ => return Err(DecodingError::RequiredFieldOfWrongType),
                        };
                        error_details = Some(match code {
                            201 => KRPCError::GenericError(message),
                            202 => KRPCError::ServerError(message),
                            203 => KRPCError::ProtocolError(message),
                            204 => KRPCError::MethodUnknown(message),
                            _ => KRPCError::UnknownError(message),
                        });
                    }
                    _ => return Err(DecodingError::RequiredFieldOfWrongType),
                },
                b"q" => match kv.value {
                    Value::String(b"ping") => query_type = QueryType::Ping,
                    Value::String(b"find_node") => query_type = QueryType::FindNode,
                    Value::String(b"get_peers") => query_type = QueryType::GetPeers,
                    Value::String(b"announce_peer") => query_type = QueryType::GetPeers,
                    _ => return Err(DecodingError::RequiredFieldOfWrongType),
                },
                b"r" => match kv.value {
                    Value::Dict(mid) => {
                        for qdkv in mid {
                            match qdkv.key {
                                b"id" => match qdkv.value {
                                    Value::String(id) => other_id = to_20_bytes(id),
                                    _ => return Err(DecodingError::RequiredFieldOfWrongType),
                                },
                                _ => (),
                            }
                        }
                    }
                    _ => return Err(DecodingError::RequiredFieldOfWrongType),
                },
                _ => (),
            }
        }

        Ok(KRPCMessage {
            transaction_id: transaction_id
                .ok_or(DecodingError::MissingRequiredField)?
                .into(),
            message: match message_type {
                MessageType::Error => KRPCMessageDetails::Error(
                    error_details.ok_or(DecodingError::MissingRequiredField)?,
                ),
                MessageType::Query => KRPCMessageDetails::Query(match query_type {
                    QueryType::Ping => KRPCQuery::Ping {
                        id: other_id.ok_or(DecodingError::MissingRequiredField)?,
                    },
                    _ => return Err(DecodingError::MissingRequiredField),
                }),
                MessageType::Response => KRPCMessageDetails::Response(KRPCResponse::Ping {
                    id: other_id.ok_or(DecodingError::MissingRequiredField)?,
                }),
                _ => return Err(DecodingError::MissingRequiredField),
            },
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn serialise_deserialise() {
        // Test serialise/deserialise error
        let expected = KRPCMessage {
            transaction_id: "be".to_string(),
            message: KRPCMessageDetails::Error(KRPCError::ServerError("".to_string())),
        };
        assert_eq!(expected.to_bencode(), "d1:eli202e0:e1:t2:be1:y1:ee");

        let deserialised1 = KRPCMessage::from_bencode("d1:eli202e0:e1:t2:be1:y1:ee");
        assert_eq!(deserialised1, Ok(expected));

        // ignores unknown fields
        let deserialised2 =
            KRPCMessage::from_bencode("d3:abc1:d1:eli203e0:1:f4:listl1:a2:xzee1:t0:1:y1:ee");
        assert_eq!(
            deserialised2,
            Ok(KRPCMessage {
                transaction_id: "".to_string(),
                message: KRPCMessageDetails::Error(KRPCError::ProtocolError("".to_string()))
            }),
        );

        // MethodUnknown
        let deserialised3 = KRPCMessage::from_bencode("d1:eli204e0:e1:t2:ee3:123le1:y1:ee");
        assert_eq!(
            deserialised3,
            Ok(KRPCMessage {
                transaction_id: "ee".to_string(),
                message: KRPCMessageDetails::Error(KRPCError::MethodUnknown("".to_string()))
            }),
        );

        // Error examples from spec
        let krpc_error_1 =
            KRPCMessage::from_bencode("d1:eli201e23:A Generic Error Ocurrede1:t2:aa1:y1:ee");
        assert_eq!(
            krpc_error_1,
            Ok(KRPCMessage {
                transaction_id: "aa".to_string(),
                message: KRPCMessageDetails::Error(KRPCError::GenericError(
                    "A Generic Error Ocurred".to_string(),
                ))
            })
        );
    }
}
