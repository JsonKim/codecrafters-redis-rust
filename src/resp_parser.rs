use nom::{
    branch::alt,
    bytes::complete::{tag, take, take_until},
    character::complete::{char, crlf},
    combinator::opt,
    multi::count,
    sequence::delimited,
    IResult,
};

#[derive(Debug, PartialEq)]
pub enum RespData {
    SimpleString(String),
    Error(String),
    Integer(i64),
    BulkString(String),
    BulkStringNull,
    Array(Vec<RespData>),
}

fn parse_simple_string(input: &[u8]) -> IResult<&[u8], RespData> {
    let (input, data) = delimited(char('+'), take_until("\r\n"), tag("\r\n"))(input)?;
    Ok((
        input,
        RespData::SimpleString(String::from_utf8_lossy(data).into_owned()),
    ))
}

fn parse_error(input: &[u8]) -> IResult<&[u8], RespData> {
    let (input, data) = delimited(char('-'), take_until("\r\n"), tag("\r\n"))(input)?;
    Ok((
        input,
        RespData::Error(String::from_utf8_lossy(data).into_owned()),
    ))
}

fn parse_integer(input: &[u8]) -> IResult<&[u8], RespData> {
    let (input, data) = delimited(char(':'), take_until("\r\n"), tag("\r\n"))(input)?;
    let data = core::str::from_utf8(data).map_err(|_| {
        nom::Err::Failure(nom::error::Error::new(input, nom::error::ErrorKind::Verify))
    })?;
    let num = data.parse::<i64>().map_err(|_| {
        nom::Err::Failure(nom::error::Error::new(input, nom::error::ErrorKind::Digit))
    })?;
    Ok((input, RespData::Integer(num)))
}

fn parse_bulk_string(input: &[u8]) -> IResult<&[u8], RespData> {
    let (input, len_bytes) = delimited(char('$'), take_until("\r\n"), tag("\r\n"))(input)?;
    let len_str = core::str::from_utf8(len_bytes).map_err(|_| {
        nom::Err::Failure(nom::error::Error::new(input, nom::error::ErrorKind::Verify))
    })?;
    let len = len_str.parse::<i64>().map_err(|_| {
        nom::Err::Failure(nom::error::Error::new(input, nom::error::ErrorKind::Digit))
    })?;

    if len == -1 {
        return Ok((input, RespData::BulkStringNull));
    }

    let (input, data) = take(len as usize)(input)?;
    let (input, is_string) = opt(crlf)(input)?;

    if is_string.is_some() {
        Ok((
            input,
            RespData::BulkString(String::from_utf8_lossy(data).into_owned()),
        ))
    } else {
        Ok((input, RespData::BulkStringNull))
    }
}

fn parse_array(input: &[u8]) -> IResult<&[u8], RespData> {
    let (input, len_bytes) = delimited(char('*'), take_until("\r\n"), tag("\r\n"))(input)?;
    let len_str = core::str::from_utf8(len_bytes).map_err(|_| {
        nom::Err::Failure(nom::error::Error::new(input, nom::error::ErrorKind::Verify))
    })?;
    let len = len_str.parse::<i64>().map_err(|_| {
        nom::Err::Failure(nom::error::Error::new(input, nom::error::ErrorKind::Digit))
    })?;

    let (input, elements) = count(parse_resp, len as usize)(input)?;
    Ok((input, RespData::Array(elements)))
}

pub fn parse_resp(input: &[u8]) -> IResult<&[u8], RespData> {
    alt((
        parse_simple_string,
        parse_error,
        parse_integer,
        parse_bulk_string,
        parse_array,
    ))(input)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn simple_ok() {
        let res = parse_simple_string(b"+OK\r\n");
        assert_eq!(
            res,
            Ok((&b""[..], RespData::SimpleString("OK".to_string())))
        );
    }

    #[test]
    fn integer_ok() {
        let res = parse_integer(b":1000\r\n");
        assert_eq!(res, Ok((&b""[..], RespData::Integer(1000))));
    }

    #[test]
    fn bulkstring_ok() {
        let res = parse_bulk_string(b"$4\r\nPING\r\n");
        assert_eq!(
            res,
            Ok((&b""[..], RespData::BulkString("PING".to_string())))
        );
    }

    #[test]
    fn array_has_one_element() {
        let res = parse_array(b"*1\r\n$4\r\nPING\r\n");
        assert_eq!(
            res,
            Ok((
                &b""[..],
                RespData::Array(vec![RespData::BulkString("PING".to_string())])
            ))
        );
    }

    #[test]
    fn array_has_two_element() {
        let res = parse_array(b"*2\r\n$4\r\nECHO\r\n$5\r\nHello\r\n");
        assert_eq!(
            res,
            Ok((
                &b""[..],
                RespData::Array(vec![
                    RespData::BulkString("ECHO".to_string()),
                    RespData::BulkString("Hello".to_string())
                ])
            ))
        );
    }

    #[test]
    fn array_has_multiple_type_element() {
        let res = parse_array(b"*3\r\n$4\r\nECHO\r\n:1000\r\n$5\r\nHello\r\n");
        assert_eq!(
            res,
            Ok((
                &b""[..],
                RespData::Array(vec![
                    RespData::BulkString("ECHO".to_string()),
                    RespData::Integer(1000),
                    RespData::BulkString("Hello".to_string()),
                ])
            ))
        );
    }

    #[test]
    fn has_remain_input() {
        let res = parse_array(b"*3\r\n$4\r\nECHO\r\n:1000\r\n$5\r\nHello\r\n*3\r\n$4\r\nECHO\r\n:1000\r\n$5\r\nHello\r\n");
        assert_eq!(
            res,
            Ok((
                &b"*3\r\n$4\r\nECHO\r\n:1000\r\n$5\r\nHello\r\n"[..],
                RespData::Array(vec![
                    RespData::BulkString("ECHO".to_string()),
                    RespData::Integer(1000),
                    RespData::BulkString("Hello".to_string()),
                ])
            ))
        );
    }
}
