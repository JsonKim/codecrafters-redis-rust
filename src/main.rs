use std::io::{Error, Read, Write};
use std::net::{TcpListener, TcpStream};

use cli::parse_cli;
use command::{parse_command, RedisCommand};
use resp_parser::parse_resp;
use store::Store;

mod cli;
mod command;
mod resp_parser;
mod store;

fn make_bulk_string(data: &str) -> String {
    format!("${}\r\n{}\r\n", data.len(), data)
}

fn handle_client(mut stream: &TcpStream, message: &str) -> Result<(), Error> {
    stream.write(message.as_bytes())?;
    stream.flush()?;
    Ok(())
}

fn main() {
    let args = parse_cli();

    let replicaof = args.replicaof.as_ref();
    let _ = replicaof.map(|replica| {
        let stream = TcpStream::connect(format!("{}:{}", replica.host, replica.port)).unwrap();
        let message = "*1\r\n$4\r\nPING\r\n";
        handle_client(&stream, &message).unwrap();
        stream
    });

    let listener = TcpListener::bind(format!("127.0.0.1:{}", args.port)).unwrap();
    let store = Store::new();

    for stream in listener.incoming() {
        match stream {
            Ok(mut stream) => {
                let replicaof = args.replicaof.clone();
                let store = store.clone();
                std::thread::spawn(move || {
                    println!("accepted new connection");
                    loop {
                        let mut buf = [0; 1024];
                        let size = stream.read(&mut buf).unwrap_or(0);
                        if size == 0 {
                            break;
                        }

                        let resp = parse_resp(&String::from_utf8_lossy(&buf)).unwrap().1;
                        let command = parse_command(&resp).unwrap();
                        match command {
                            RedisCommand::Ping => {
                                if let Err(e) = handle_client(&stream, "+PONG\r\n") {
                                    eprintln!("Error handling client: {}", e);
                                }
                            }
                            RedisCommand::Echo(message) => {
                                let message = format!("+{}\r\n", message);
                                if let Err(e) = handle_client(&stream, &message) {
                                    eprintln!("Error handling client: {}", e);
                                }
                            }
                            RedisCommand::Set(key, value, px) => {
                                store.set(key, value, px);
                                if let Err(e) = handle_client(&stream, "+OK\r\n") {
                                    eprintln!("Error handling client: {}", e);
                                }
                            }
                            RedisCommand::Get(key) => {
                                let message = store
                                    .get(&key)
                                    .map(|v| format!("${}\r\n{}\r\n", v.len(), v))
                                    .unwrap_or("$-1\r\n".to_string());
                                if let Err(e) = handle_client(&stream, &message) {
                                    eprintln!("Error handling client: {}", e);
                                }
                            }
                            RedisCommand::Info => {
                                let role = match replicaof {
                                    Some(_) => "slave",
                                    None => "master",
                                };

                                let info = [
                                    format!("role:{}", role),
                                    "master_replid:8371b4fb1155b71f4a04d3e1bc3e18c4a990aeeb"
                                        .to_string(),
                                    "master_repl_offset:0".to_string(),
                                ]
                                .join("\r\n");
                                let message = make_bulk_string(&info);
                                if let Err(e) = handle_client(&stream, &message) {
                                    eprintln!("Error handling client: {}", e);
                                }
                            }
                        }
                    }
                });
            }
            Err(e) => {
                println!("error: {}", e);
            }
        }
    }
}
