use std::io::{Error, ErrorKind, Read, Write};
use std::net::{TcpListener, TcpStream};
use std::sync::{Arc, Mutex};

use cli::parse_cli;
use command::{parse_command, RedisCommand};
use replica::main_of_replica;
use resp_parser::parse_resp;
use store::Store;
use tcp::send_message_to_client;

mod cli;
mod command;
mod replica;
mod resp_parser;
mod store;
mod tcp;

fn make_bulk_string(data: &str) -> String {
    format!("${}\r\n{}\r\n", data.len(), data)
}

fn decode_hex(s: &str) -> Result<Vec<u8>, Error> {
    if s.len() % 2 != 0 {
        return Err(Error::new(
            ErrorKind::InvalidInput,
            "Odd number of hex digits",
        ));
    }

    (0..s.len())
        .step_by(2)
        .map(|i| {
            u8::from_str_radix(&s[i..i + 2], 16).map_err(|e| Error::new(ErrorKind::InvalidInput, e))
        })
        .collect()
}

fn main() {
    main_of_replica();

    let args = parse_cli();

    let listener = TcpListener::bind(format!("127.0.0.1:{}", args.port)).unwrap();
    let store = Store::new();

    let cloned_client: Arc<Mutex<Option<TcpStream>>> = Arc::new(Mutex::new(None));

    for stream in listener.incoming() {
        match stream {
            Ok(mut stream) => {
                let replicaof = args.replicaof.clone();
                let store = store.clone();
                let cloned_client = Arc::clone(&cloned_client);

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
                                if let Err(e) = send_message_to_client(&stream, "+PONG\r\n") {
                                    eprintln!("Error handling client: {}", e);
                                }
                            }
                            RedisCommand::Echo(message) => {
                                let message = format!("+{}\r\n", message);
                                if let Err(e) = send_message_to_client(&stream, &message) {
                                    eprintln!("Error handling client: {}", e);
                                }
                            }
                            RedisCommand::Set(key, value, px) => {
                                store.set(key, value, px);
                                if let Err(e) = send_message_to_client(&stream, "+OK\r\n") {
                                    eprintln!("Error handling client: {}", e);
                                }

                                let mut cloned_client_lock = cloned_client.lock().unwrap();
                                if let Some(ref mut cloned_client_stream) = *cloned_client_lock {
                                    cloned_client_stream.write_all(&buf[..size]).unwrap();
                                    cloned_client_stream.flush().unwrap();
                                }
                            }
                            RedisCommand::Get(key) => {
                                let message = store
                                    .get(&key)
                                    .map(|v| format!("${}\r\n{}\r\n", v.len(), v))
                                    .unwrap_or("$-1\r\n".to_string());
                                if let Err(e) = send_message_to_client(&stream, &message) {
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
                                if let Err(e) = send_message_to_client(&stream, &message) {
                                    eprintln!("Error handling client: {}", e);
                                }
                            }
                            RedisCommand::ReplConf => {
                                if let Err(e) = send_message_to_client(&stream, "+OK\r\n") {
                                    eprintln!("Error handling client: {}", e);
                                }
                            }
                            RedisCommand::PSync => {
                                let message =
                                    "+FULLRESYNC 8371b4fb1155b71f4a04d3e1bc3e18c4a990aeeb 0\r\n";
                                if let Err(e) = send_message_to_client(&stream, &message) {
                                    eprintln!("Error handling client: {}", e);
                                }

                                let file_content = "524544495330303131fa0972656469732d76657205372e322e30fa0a72656469732d62697473c040fa056374696d65c26d08bc65fa08757365642d6d656dc2b0c41000fa08616f662d62617365c000fff06e3bfec0ff5aa2";
                                let file_content = decode_hex(file_content).unwrap();
                                stream
                                    .write(format!("${}\r\n", file_content.len()).as_bytes())
                                    .unwrap();
                                stream.write(&file_content).unwrap();
                                stream.flush().unwrap();

                                let mut cloned_client_lock = cloned_client.lock().unwrap();
                                *cloned_client_lock = Some(stream.try_clone().unwrap());
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
