use std::io::{Error, Read, Write};
use std::net::{TcpListener, TcpStream};

fn handle_client(mut stream: &TcpStream, message: &str) -> Result<(), Error> {
    stream.write(message.as_bytes())?;
    stream.flush()?;
    Ok(())
}

fn main() {
    let listener = TcpListener::bind("127.0.0.1:6379").unwrap();

    for stream in listener.incoming() {
        match stream {
            Ok(mut stream) => {
                println!("accepted new connection");
                loop {
                    let mut buf = [0; 1024];
                    let size = stream.read(&mut buf).unwrap_or(0);
                    if size == 0 {
                        break;
                    }

                    if let Err(e) = handle_client(&stream, "+PONG\r\n") {
                        eprintln!("Error handling client: {}", e);
                    }
                }
            }
            Err(e) => {
                println!("error: {}", e);
            }
        }
    }
}
