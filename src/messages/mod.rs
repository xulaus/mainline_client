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
pub enum KRPCQuery {
    Ping { id: [u8; 20] },
    FindNode { id: [u8; 20], target: [u8; 20] },
    GetPeers { id: [u8; 20], info_hash: [u8; 20] },
}

#[derive(Debug, PartialEq)]
pub enum KRPCMessageDetails {
    Error(KRPCError),
    Query(KRPCQuery),
}

#[derive(Debug, PartialEq)]
pub struct KRPCMessage {
    transaction_id: String,
    message: KRPCMessageDetails,
}

impl ToBencode for KRPCMessage {
    fn to_bencode(&self) -> String {
        let message_type = match self.message {
            KRPCMessageDetails::Error(_) => "e",
            KRPCMessageDetails::Query(_) => "q",
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
        };
        format!(
            "d{}1:t{}:{}1:y1:{}e",
            message_contents,
            self.transaction_id.len(),
            self.transaction_id,
            message_type
        )
    }
}

impl FromBencode for KRPCMessage {
    fn from_bencode(serialised: &str) -> Result<KRPCMessage, DecodingError> {
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
        let mut transaction_id: Option<&str> = None;
        let mut message_type = MessageType::Unknown;
        let mut query_type = QueryType::Unknown;

        let mut error_details: Option<KRPCError> = None;
        let mut query_details: Option<Dict> = None;
        let top_level = Bencode { buffer: serialised }.as_dict()?;

        for kv in top_level {
            match kv.key {
                "t" => match kv.value {
                    Value::String(v) => transaction_id = Some(v),
                    _ => return Err(DecodingError::RequiredFieldOfWrongType),
                },
                "y" => match kv.value {
                    Value::String("e") => message_type = MessageType::Error,
                    Value::String("q") => message_type = MessageType::Query,
                    Value::String("r") => message_type = MessageType::Response,
                    _ => return Err(DecodingError::RequiredFieldOfWrongType),
                },
                "e" => match kv.value {
                    Value::List(mut list) => {
                        let raw_code = list.next();
                        let raw_message = list.next();

                        let code: u8 = match raw_code {
                            Some(Value::Integer(v)) => v as u8,
                            _ => return Err(DecodingError::RequiredFieldOfWrongType),
                        };
                        let message: String = match raw_message {
                            Some(Value::String(v)) => v.to_string(),
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
                "q" => match kv.value {
                    Value::String("ping") => query_type = QueryType::Ping,
                    Value::String("find_node") => query_type = QueryType::FindNode,
                    Value::String("get_peers") => query_type = QueryType::GetPeers,
                    Value::String("announce_peer") => query_type = QueryType::GetPeers,
                    _ => return Err(DecodingError::RequiredFieldOfWrongType),
                },
                "a" => match kv.value {
                    Value::Dict(dict) => query_details = Some(dict),
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
                    QueryType::Ping => KRPCQuery::Ping { id: [0; 20] },
                    _ => return Err(DecodingError::MissingRequiredField),
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
