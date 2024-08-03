use std::{io::Read, net::TcpStream, sync::mpsc::Sender};

use crate::{
    cli::parse_cli,
    command::{parse_command, RedisCommand, ReplConf},
    resp_parser::parse_resp,
    tcp::send_message_to_client,
    Message,
};

fn run_client(tx: &Sender<Message>, host: &str, port: u16) {
    let args = parse_cli();

    let mut stream = TcpStream::connect(format!("{}:{}", host, port)).unwrap();
    let message = "*1\r\n$4\r\nPING\r\n";
    send_message_to_client(&stream, &message).unwrap();
    let _ = stream.read(&mut [0; 128]);

    let message = format!(
        "*3\r\n$8\r\nREPLCONF\r\n$14\r\nlistening-port\r\n${}\r\n{}\r\n",
        args.port.to_string().len(),
        args.port
    );
    send_message_to_client(&stream, &message).unwrap();
    let _ = stream.read(&mut [0; 128]);

    let message = format!("*3\r\n$8\r\nREPLCONF\r\n$4\r\ncapa\r\n$6\r\npsync2\r\n");
    send_message_to_client(&stream, &message).unwrap();
    let _ = stream.read(&mut [0; 128]);

    let message = "*3\r\n$5\r\nPSYNC\r\n$1\r\n?\r\n$2\r\n-1\r\n";
    send_message_to_client(&stream, &message).unwrap();

    let mut input = Vec::new();

    loop {
        let current_input = if input.is_empty() {
            let mut buf = [0; 1024];
            let bytes_read = stream.read(&mut buf).unwrap();

            if bytes_read == 0 {
                println!("Server closed the connection");
                break;
            }

            buf[..bytes_read].to_vec()
        } else {
            input.clone()
        };

        let (next_input, resp) = parse_resp(&current_input).unwrap();
        if parse_command(&resp).is_some() {
            input = current_input;
            break;
        } else {
            input = next_input.to_vec();
        }
    }

    let mut offset = 0;

    loop {
        let current_input = if input.is_empty() {
            let mut buf = [0; 1024];
            let bytes_read = stream.read(&mut buf).unwrap();

            if bytes_read == 0 {
                println!("Server closed the connection");
                break;
            }

            buf[..bytes_read].to_vec()
        } else {
            input
        };

        let (next_input, resp) = parse_resp(&current_input).unwrap();
        input = next_input.to_vec();

        let command = parse_command(&resp).unwrap();
        match command {
            RedisCommand::ReplConf(ReplConf::GetAck) => {
                let message = format!(
                    "*3\r\n$8\r\nREPLCONF\r\n$3\r\nACK\r\n${}\r\n{}\r\n",
                    offset.to_string().len(),
                    offset
                );

                send_message_to_client(&stream, &message).unwrap();
            }
            RedisCommand::Set(key, value, px) => {
                tx.send(Message::Set(key, value, px)).unwrap();
            }
            _ => {}
        }

        offset += current_input.len();
    }
}

pub fn main_of_replica(tx: &Sender<Message>) {
    let tx = tx.clone();
    std::thread::spawn(move || {
        let args = parse_cli();

        args.replicaof
            .map(|replica| run_client(&tx, &replica.host, replica.port));
    });
}
