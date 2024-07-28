use crate::resp_parser::RespData;

#[derive(Debug, PartialEq)]
pub enum RedisCommand {
    Ping,
    Echo(String),
    Set(String, String),
    Get(String),
}

pub fn parse_command(data: &RespData) -> Option<RedisCommand> {
    let array = match data {
        RespData::Array(arr) => arr,
        _ => return None,
    };

    let cmd = match array.get(0) {
        Some(RespData::BulkString(s)) => s,
        _ => return None,
    };

    match cmd.to_uppercase().as_str() {
        "PING" => {
            if array.len() == 1 {
                Some(RedisCommand::Ping)
            } else {
                None
            }
        }
        "ECHO" => {
            if array.len() == 2 {
                match array.get(1) {
                    Some(RespData::BulkString(message)) => {
                        Some(RedisCommand::Echo(message.clone()))
                    }
                    _ => None,
                }
            } else {
                None
            }
        }
        "SET" => {
            if array.len() == 3 {
                match (array.get(1), array.get(2)) {
                    (Some(RespData::BulkString(key)), Some(RespData::BulkString(value))) => {
                        Some(RedisCommand::Set(key.clone(), value.clone()))
                    }
                    _ => None,
                }
            } else {
                None
            }
        }
        "GET" => {
            if array.len() == 2 {
                match array.get(1) {
                    Some(RespData::BulkString(key)) => Some(RedisCommand::Get(key.clone())),
                    _ => None,
                }
            } else {
                None
            }
        }
        _ => None,
    }
}
