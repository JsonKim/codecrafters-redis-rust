use std::{io::Read, net::TcpStream};

use crate::{cli::parse_cli, tcp::send_message_to_client};

pub fn main_of_replica() {
    let args = parse_cli();

    args.replicaof.map(|replica| {
        let mut stream = TcpStream::connect(format!("{}:{}", replica.host, replica.port)).unwrap();
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
    });
}
