use std::{io::Read, net::TcpStream};

use crate::{cli::parse_cli, tcp::send_message_to_client};

fn run_client(host: &str, port: u16) {
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

    loop {
        let mut buffer = [0; 1024];
        let bytes_read = stream.read(&mut buffer).unwrap();

        if bytes_read == 0 {
            println!("Server closed the connection");
            break;
        }

        let received = String::from_utf8_lossy(&buffer[..bytes_read]);
        println!("Received: {}", received);
    }
}

pub fn main_of_replica() {
    std::thread::spawn(|| {
        let args = parse_cli();

        args.replicaof
            .map(|replica| run_client(&replica.host, replica.port));
    });
}
