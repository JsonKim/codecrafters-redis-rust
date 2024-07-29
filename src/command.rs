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

    let (cmd, args) = array.split_first().unwrap();
    let cmd = match cmd {
        RespData::BulkString(s) => s,
        _ => return None,
    };

    match cmd.to_uppercase().as_str() {
        "PING" => match args {
            [] => Some(RedisCommand::Ping),
            _ => None,
        },
        "ECHO" => match args {
            [RespData::BulkString(message)] => Some(RedisCommand::Echo(message.clone())),
            _ => None,
        },
        "SET" => match args {
            [RespData::BulkString(key), RespData::BulkString(value), _rest @ ..] => {
                Some(RedisCommand::Set(key.clone(), value.clone()))
            }
            _ => None,
        },
        "GET" => match args {
            [RespData::BulkString(key)] => Some(RedisCommand::Get(key.clone())),
            _ => None,
        },
        _ => None,
    }
}
