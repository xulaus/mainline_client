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

// #[derive(Debug)]
// struct NodeInfo<'a> {
//     id: &'a [u8; 20],
//     ip: &'a [u8; 4],
//     port: &'a [u8; 2]
// }

#[derive(Debug, PartialEq)]
pub enum KRPCResponse<'a> {
    Ping {
        id: &'a [u8; 20],
    },
    FindNode {
        id: &'a [u8; 20],
        // TODO: NodeInfo should go here. spec is ambiguious though, using str for now.
        nodes: &'a [u8],
    },
    GetPeers {
        id: &'a [u8; 20],
        token: &'a [u8],
        // TODO: Values vs NodeInfo to go here
    },
}

#[derive(Debug, PartialEq)]
pub enum KRPCMessageDetails<'a> {
    Error(KRPCError),
    Query(KRPCQuery<'a>),
    Response(KRPCResponse<'a>),
}

#[derive(Debug, PartialEq)]
pub struct KRPCMessage<'a> {
    pub transaction_id: &'a [u8],
    pub message: KRPCMessageDetails<'a>,
}

impl<'a> ToBencode for KRPCMessage<'a> {
    fn to_bencode(&self) -> Vec<u8> {
        // This method is dogshite. Relies on coincidence to order the
        // encoded message correctly. Rewrite would be hard without more allocations though
        // and it works for now
        let mut vec1 = Vec::with_capacity(256);
        vec1.push('d' as u8);

        match &self.message {
            KRPCMessageDetails::Error(err) => match err {
                KRPCError::UnknownError(msg) => {
                    vec1.extend(format!("1:eli201e{}:{}e", msg.len(), msg).bytes())
                }
                KRPCError::GenericError(msg) => {
                    vec1.extend(format!("1:eli201e{}:{}e", msg.len(), msg).bytes())
                }
                KRPCError::ServerError(msg) => {
                    vec1.extend(format!("1:eli202e{}:{}e", msg.len(), msg).bytes())
                }
                KRPCError::ProtocolError(msg) => {
                    vec1.extend(format!("1:eli203e{}:{}e", msg.len(), msg).bytes())
                }
                KRPCError::MethodUnknown(msg) => {
                    vec1.extend(format!("1:eli204e{}:{}e", msg.len(), msg).bytes())
                }
            },
            KRPCMessageDetails::Query(q) => match q {
                KRPCQuery::Ping { id } => {
                    vec1.extend(b"1:ad2:id20:");
                    vec1.extend(*id);
                    vec1.extend(b"e1:q4:ping");
                }
                KRPCQuery::GetPeers { id, info_hash } => {
                    vec1.extend(b"1:ad2:id20:");
                    vec1.extend(*id);
                    vec1.extend(b"9:info_hash20:");
                    vec1.extend(*info_hash);
                    vec1.extend(b"e1:q9:get_peers");
                }
                KRPCQuery::FindNode { id, target } => {
                    vec1.extend(b"1:ad2:id20:");
                    vec1.extend(*id);
                    vec1.extend(b"6:target20:");
                    vec1.extend(*target);
                    vec1.extend(b"e1:q9:find_node");
                }
            },
            KRPCMessageDetails::Response(q) => match q {
                KRPCResponse::Ping { id } => {
                    vec1.extend(b"1:rd2:id20:");
                    vec1.extend(*id);
                    vec1.extend(b"e");
                }
                KRPCResponse::GetPeers { id, token } => {
                    vec1.extend(b"1:rd2:id20:");
                    vec1.extend(*id);
                    vec1.extend(format!("5:token{}:", token.len()).bytes());
                    vec1.extend(*token);
                    vec1.extend(b"e");
                }
                KRPCResponse::FindNode { id, nodes } => {
                    vec1.extend(b"1:rd2:id20:");
                    vec1.extend(*id);
                    vec1.extend(format!("5:nodes{}:", nodes.len()).bytes());
                    vec1.extend(*nodes);
                    vec1.extend(b"e");
                }
            },
        };

        vec1.extend(format!("1:t{}:", self.transaction_id.len()).bytes());
        vec1.extend(self.transaction_id);

        let message_type = match self.message {
            KRPCMessageDetails::Error(_) => 'e' as u8,
            KRPCMessageDetails::Query(_) => 'q' as u8,
            KRPCMessageDetails::Response(_) => 'r' as u8,
        };
        vec1.extend(b"1:y1:");
        vec1.push(message_type);

        vec1.push('e' as u8);
        vec1
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
        // eww

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
        let mut info_hash: Option<&[u8; 20]> = None;
        let mut target: Option<&[u8; 20]> = None;
        let mut token: Option<&[u8]> = None;
        let mut nodes: Option<&[u8]> = None;

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
                            Some(Value::String(v)) => String::from_utf8(v.into())
                                .map_err(|_| DecodingError::RequiredFieldOfWrongType)?,
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
                                b"token" => match qdkv.value {
                                    Value::String(t) => token = Some(t),
                                    _ => return Err(DecodingError::RequiredFieldOfWrongType),
                                },
                                b"nodes" => match qdkv.value {
                                    Value::String(n) => nodes = Some(n),
                                    _ => return Err(DecodingError::RequiredFieldOfWrongType),
                                },
                                _ => (),
                            }
                        }
                    }
                    _ => return Err(DecodingError::RequiredFieldOfWrongType),
                },
                b"a" => match kv.value {
                    Value::Dict(mid) => {
                        for qdkv in mid {
                            match qdkv.key {
                                b"id" => match qdkv.value {
                                    Value::String(id) => other_id = to_20_bytes(id),
                                    _ => return Err(DecodingError::RequiredFieldOfWrongType),
                                },
                                b"info_hash" => match qdkv.value {
                                    Value::String(id) => info_hash = to_20_bytes(id),
                                    _ => return Err(DecodingError::RequiredFieldOfWrongType),
                                },
                                b"target" => match qdkv.value {
                                    Value::String(id) => target = to_20_bytes(id),
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
                    QueryType::GetPeers => KRPCQuery::GetPeers {
                        id: other_id.ok_or(DecodingError::MissingRequiredField)?,
                        info_hash: info_hash.ok_or(DecodingError::MissingRequiredField)?,
                    },
                    QueryType::FindNode => KRPCQuery::FindNode {
                        id: other_id.ok_or(DecodingError::MissingRequiredField)?,
                        target: target.ok_or(DecodingError::MissingRequiredField)?,
                    },
                    _ => return Err(DecodingError::MissingRequiredField),
                }),
                MessageType::Response => {
                    // We have no way of explicitly knowing what we should be
                    // deserialising to. Infer from fields
                    let response = if let Some(unwrapped_token) = token {
                        KRPCResponse::GetPeers {
                            id: other_id.ok_or(DecodingError::MissingRequiredField)?,
                            token: unwrapped_token,
                        }
                    } else if let Some(unwrapped_nodes) = nodes {
                        KRPCResponse::FindNode {
                            id: other_id.ok_or(DecodingError::MissingRequiredField)?,
                            nodes: unwrapped_nodes,
                        }
                    } else if let Some(unwrapped_id) = other_id {
                        KRPCResponse::Ping { id: unwrapped_id }
                    } else {
                        return Err(DecodingError::MissingRequiredField);
                    };
                    KRPCMessageDetails::Response(response)
                }
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
            transaction_id: b"be",
            message: KRPCMessageDetails::Error(KRPCError::ServerError("".to_string())),
        };
        assert_eq!(expected.to_bencode(), b"d1:eli202e0:e1:t2:be1:y1:ee");

        let deserialised1 = KRPCMessage::from_bencode(b"d1:eli202e0:e1:t2:be1:y1:ee");
        assert_eq!(deserialised1, Ok(expected));

        // ignores unknown fields
        let deserialised2 =
            KRPCMessage::from_bencode(b"d3:abc1:d1:eli203e0:1:f4:listl1:a2:xzee1:t0:1:y1:ee");
        assert_eq!(
            deserialised2,
            Ok(KRPCMessage {
                transaction_id: b"",
                message: KRPCMessageDetails::Error(KRPCError::ProtocolError("".to_string()))
            }),
        );

        // MethodUnknown
        let deserialised3 = KRPCMessage::from_bencode(b"d1:eli204e0:e1:t2:ee3:123le1:y1:ee");
        assert_eq!(
            deserialised3,
            Ok(KRPCMessage {
                transaction_id: b"ee",
                message: KRPCMessageDetails::Error(KRPCError::MethodUnknown("".to_string()))
            }),
        );

        // Error examples from spec
        let error_1 = KRPCMessage {
            transaction_id: b"aa",
            message: KRPCMessageDetails::Error(KRPCError::GenericError(
                "A Generic Error Ocurred".to_string(),
            )),
        };
        let error_1_encoded = b"d1:eli201e23:A Generic Error Ocurrede1:t2:aa1:y1:ee";
        let error_1_decoded = KRPCMessage::from_bencode(error_1_encoded);
        assert_eq!(error_1.to_bencode(), error_1_encoded.to_vec());
        assert_eq!(error_1_decoded, Ok(error_1));

        // Ping example from spec
        let ping_query = KRPCMessage {
            transaction_id: b"aa",
            message: KRPCMessageDetails::Query(KRPCQuery::Ping {
                id: b"abcdefghij0123456789",
            }),
        };
        let ping_query_encoded = b"d1:ad2:id20:abcdefghij0123456789e1:q4:ping1:t2:aa1:y1:qe";
        let ping_query_decoded = KRPCMessage::from_bencode(ping_query_encoded);
        assert_eq!(ping_query.to_bencode(), ping_query_encoded.to_vec());
        assert_eq!(ping_query_decoded, Ok(ping_query));

        // Get Peers from spec
        let get_peers_query = KRPCMessage {
            transaction_id: b"aa",
            message: KRPCMessageDetails::Query(KRPCQuery::GetPeers {
                id: b"abcdefghij0123456789",
                info_hash: b"mnopqrstuvwxyz123456",
            }),
        };
        let get_peers_query_encoded = b"d1:ad2:id20:abcdefghij01234567899:info_hash20:mnopqrstuvwxyz123456e1:q9:get_peers1:t2:aa1:y1:qe";
        let get_peers_query_decoded = KRPCMessage::from_bencode(get_peers_query_encoded);
        assert_eq!(
            get_peers_query.to_bencode(),
            get_peers_query_encoded.to_vec()
        );
        assert_eq!(get_peers_query_decoded, Ok(get_peers_query));

        let get_peers_response_peers = KRPCMessage {
            transaction_id: b"aa",
            message: KRPCMessageDetails::Response(KRPCResponse::GetPeers {
                id: b"abcdefghij0123456789",
                token: b"aoeusnth",
            }),
        };
        // d1:rd2:id20:abcdefghij01234567895:token8:aoeusnth6:valuesl6:axje.u6:idhtnmee1:t2:aa1:y1:re
        let get_peers_response_peers_encoded =
            b"d1:rd2:id20:abcdefghij01234567895:token8:aoeusnthe1:t2:aa1:y1:re";
        let get_peers_response_peers_decoded =
            KRPCMessage::from_bencode(get_peers_response_peers_encoded);
        assert_eq!(
            get_peers_response_peers.to_bencode(),
            get_peers_response_peers_encoded.to_vec()
        );
        assert_eq!(
            get_peers_response_peers_decoded,
            Ok(get_peers_response_peers)
        );

        // d1:rd2:id20:abcdefghij01234567895:nodes9:def456...5:token8:aoeusnthe1:t2:aa1:y1:re
        let get_peers_response_nodes = KRPCMessage {
            transaction_id: b"aa",
            message: KRPCMessageDetails::Response(KRPCResponse::GetPeers {
                id: b"abcdefghij0123456789",
                token: b"aoeusnth",
            }),
        };
        let get_peers_response_nodes_encoded =
            b"d1:rd2:id20:abcdefghij01234567895:token8:aoeusnthe1:t2:aa1:y1:re";
        let get_peers_response_nodes_decoded =
            KRPCMessage::from_bencode(get_peers_response_nodes_encoded);
        assert_eq!(
            get_peers_response_nodes.to_bencode(),
            get_peers_response_nodes_encoded.to_vec()
        );
        assert_eq!(
            get_peers_response_nodes_decoded,
            Ok(get_peers_response_nodes)
        );

        // Find Node from spec
        let find_node_query = KRPCMessage {
            transaction_id: b"aa",
            message: KRPCMessageDetails::Query(KRPCQuery::FindNode {
                id: b"abcdefghij0123456789",
                target: b"mnopqrstuvwxyz123456",
            }),
        };
        let find_node_query_encoded = b"d1:ad2:id20:abcdefghij01234567896:target20:mnopqrstuvwxyz123456e1:q9:find_node1:t2:aa1:y1:qe";
        let find_node_query_decoded = KRPCMessage::from_bencode(find_node_query_encoded);
        assert_eq!(
            find_node_query.to_bencode(),
            find_node_query_encoded.to_vec()
        );
        assert_eq!(find_node_query_decoded, Ok(find_node_query));

        let find_node_response = KRPCMessage {
            transaction_id: b"aa",
            message: KRPCMessageDetails::Response(KRPCResponse::FindNode {
                id: b"0123456789abcdefghij",
                nodes: b"def456...",
            }),
        };
        let find_node_response_encoded =
            b"d1:rd2:id20:0123456789abcdefghij5:nodes9:def456...e1:t2:aa1:y1:re";
        let find_node_response_decoded = KRPCMessage::from_bencode(find_node_response_encoded);
        assert_eq!(
            find_node_response.to_bencode(),
            find_node_response_encoded.to_vec()
        );
        assert_eq!(find_node_response_decoded, Ok(find_node_response));
    }
}
