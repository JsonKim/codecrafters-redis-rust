use std::io::{Error, Read, Write};
use std::net::{TcpListener, TcpStream};

fn handle_client(mut stream: &TcpStream, message: &str) -> Result<(), Error> {
    stream.write(message.as_bytes())?;
    stream.flush()?;
    Ok(())
}

fn buf_to_lines(buf: &[u8]) -> Vec<String> {
    String::from_utf8_lossy(buf)
        .lines()
        .map(|line| line.trim_end().to_string())
        .collect()
}

fn main() {
    let listener = TcpListener::bind("127.0.0.1:6379").unwrap();

    for stream in listener.incoming() {
        match stream {
            Ok(mut stream) => {
                std::thread::spawn(move || {
                    println!("accepted new connection");
                    loop {
                        let mut buf = [0; 1024];
                        let size = stream.read(&mut buf).unwrap_or(0);
                        if size == 0 {
                            break;
                        }

                        let lines = buf_to_lines(&buf);
                        match lines.get(2) {
                            Some(command) if command == "PING" => {
                                if let Err(e) = handle_client(&stream, "+PONG\r\n") {
                                    eprintln!("Error handling client: {}", e);
                                }
                            }
                            Some(command) if command == "ECHO" => {
                                let message = format!("+{}\r\n", lines.get(4).unwrap().to_string());
                                if let Err(e) = handle_client(&stream, &message) {
                                    eprintln!("Error handling client: {}", e);
                                }
                            }
                            _ => {
                                eprintln!("Unknown command: {:?}", lines);
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
