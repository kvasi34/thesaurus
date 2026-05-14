use tokio::io::{AsyncBufRead, AsyncBufReadExt, AsyncRead, AsyncReadExt};

use crate::errors::RespError;

/// Represents the five data types defined by the RESP2 protocol.
///
/// Each variant maps directly to a wire-format prefix:
/// `+` SimpleString, `-` SimpleError, `:` Integer, `$` BulkString, `*` Array.
#[derive(Debug, Clone, PartialEq)]
pub enum RespValue {
    /// `+OK\r\n` — a short, non-binary-safe string, typically used for server replies.
    SimpleString(String),
    /// `-ERR message\r\n` — an error message returned by the server.
    SimpleError(String),
    /// `:42\r\n` — a signed 64-bit integer.
    Integer(i64),
    /// `$6\r\nfoobar\r\n` — a binary-safe string. `None` represents the null bulk string (`$-1\r\n`).
    BulkString(Option<String>),
    /// `*3\r\n:1\r\n:2\r\n:3\r\n` — an ordered list of `RespValue` elements. `None` represents an empty array (`*0\r\n`).
    Array(Option<Vec<RespValue>>),
}

/// Decodes one RESP2 message from `reader` and returns the parsed [`RespValue`].
///
/// Reads exactly one message per call — the caller is responsible for calling
/// this in a loop to process multiple commands from the same connection.
///
/// See the [RESP2 spec](https://redis.io/docs/latest/develop/reference/protocol-spec/).
pub async fn decode<R>(reader: &mut R) -> Result<RespValue, RespError>
where
    R: AsyncBufRead + AsyncRead + Unpin,
{
    let mut line = String::new();
    match reader.read_line(&mut line).await {
        Ok(0) | Err(_) => return Err(RespError::UnexpectedEof),
        Ok(_) => {}
    }

    let bytes = line.as_bytes();
    let first_byte = bytes.first().ok_or(RespError::UnexpectedEof)?;
    let rest = line[1..].trim_end_matches("\r\n").to_string();

    // Use the first byte to identify the RESP data type and parse accordingly
    match first_byte {
        b'+' => Ok(RespValue::SimpleString(rest)),
        b'-' => Ok(RespValue::SimpleError(rest)),
        b':' => rest
            .parse::<i64>()
            .map(RespValue::Integer)
            .map_err(|_| RespError::InvalidInteger(rest)),
        b'$' => parse_bulk_string(rest, reader).await,
        b'*' => parse_array(rest, reader).await,
        _ => Err(RespError::UnknownPrefix(*first_byte as char)),
    }
}

/*
Parses a RESP2 bulk string. The `length` parameter is the string value extracted from the first part of the bulk string,
indicating how many bytes are in the string.

The `reader` parameter is needed to retrieve the second part of the bulk string, which is the actual string.
*/
async fn parse_bulk_string<R>(length: String, reader: &mut R) -> Result<RespValue, RespError>
where
    R: AsyncBufRead + AsyncRead + Unpin,
{
    let length_value = parse_length(length)?;

    // Handle null bulk strings
    if length_value == -1 {
        return Ok(RespValue::BulkString(None));
    }

    // Parse the second part of the bulk string, which contains the actual string
    let mut second_part = vec![0u8; length_value as usize + 2]; // +2 for \r\n
    reader
        .read_exact(&mut second_part)
        .await
        .map_err(|_| RespError::UnexpectedEof)?;

    // Strip the trailing \r\n
    let value = String::from_utf8(second_part[..length_value as usize].to_vec())
        .map_err(|_| RespError::InvalidUtf8)?;

    Ok(RespValue::BulkString(Some(value)))
}

/*
Parses a RESP2 array. The `length` parameter is the string value extracted from the first part of the array,
indicating how many elements are in the array.

The `reader` parameter is needed to retrieve the rest of the elements of the array.
*/
async fn parse_array<R>(length: String, reader: &mut R) -> Result<RespValue, RespError>
where
    R: AsyncBufRead + AsyncRead + Unpin,
{
    let length_value = parse_length(length)?;

    // Return empty arrays as None
    if length_value == 0 {
        return Ok(RespValue::Array(None));
    }

    // Recursively parse the rest of the stream
    let mut resp_values_array = Vec::<RespValue>::new();
    for _ in 0..length_value {
        // Box::pin is needed here because recursion in an async function requires boxing
        resp_values_array.push(Box::pin(decode(reader)).await?);
    }

    Ok(RespValue::Array(Some(resp_values_array)))
}

// Parses and validates the length value that is part of some RESP2 data types
fn parse_length(length: String) -> Result<i64, RespError> {
    let length_value = length
        .parse::<i64>()
        .map_err(|_| RespError::InvalidInteger(length))?;

    // Reject any other negative length
    if length_value < -1 {
        return Err(RespError::InvalidInteger(length_value.to_string()));
    }

    Ok(length_value)
}

#[cfg(test)]
mod tests {
    use std::io::{BufRead, Cursor};

    use super::*;

    #[tokio::test]
    async fn test_simple_string() {
        let mut cursor = Cursor::new(b"+OK\r\n");
        let result = decode(&mut cursor).await;
        assert!(result.is_ok());
        assert_eq!(result.unwrap(), RespValue::SimpleString("OK".to_string()));
    }

    #[tokio::test]
    async fn test_simple_error() {
        let mut cursor = Cursor::new(b"-Error message\r\n");
        let result = decode(&mut cursor).await;
        assert!(result.is_ok());
        assert_eq!(
            result.unwrap(),
            RespValue::SimpleError("Error message".to_string())
        );
    }

    #[tokio::test]
    async fn test_integer() {
        let mut cursor = Cursor::new(b":56\r\n");
        let result = decode(&mut cursor).await;
        assert!(result.is_ok());
        assert_eq!(result.unwrap(), RespValue::Integer(56));
    }

    #[tokio::test]
    async fn test_negative_integer() {
        let mut cursor = Cursor::new(b":-1\r\n");
        let result = decode(&mut cursor).await;
        assert!(result.is_ok());
        assert_eq!(result.unwrap(), RespValue::Integer(-1));
    }

    #[tokio::test]
    async fn test_invalid_integer() {
        let mut cursor = Cursor::new(b":foo\r\n");
        let result = decode(&mut cursor).await;
        assert!(result.is_err());
        assert_eq!(
            result.err().unwrap(),
            RespError::InvalidInteger("foo".to_string())
        );
    }

    #[tokio::test]
    async fn test_bulk_string() {
        let mut cursor = Cursor::new(b"$5\r\nhello\r\n");
        let result = decode(&mut cursor).await;
        assert!(result.is_ok());
        assert_eq!(
            result.unwrap(),
            RespValue::BulkString(Some("hello".to_string()))
        );
    }

    #[tokio::test]
    async fn test_null_bulk_string() {
        let mut cursor = Cursor::new(b"$-1\r\n");
        let result = decode(&mut cursor).await;
        assert!(result.is_ok());
        assert_eq!(result.unwrap(), RespValue::BulkString(None));
    }

    #[tokio::test]
    async fn test_empty_bulk_string() {
        let mut cursor = Cursor::new(b"$0\r\n\r\n");
        let result = decode(&mut cursor).await;
        assert!(result.is_ok());
        assert_eq!(result.unwrap(), RespValue::BulkString(Some("".to_string())));
    }

    #[tokio::test]
    async fn test_invalid_utf8() {
        let mut cursor = Cursor::new(b"$3\r\n\xFF\xFE\xFD\r\n");
        let result = decode(&mut cursor).await;
        assert!(result.is_err());
        assert_eq!(result.err().unwrap(), RespError::InvalidUtf8);
    }

    #[tokio::test]
    async fn test_array() {
        let mut cursor = Cursor::new(b"*2\r\n$5\r\nhello\r\n$5\r\nworld\r\n");
        let result = decode(&mut cursor).await;
        assert!(result.is_ok());
        assert_eq!(
            result.unwrap(),
            RespValue::Array(Some(vec![
                RespValue::BulkString(Some("hello".to_string())),
                RespValue::BulkString(Some("world".to_string())),
            ]))
        );
    }

    #[tokio::test]
    async fn test_empty_array() {
        let mut cursor = Cursor::new(b"*0\r\n");
        let result = decode(&mut cursor).await;
        assert!(result.is_ok());
        assert_eq!(result.unwrap(), RespValue::Array(None));
    }

    #[tokio::test]
    async fn test_nested_array() {
        /*
        *2\r\n              outer array of 2 elements
          *2\r\n            first element: inner array of 2 elements
            $3\r\nfoo\r\n
            $3\r\nbar\r\n
          *2\r\n            second element: inner array of 2 elements
            $3\r\nbaz\r\n
            $3\r\nqux\r\n
        */
        let mut cursor =
            Cursor::new(b"*2\r\n*2\r\n$3\r\nfoo\r\n$3\r\nbar\r\n*2\r\n$3\r\nbaz\r\n$3\r\nqux\r\n");
        let result = decode(&mut cursor).await;
        assert!(result.is_ok());
        assert_eq!(
            result.unwrap(),
            RespValue::Array(Some(vec![
                RespValue::Array(Some(vec![
                    RespValue::BulkString(Some("foo".to_string())),
                    RespValue::BulkString(Some("bar".to_string())),
                ])),
                RespValue::Array(Some(vec![
                    RespValue::BulkString(Some("baz".to_string())),
                    RespValue::BulkString(Some("qux".to_string())),
                ])),
            ]))
        );
    }

    #[tokio::test]
    async fn test_different_types_array() {
        /*
        *5\r\n
        :1\r\n
        :2\r\n
        :3\r\n
        :4\r\n
        $5\r\nhello\r\n
        */
        let mut cursor = Cursor::new(b"*5\r\n:1\r\n:2\r\n:3\r\n:4\r\n$5\r\nhello\r\n");
        let result = decode(&mut cursor).await;
        assert!(result.is_ok());
        assert_eq!(
            result.unwrap(),
            RespValue::Array(Some(vec![
                RespValue::Integer(1),
                RespValue::Integer(2),
                RespValue::Integer(3),
                RespValue::Integer(4),
                RespValue::BulkString(Some("hello".to_string())),
            ]))
        );
    }

    #[tokio::test]
    async fn test_negative_length() {
        let mut cursor = Cursor::new(b"$-5\r\nhello\r\n");
        let result = decode(&mut cursor).await;
        assert!(result.is_err());
        assert_eq!(
            result.err().unwrap(),
            RespError::InvalidInteger("-5".to_string())
        );
    }

    #[tokio::test]
    async fn test_unknown_prefix_error() {
        let mut cursor = Cursor::new(b"\r\n");
        let result = decode(&mut cursor).await;
        assert!(result.is_err());
        assert_eq!(result.err().unwrap(), RespError::UnknownPrefix('\r'));
    }

    #[tokio::test]
    async fn test_unexpected_eof() {
        // Bulk string promises 5 bytes but stream ends after 3
        let mut cursor = Cursor::new(b"$5\r\nhel");
        let result = decode(&mut cursor).await;
        assert!(result.is_err());
        assert_eq!(result.err().unwrap(), RespError::UnexpectedEof);
    }

    #[tokio::test]
    async fn test_invalid_array_length() {
        let mut cursor = Cursor::new(b"*foo\r\n");
        let result = decode(&mut cursor).await;
        assert!(result.is_err());
        assert_eq!(
            result.err().unwrap(),
            RespError::InvalidInteger("foo".to_string())
        );
    }

    // Test checking that the stream is empty after decoding everything
    #[tokio::test]
    async fn test_stream_empty_after_decode() {
        // +OK\r\n -ERR\r\n  :-1\r\n  $5\r\nhello\r\n  *2\r\n:1\r\n:2\r\n
        let mut cursor = Cursor::new(b"+OK\r\n-ERR\r\n:-1\r\n$5\r\nhello\r\n*2\r\n:1\r\n:2\r\n");
        decode(&mut cursor).await.unwrap();
        decode(&mut cursor).await.unwrap();
        decode(&mut cursor).await.unwrap();
        decode(&mut cursor).await.unwrap();
        decode(&mut cursor).await.unwrap();

        // Check that the stream is empty
        assert!(BufRead::fill_buf(&mut cursor).unwrap().is_empty());
    }
}
