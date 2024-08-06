use crate::resp_parser::RespData;

#[derive(Debug, PartialEq)]
pub enum ReplConf {
    ListeningPort(u16),
    Capa,
    GetAck,
    Ack(u64),
}

#[derive(Debug, PartialEq)]
pub enum ConfigGet {
    Dir,
    Dbfilename,
}

#[derive(Debug, PartialEq)]
pub enum RedisCommand {
    Ping,
    Echo(String),
    Set(String, String, Option<u64>),
    Get(String),
    Info,
    ReplConf(ReplConf),
    PSync,
    Wait(u64, u64),
    ConfigGet(ConfigGet),
}

fn parse_px(args: &[RespData]) -> Option<u64> {
    match args {
        [RespData::BulkString(px), RespData::BulkString(ms)] if px.to_uppercase() == "PX" => {
            ms.parse().ok()
        }
        _ => None,
    }
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
            [RespData::BulkString(key), RespData::BulkString(value), rest @ ..] => {
                let px = parse_px(rest);
                Some(RedisCommand::Set(key.clone(), value.clone(), px))
            }
            _ => None,
        },
        "GET" => match args {
            [RespData::BulkString(key)] => Some(RedisCommand::Get(key.clone())),
            _ => None,
        },
        "INFO" => match args {
            [RespData::BulkString(role)] if role == "replication" => Some(RedisCommand::Info),
            _ => None,
        },
        "REPLCONF" => match args {
            [RespData::BulkString(conf), RespData::BulkString(value)] => {
                match conf.to_uppercase().as_str() {
                    "LISTENING-PORT" => {
                        let port = value.parse().ok()?;
                        Some(RedisCommand::ReplConf(ReplConf::ListeningPort(port)))
                    }
                    "CAPA" => Some(RedisCommand::ReplConf(ReplConf::Capa)),
                    "GETACK" => Some(RedisCommand::ReplConf(ReplConf::GetAck)),
                    "ACK" => {
                        let offset = value.parse().ok()?;
                        Some(RedisCommand::ReplConf(ReplConf::Ack(offset)))
                    }
                    _ => None,
                }
            }
            _ => None,
        },
        "PSYNC" => Some(RedisCommand::PSync),
        "WAIT" => match args {
            [RespData::BulkString(numreplicas), RespData::BulkString(timeout)] => Some(
                RedisCommand::Wait(numreplicas.parse().unwrap(), timeout.parse().unwrap()),
            ),
            _ => None,
        },
        "CONFIG" => match args {
            [RespData::BulkString(subcommand), RespData::BulkString(parameter)]
                if subcommand.to_uppercase() == "GET" =>
            {
                if parameter == "dir" {
                    Some(RedisCommand::ConfigGet(ConfigGet::Dir))
                } else if parameter == "dir" {
                    Some(RedisCommand::ConfigGet(ConfigGet::Dbfilename))
                } else {
                    None
                }
            }
            _ => None,
        },
        _ => None,
    }
}
