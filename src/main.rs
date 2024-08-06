use std::collections::HashMap;
use std::io::{Error, ErrorKind, Read, Write};
use std::net::{TcpListener, TcpStream};
use std::sync::{mpsc, Arc, Mutex, RwLock};

use cli::parse_cli;
use command::{parse_command, ConfigGet, RedisCommand, ReplConf};
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

enum Message {
    NewConnection(TcpStream),
    DisconnectReplica(TcpStream),
    Data(Vec<u8>),
    Set(String, String, Option<u64>),
    Get(TcpStream, String),
    WaitHandshake(TcpStream, u64, u64),
    UpdateOffset(TcpStream, u64),
}

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
    let listener = TcpListener::bind(format!("127.0.0.1:{}", parse_cli().port)).unwrap();

    let (tx, rx) = mpsc::channel();

    main_of_replica(&tx);

    let _ = {
        let tx = tx.clone();
        std::thread::spawn(move || {
            let replicas: Arc<Mutex<HashMap<core::net::SocketAddr, (TcpStream, u64)>>> =
                Arc::new(Mutex::new(HashMap::new()));
            let store = Store::new();

            let last_write_bytes = Arc::new(RwLock::new(0));
            let total_write_bytes = Arc::new(RwLock::new(0));

            for message in rx {
                match message {
                    Message::NewConnection(stream) => {
                        println!("New connection established");
                        let mut replicas = replicas.lock().unwrap();
                        replicas.insert(stream.peer_addr().unwrap(), (stream, 0));
                    }
                    Message::DisconnectReplica(stream) => {
                        let mut replicas = replicas.lock().unwrap();
                        replicas.remove(&stream.peer_addr().unwrap());
                    }
                    Message::Data(data) => {
                        let mut replicas = replicas.lock().unwrap();

                        for (replica, _) in replicas.values_mut() {
                            replica.write_all(&data).unwrap();
                            replica.flush().unwrap();
                        }

                        let mut last_write_bytes = last_write_bytes.write().unwrap();
                        *last_write_bytes = data.len();

                        let mut total_write_bytes = total_write_bytes.write().unwrap();
                        *total_write_bytes += data.len();
                    }
                    Message::Set(key, value, px) => {
                        store.set(key, value, px);
                    }
                    Message::Get(stream, key) => {
                        let message = store
                            .get(&key)
                            .map(|v| format!("${}\r\n{}\r\n", v.len(), v))
                            .unwrap_or("$-1\r\n".to_string());
                        if let Err(e) = send_message_to_client(&stream, &message) {
                            eprintln!("Error handling client: {}", e);
                        }
                    }
                    Message::WaitHandshake(stream, numreplicas, timeout) => {
                        if *total_write_bytes.read().unwrap() != 0 {
                            let data_of_getack = format!(
                                "*3\r\n{}{}{}",
                                make_bulk_string("REPLCONF"),
                                make_bulk_string("GETACK"),
                                make_bulk_string("*")
                            );
                            tx.send(Message::Data(data_of_getack.as_bytes().to_vec()))
                                .unwrap();
                        }

                        if numreplicas == 0 {
                            let message = format!(":{}\r\n", 0);
                            if let Err(e) = send_message_to_client(&stream, &message) {
                                eprintln!("Error handling client: {}", e);
                            }

                            break;
                        }

                        let replicas = Arc::clone(&replicas);
                        let last_write_bytes = Arc::clone(&last_write_bytes);
                        let total_write_bytes = Arc::clone(&total_write_bytes);
                        std::thread::spawn(move || {
                            std::thread::sleep(std::time::Duration::from_millis(timeout));

                            let replicas = replicas.lock().unwrap();
                            let last_write_bytes = last_write_bytes.read().unwrap();
                            let total_write_bytes = total_write_bytes.read().unwrap();
                            let synced_replica_count = replicas
                                .values()
                                .filter(|(_, offset)| {
                                    *offset == (*total_write_bytes - *last_write_bytes) as u64
                                })
                                .count();

                            let message = format!(":{}\r\n", synced_replica_count);
                            if let Err(e) = send_message_to_client(&stream, &message) {
                                eprintln!("Error handling client: {}", e);
                            }
                        });
                    }
                    Message::UpdateOffset(stream, offset) => {
                        let mut replicas = replicas.lock().unwrap();
                        replicas
                            .entry(stream.peer_addr().unwrap())
                            .and_modify(|v| v.1 = offset);
                    }
                }
            }
        });
    };

    for stream in listener.incoming() {
        match stream {
            Ok(mut stream) => {
                let tx = tx.clone();

                std::thread::spawn(move || {
                    println!("accepted new connection");

                    let mut is_replica = false;
                    loop {
                        let mut buf = [0; 1024];
                        let size = stream.read(&mut buf).unwrap_or(0);
                        if size == 0 {
                            break;
                        }

                        let resp = parse_resp(&buf[..size]).unwrap().1;
                        let command = parse_command(&resp).map_or_else(
                            || {
                                println!(
                                    "Invalid command: {}",
                                    String::from_utf8_lossy(&buf[..size])
                                );
                                None
                            },
                            Some,
                        );

                        if command.is_none() {
                            continue;
                        }

                        match command.unwrap() {
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
                                tx.send(Message::Set(key, value, px)).unwrap();
                                if let Err(e) = send_message_to_client(&stream, "+OK\r\n") {
                                    eprintln!("Error handling client: {}", e);
                                }

                                tx.send(Message::Data(buf[..size].to_vec())).unwrap();
                            }
                            RedisCommand::Get(key) => {
                                tx.send(Message::Get(stream.try_clone().unwrap(), key))
                                    .unwrap();
                            }
                            RedisCommand::Info => {
                                let role = match parse_cli().replicaof {
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
                            RedisCommand::ReplConf(ReplConf::Ack(offset)) => {
                                tx.send(Message::UpdateOffset(stream.try_clone().unwrap(), offset))
                                    .unwrap();
                            }
                            RedisCommand::ReplConf(_) => {
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

                                is_replica = true;
                                tx.send(Message::NewConnection(stream.try_clone().unwrap()))
                                    .unwrap();
                            }
                            RedisCommand::Wait(numreplicas, timeout) => {
                                tx.send(Message::WaitHandshake(
                                    stream.try_clone().unwrap(),
                                    numreplicas,
                                    timeout,
                                ))
                                .unwrap();
                            }
                            RedisCommand::ConfigGet(ConfigGet::Dir) => {
                                let args = parse_cli();
                                let message = format!(
                                    "*2\r\n{}{}",
                                    make_bulk_string("dir"),
                                    make_bulk_string(&args.dir)
                                );
                                if let Err(e) = send_message_to_client(&stream, &message) {
                                    eprintln!("Error handling client: {}", e);
                                }
                            }
                            RedisCommand::ConfigGet(ConfigGet::Dbfilename) => {
                                let args = parse_cli();
                                let message = format!(
                                    "*2\r\n{}{}",
                                    make_bulk_string("dbfilename"),
                                    make_bulk_string(&args.dbfilename)
                                );
                                if let Err(e) = send_message_to_client(&stream, &message) {
                                    eprintln!("Error handling client: {}", e);
                                }
                            }
                        }
                    }

                    if is_replica {
                        tx.send(Message::DisconnectReplica(stream.try_clone().unwrap()))
                            .unwrap();
                    }
                });
            }
            Err(e) => {
                println!("error: {}", e);
            }
        }
    }
}
