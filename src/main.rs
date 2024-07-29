use std::io::{Error, Read, Write};
use std::net::{TcpListener, TcpStream};

use clap::{arg, Parser};
use command::{parse_command, RedisCommand};
use resp_parser::parse_resp;
use store::Store;

mod command;
mod resp_parser;
mod store;

#[derive(Parser, Debug)]
struct Args {
    #[arg(long, default_value_t = 6379)]
    port: u16,
}

fn handle_client(mut stream: &TcpStream, message: &str) -> Result<(), Error> {
    stream.write(message.as_bytes())?;
    stream.flush()?;
    Ok(())
}

fn main() {
    let args = Args::parse();
    let listener = TcpListener::bind(format!("127.0.0.1:{}", args.port)).unwrap();
    let store = Store::new();

    for stream in listener.incoming() {
        match stream {
            Ok(mut stream) => {
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
                                let message = "$11\r\nrole:master\r\n".to_string();
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
