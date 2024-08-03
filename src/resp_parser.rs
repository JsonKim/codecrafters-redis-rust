use nom::{
    branch::alt,
    bytes::complete::{tag, take, take_until},
    character::complete::char,
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

fn parse_simple_string(input: &str) -> IResult<&str, RespData> {
    let (input, data) = delimited(char('+'), take_until("\r\n"), tag("\r\n"))(input)?;
    Ok((input, RespData::SimpleString(data.to_string())))
}

fn parse_error(input: &str) -> IResult<&str, RespData> {
    let (input, data) = delimited(char('-'), take_until("\r\n"), tag("\r\n"))(input)?;
    Ok((input, RespData::Error(data.to_string())))
}

fn parse_integer(input: &str) -> IResult<&str, RespData> {
    let (input, data) = delimited(char(':'), take_until("\r\n"), tag("\r\n"))(input)?;
    let num = data.parse::<i64>().map_err(|_| {
        nom::Err::Failure(nom::error::Error::new(input, nom::error::ErrorKind::Digit))
    })?;
    Ok((input, RespData::Integer(num)))
}

fn parse_bulk_string(input: &str) -> IResult<&str, RespData> {
    let (input, len_str) = delimited(char('$'), take_until("\r\n"), tag("\r\n"))(input)?;
    let len = len_str.parse::<i64>().map_err(|_| {
        nom::Err::Failure(nom::error::Error::new(input, nom::error::ErrorKind::Digit))
    })?;

    if len == -1 {
        Ok((input, RespData::BulkStringNull))
    } else {
        let (input, data) = take(len as usize)(input)?;
        let (input, _) = tag("\r\n")(input)?;
        Ok((input, RespData::BulkString(data.to_string())))
    }
}

pub fn parse_bulk_bytes(input: &[u8]) -> IResult<&[u8], RespData> {
    let (input, len_bytes) = delimited(char('$'), take_until("\r\n"), tag("\r\n"))(input)?;
    let len_str = core::str::from_utf8(len_bytes).map_err(|_| {
        nom::Err::Failure(nom::error::Error::new(input, nom::error::ErrorKind::Verify))
    })?;
    let len = len_str.parse::<i64>().map_err(|_| {
        nom::Err::Failure(nom::error::Error::new(input, nom::error::ErrorKind::Verify))
    })?;

    if len == -1 {
        Ok((input, RespData::BulkStringNull))
    } else {
        let (input, _) = take(len as usize)(input)?;
        Ok((input, RespData::BulkStringNull))
    }
}

fn parse_array(input: &str) -> IResult<&str, RespData> {
    let (input, len_str) = delimited(char('*'), take_until("\r\n"), tag("\r\n"))(input)?;
    let len = len_str.parse::<i64>().map_err(|_| {
        nom::Err::Failure(nom::error::Error::new(input, nom::error::ErrorKind::Digit))
    })?;

    let (input, elements) = count(parse_resp, len as usize)(input)?;
    Ok((input, RespData::Array(elements)))
}

pub fn parse_resp(input: &str) -> IResult<&str, RespData> {
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
        let res = parse_simple_string("+OK\r\n");
        assert_eq!(res, Ok(("", RespData::SimpleString("OK".to_string()))));
    }

    #[test]
    fn integer_ok() {
        let res = parse_integer(":1000\r\n");
        assert_eq!(res, Ok(("", RespData::Integer(1000))));
    }

    #[test]
    fn bulkstring_ok() {
        let res = parse_bulk_string("$4\r\nPING\r\n");
        assert_eq!(res, Ok(("", RespData::BulkString("PING".to_string()))));
    }

    #[test]
    fn bulkbytes_ok_and_remain_input() {
        let res = parse_bulk_bytes(b"$4\r\nPING123");
        assert_eq!(res, Ok((&b"123"[..], RespData::BulkStringNull)));
    }

    #[test]
    fn array_has_one_element() {
        let res = parse_array("*1\r\n$4\r\nPING\r\n");
        assert_eq!(
            res,
            Ok((
                "",
                RespData::Array(vec![RespData::BulkString("PING".to_string())])
            ))
        );
    }

    #[test]
    fn array_has_two_element() {
        let res = parse_array("*2\r\n$4\r\nECHO\r\n$5\r\nHello\r\n");
        assert_eq!(
            res,
            Ok((
                "",
                RespData::Array(vec![
                    RespData::BulkString("ECHO".to_string()),
                    RespData::BulkString("Hello".to_string())
                ])
            ))
        );
    }

    #[test]
    fn array_has_multiple_type_element() {
        let res = parse_array("*3\r\n$4\r\nECHO\r\n:1000\r\n$5\r\nHello\r\n");
        assert_eq!(
            res,
            Ok((
                "",
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
        let res = parse_array("*3\r\n$4\r\nECHO\r\n:1000\r\n$5\r\nHello\r\n*3\r\n$4\r\nECHO\r\n:1000\r\n$5\r\nHello\r\n");
        assert_eq!(
            res,
            Ok((
                "*3\r\n$4\r\nECHO\r\n:1000\r\n$5\r\nHello\r\n",
                RespData::Array(vec![
                    RespData::BulkString("ECHO".to_string()),
                    RespData::Integer(1000),
                    RespData::BulkString("Hello".to_string()),
                ])
            ))
        );
    }
}
