use std::{
    io::{Error, Write},
    net::TcpStream,
};

pub fn send_message_to_client(mut stream: &TcpStream, message: &str) -> Result<(), Error> {
    stream.write(message.as_bytes())?;
    stream.flush()?;
    Ok(())
}
