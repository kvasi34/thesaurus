use std::io::{BufRead, Read, Write};
use std::time::{SystemTime, UNIX_EPOCH};

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
    /// `*3\r\n:1\r\n:2\r\n:3\r\n` — an ordered list of `RespValue` elements. `None` represents the null array (`*-1\r\n`);
    /// `Some(vec![])` represents the empty array (`*0\r\n`).
    Array(Option<Vec<RespValue>>),
}

/// Decodes one RESP2 message from an asynchronous `reader` and returns the parsed [`RespValue`].
///
/// Reads exactly one message per call — the caller is responsible for calling
/// this in a loop to process multiple commands from the same connection.
///
/// See the [RESP2 spec](https://redis.io/docs/latest/develop/reference/protocol-spec/).
pub async fn decode_async<R>(reader: &mut R) -> Result<RespValue, RespError>
where
    R: AsyncBufRead + AsyncRead + Unpin,
{
    let mut line = String::new();
    match reader.read_line(&mut line).await {
        Ok(0) | Err(_) => return Err(RespError::UnexpectedEof),
        Ok(_) => {}
    }

    let (first_byte, rest) = parse_header(&line)?;

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
        _ => Err(RespError::UnknownPrefix(first_byte as char)),
    }
}

/// Decodes one RESP2 message from a synchronous `reader` and returns the parsed [`RespValue`].
///
/// Intended for startup AOF replay where async I/O is not needed.
/// Reads exactly one message per call — the caller is responsible for calling
/// this in a loop to process all commands.
///
/// See the [RESP2 spec](https://redis.io/docs/latest/develop/reference/protocol-spec/).
pub fn decode<R>(reader: &mut R) -> Result<RespValue, RespError>
where
    R: BufRead + Read,
{
    let mut line = String::new();
    match reader.read_line(&mut line) {
        Ok(0) | Err(_) => return Err(RespError::UnexpectedEof),
        Ok(_) => {}
    }

    let (first_byte, rest) = parse_header(&line)?;

    // Use the first byte to identify the RESP data type and parse accordingly
    match first_byte {
        b'+' => Ok(RespValue::SimpleString(rest)),
        b'-' => Ok(RespValue::SimpleError(rest)),
        b':' => rest
            .parse::<i64>()
            .map(RespValue::Integer)
            .map_err(|_| RespError::InvalidInteger(rest)),
        b'$' => parse_bulk_string_sync(rest, reader),
        b'*' => parse_array_sync(rest, reader),
        _ => Err(RespError::UnknownPrefix(first_byte as char)),
    }
}

/// Encodes a [`RespValue`] into its RESP2 wire-format bytes.
///
/// The returned `Vec<u8>` is ready to be written directly to a TCP stream.
///
/// See the [RESP2 spec](https://redis.io/docs/latest/develop/reference/protocol-spec/).
pub fn encode(resp_value: &RespValue) -> Vec<u8> {
    let mut buffer: Vec<u8> = Vec::new();
    match resp_value {
        RespValue::SimpleString(s) => write!(buffer, "+{}\r\n", s).unwrap(),
        RespValue::SimpleError(s) => write!(buffer, "-{}\r\n", s).unwrap(),
        RespValue::Integer(n) => write!(buffer, ":{}\r\n", n).unwrap(),
        RespValue::BulkString(s) => encode_bulk_string(s, &mut buffer),
        RespValue::Array(arr) => encode_array(arr, &mut buffer),
    }

    buffer
}

/// Converts EXPIRE (relative seconds) to PEXPIREAT (absolute Unix ms).
pub fn convert_expire_to_pexpireat(key: String, seconds: u64) -> Vec<u8> {
    let deadline_ms = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_millis() as u64
        + seconds.saturating_mul(1000);
    encode(&RespValue::Array(Some(vec![
        RespValue::BulkString(Some("PEXPIREAT".to_string())),
        RespValue::BulkString(Some(key)),
        RespValue::BulkString(Some(deadline_ms.to_string())),
    ])))
}

/// Converts PEXPIRE (relative milliseconds) to PEXPIREAT (absolute Unix ms).
pub fn convert_pexpire_to_pexpireat(key: String, milliseconds: u64) -> Vec<u8> {
    let deadline_ms = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_millis() as u64
        + milliseconds;
    encode(&RespValue::Array(Some(vec![
        RespValue::BulkString(Some("PEXPIREAT".to_string())),
        RespValue::BulkString(Some(key)),
        RespValue::BulkString(Some(deadline_ms.to_string())),
    ])))
}

// Parses the first line of a RESP2 message into its prefix byte and the rest of the content
fn parse_header(line: &str) -> Result<(u8, String), RespError> {
    let bytes = line.as_bytes();
    let first_byte = *bytes.first().ok_or(RespError::UnexpectedEof)?;
    let rest = line[1..].trim_end_matches("\r\n").to_string();
    Ok((first_byte, rest))
}

// Converts a length and raw bytes (including trailing \r\n) into a BulkString RespValue
fn bulk_string_from_bytes(length_value: i64, bytes: Vec<u8>) -> Result<RespValue, RespError> {
    // Strip the trailing \r\n
    let value = String::from_utf8(bytes[..length_value as usize].to_vec())
        .map_err(|_| RespError::InvalidUtf8)?;
    Ok(RespValue::BulkString(Some(value)))
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

    bulk_string_from_bytes(length_value, second_part)
}

// Sync version of parse_bulk_string
fn parse_bulk_string_sync<R>(length: String, reader: &mut R) -> Result<RespValue, RespError>
where
    R: Read,
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
        .map_err(|_| RespError::UnexpectedEof)?;

    bulk_string_from_bytes(length_value, second_part)
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

    // Handle null arrays
    if length_value == -1 {
        return Ok(RespValue::Array(None));
    }

    // Return empty arrays as Some(vec![])
    if length_value == 0 {
        return Ok(RespValue::Array(Some(Vec::new())));
    }

    // Recursively parse the rest of the stream
    let mut resp_values_array = Vec::<RespValue>::new();
    for _ in 0..length_value {
        // Box::pin is needed here because recursion in an async function requires boxing
        resp_values_array.push(Box::pin(decode_async(reader)).await?);
    }

    Ok(RespValue::Array(Some(resp_values_array)))
}

// Sync version of parse_array
fn parse_array_sync<R>(length: String, reader: &mut R) -> Result<RespValue, RespError>
where
    R: BufRead + Read,
{
    let length_value = parse_length(length)?;

    // Handle null arrays
    if length_value == -1 {
        return Ok(RespValue::Array(None));
    }

    // Return empty arrays as Some(vec![])
    if length_value == 0 {
        return Ok(RespValue::Array(Some(Vec::new())));
    }

    // Recursively parse the rest of the stream
    let mut resp_values_array = Vec::<RespValue>::new();
    for _ in 0..length_value {
        resp_values_array.push(decode(reader)?);
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

// Encodes a RESP bulk string and writes the results to the buffer
fn encode_bulk_string(bulk_string: &Option<String>, buffer: &mut Vec<u8>) {
    match bulk_string {
        Some(s) => write!(buffer, "${}\r\n{}\r\n", s.len(), s).unwrap(),
        None => write!(buffer, "$-1\r\n").unwrap(),
    }
}

// Encodes a RESP array and writes the results to the buffer
fn encode_array(arr: &Option<Vec<RespValue>>, buffer: &mut Vec<u8>) {
    match arr {
        Some(items) => {
            if items.is_empty() {
                write!(buffer, "*0\r\n").unwrap();
                return;
            }

            // Write the array prefix to buffer
            write!(buffer, "*{}\r\n", items.len()).unwrap();

            // Encode and append each item to the buffer
            for item in items.iter() {
                let encoded_item = encode(item);
                buffer.extend_from_slice(&encoded_item);
            }
        }
        None => write!(buffer, "*-1\r\n").unwrap(),
    }
}

#[cfg(test)]
mod tests {
    use std::io::{BufRead, Cursor};

    use super::*;

    /*Test decoding */
    #[tokio::test]
    async fn test_decode_simple_string() {
        let mut cursor = Cursor::new(b"+OK\r\n");
        let result = decode_async(&mut cursor).await;
        assert!(result.is_ok());
        assert_eq!(result.unwrap(), RespValue::SimpleString("OK".to_string()));
    }

    #[tokio::test]
    async fn test_decode_simple_error() {
        let mut cursor = Cursor::new(b"-Error message\r\n");
        let result = decode_async(&mut cursor).await;
        assert!(result.is_ok());
        assert_eq!(
            result.unwrap(),
            RespValue::SimpleError("Error message".to_string())
        );
    }

    #[tokio::test]
    async fn test_decode_integer() {
        let mut cursor = Cursor::new(b":56\r\n");
        let result = decode_async(&mut cursor).await;
        assert!(result.is_ok());
        assert_eq!(result.unwrap(), RespValue::Integer(56));
    }

    #[tokio::test]
    async fn test_decode_negative_integer() {
        let mut cursor = Cursor::new(b":-1\r\n");
        let result = decode_async(&mut cursor).await;
        assert!(result.is_ok());
        assert_eq!(result.unwrap(), RespValue::Integer(-1));
    }

    #[tokio::test]
    async fn test_decode_invalid_integer() {
        let mut cursor = Cursor::new(b":foo\r\n");
        let result = decode_async(&mut cursor).await;
        assert!(result.is_err());
        assert_eq!(
            result.err().unwrap(),
            RespError::InvalidInteger("foo".to_string())
        );
    }

    #[tokio::test]
    async fn test_decode_bulk_string() {
        let mut cursor = Cursor::new(b"$5\r\nhello\r\n");
        let result = decode_async(&mut cursor).await;
        assert!(result.is_ok());
        assert_eq!(
            result.unwrap(),
            RespValue::BulkString(Some("hello".to_string()))
        );
    }

    #[tokio::test]
    async fn test_decode_null_bulk_string() {
        let mut cursor = Cursor::new(b"$-1\r\n");
        let result = decode_async(&mut cursor).await;
        assert!(result.is_ok());
        assert_eq!(result.unwrap(), RespValue::BulkString(None));
    }

    #[tokio::test]
    async fn test_decode_empty_bulk_string() {
        let mut cursor = Cursor::new(b"$0\r\n\r\n");
        let result = decode_async(&mut cursor).await;
        assert!(result.is_ok());
        assert_eq!(result.unwrap(), RespValue::BulkString(Some("".to_string())));
    }

    #[tokio::test]
    async fn test_decode_invalid_utf8() {
        let mut cursor = Cursor::new(b"$3\r\n\xFF\xFE\xFD\r\n");
        let result = decode_async(&mut cursor).await;
        assert!(result.is_err());
        assert_eq!(result.err().unwrap(), RespError::InvalidUtf8);
    }

    #[tokio::test]
    async fn test_decode_array() {
        let mut cursor = Cursor::new(b"*2\r\n$5\r\nhello\r\n$5\r\nworld\r\n");
        let result = decode_async(&mut cursor).await;
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
    async fn test_decode_empty_array() {
        let mut cursor = Cursor::new(b"*0\r\n");
        let result = decode_async(&mut cursor).await;
        assert!(result.is_ok());
        assert_eq!(result.unwrap(), RespValue::Array(Some(Vec::new())));
    }

    #[tokio::test]
    async fn test_decode_null_array() {
        let mut cursor = Cursor::new(b"*-1\r\n");
        let result = decode_async(&mut cursor).await;
        assert!(result.is_ok());
        assert_eq!(result.unwrap(), RespValue::Array(None));
    }

    #[tokio::test]
    async fn test_decode_nested_array() {
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
        let result = decode_async(&mut cursor).await;
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
    async fn test_decode_different_types_array() {
        /*
        *5\r\n
        :1\r\n
        :2\r\n
        :3\r\n
        :4\r\n
        $5\r\nhello\r\n
        */
        let mut cursor = Cursor::new(b"*5\r\n:1\r\n:2\r\n:3\r\n:4\r\n$5\r\nhello\r\n");
        let result = decode_async(&mut cursor).await;
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
    async fn test_decode_negative_length() {
        let mut cursor = Cursor::new(b"$-5\r\nhello\r\n");
        let result = decode_async(&mut cursor).await;
        assert!(result.is_err());
        assert_eq!(
            result.err().unwrap(),
            RespError::InvalidInteger("-5".to_string())
        );
    }

    #[tokio::test]
    async fn test_decode_unknown_prefix_error() {
        let mut cursor = Cursor::new(b"\r\n");
        let result = decode_async(&mut cursor).await;
        assert!(result.is_err());
        assert_eq!(result.err().unwrap(), RespError::UnknownPrefix('\r'));
    }

    #[tokio::test]
    async fn test_decode_unexpected_eof() {
        // Bulk string promises 5 bytes but stream ends after 3
        let mut cursor = Cursor::new(b"$5\r\nhel");
        let result = decode_async(&mut cursor).await;
        assert!(result.is_err());
        assert_eq!(result.err().unwrap(), RespError::UnexpectedEof);
    }

    #[tokio::test]
    async fn test_decode_invalid_array_length() {
        let mut cursor = Cursor::new(b"*foo\r\n");
        let result = decode_async(&mut cursor).await;
        assert!(result.is_err());
        assert_eq!(
            result.err().unwrap(),
            RespError::InvalidInteger("foo".to_string())
        );
    }

    // Test checking that the stream is empty after decoding everything
    #[tokio::test]
    async fn test_decode_stream_empty_after_decode() {
        // +OK\r\n -ERR\r\n  :-1\r\n  $5\r\nhello\r\n  *2\r\n:1\r\n:2\r\n
        let mut cursor = Cursor::new(b"+OK\r\n-ERR\r\n:-1\r\n$5\r\nhello\r\n*2\r\n:1\r\n:2\r\n");
        decode_async(&mut cursor).await.unwrap();
        decode_async(&mut cursor).await.unwrap();
        decode_async(&mut cursor).await.unwrap();
        decode_async(&mut cursor).await.unwrap();
        decode_async(&mut cursor).await.unwrap();

        // Check that the stream is empty
        assert!(BufRead::fill_buf(&mut cursor).unwrap().is_empty());
    }

    /* Test encoding */
    #[test]
    fn test_encode_simple_string() {
        assert_eq!(
            encode(&RespValue::SimpleString("OK".to_string())),
            b"+OK\r\n"
        );
    }

    #[test]
    fn test_encode_simple_error() {
        assert_eq!(
            encode(&RespValue::SimpleError("Error message".to_string())),
            b"-Error message\r\n"
        );
    }

    #[test]
    fn test_encode_integer() {
        assert_eq!(encode(&RespValue::Integer(56)), b":56\r\n");
    }

    #[test]
    fn test_encode_negative_integer() {
        assert_eq!(encode(&RespValue::Integer(-1)), b":-1\r\n");
    }

    #[test]
    fn test_encode_bulk_string() {
        assert_eq!(
            encode(&RespValue::BulkString(Some("hello".to_string()))),
            b"$5\r\nhello\r\n"
        );
    }

    #[test]
    fn test_encode_null_bulk_string() {
        assert_eq!(encode(&RespValue::BulkString(None)), b"$-1\r\n");
    }

    #[test]
    fn test_encode_empty_bulk_string() {
        assert_eq!(
            encode(&RespValue::BulkString(Some("".to_string()))),
            b"$0\r\n\r\n"
        );
    }

    #[test]
    fn test_encode_array() {
        assert_eq!(
            encode(&RespValue::Array(Some(vec![
                RespValue::BulkString(Some("hello".to_string())),
                RespValue::BulkString(Some("world".to_string())),
            ]))),
            b"*2\r\n$5\r\nhello\r\n$5\r\nworld\r\n"
        );
    }

    #[test]
    fn test_encode_empty_array() {
        assert_eq!(encode(&RespValue::Array(Some(Vec::new()))), b"*0\r\n");
    }

    #[test]
    fn test_encode_null_array() {
        assert_eq!(encode(&RespValue::Array(None)), b"*-1\r\n");
    }

    #[test]
    fn test_encode_nested_array() {
        /*
        *2\r\n              outer array of 2 elements
          *2\r\n            first element: inner array of 2 elements
            $3\r\nfoo\r\n
            $3\r\nbar\r\n
          *2\r\n            second element: inner array of 2 elements
            $3\r\nbaz\r\n
            $3\r\nqux\r\n
        */
        assert_eq!(
            encode(&RespValue::Array(Some(vec![
                RespValue::Array(Some(vec![
                    RespValue::BulkString(Some("foo".to_string())),
                    RespValue::BulkString(Some("bar".to_string())),
                ])),
                RespValue::Array(Some(vec![
                    RespValue::BulkString(Some("baz".to_string())),
                    RespValue::BulkString(Some("qux".to_string())),
                ])),
            ]))),
            b"*2\r\n*2\r\n$3\r\nfoo\r\n$3\r\nbar\r\n*2\r\n$3\r\nbaz\r\n$3\r\nqux\r\n"
        );
    }

    #[test]
    fn test_encode_different_types_array() {
        /*
        *5\r\n
        :1\r\n
        :2\r\n
        :3\r\n
        :4\r\n
        $5\r\nhello\r\n
        */
        assert_eq!(
            encode(&RespValue::Array(Some(vec![
                RespValue::Integer(1),
                RespValue::Integer(2),
                RespValue::Integer(3),
                RespValue::Integer(4),
                RespValue::BulkString(Some("hello".to_string())),
            ]))),
            b"*5\r\n:1\r\n:2\r\n:3\r\n:4\r\n$5\r\nhello\r\n"
        );
    }

    #[test]
    fn test_convert_expire_to_pexpireat() {
        let before_ms = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_millis() as u64;
        let bytes = convert_expire_to_pexpireat("mykey".to_string(), 10);
        let after_ms = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_millis() as u64;

        let mut cursor = Cursor::new(bytes);
        let RespValue::Array(Some(arr)) = decode(&mut cursor).unwrap() else {
            panic!("expected array");
        };
        assert_eq!(arr[0], RespValue::BulkString(Some("PEXPIREAT".to_string())));
        assert_eq!(arr[1], RespValue::BulkString(Some("mykey".to_string())));
        let RespValue::BulkString(Some(deadline_str)) = &arr[2] else {
            panic!("expected bulk string");
        };
        let deadline_ms: u64 = deadline_str.parse().unwrap();
        assert!(deadline_ms >= before_ms + 10_000);
        assert!(deadline_ms <= after_ms + 10_000);
    }

    #[test]
    fn test_convert_pexpire_to_pexpireat() {
        let before_ms = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_millis() as u64;
        let bytes = convert_pexpire_to_pexpireat("mykey".to_string(), 5000);
        let after_ms = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_millis() as u64;

        let mut cursor = Cursor::new(bytes);
        let RespValue::Array(Some(arr)) = decode(&mut cursor).unwrap() else {
            panic!("expected array");
        };
        assert_eq!(arr[0], RespValue::BulkString(Some("PEXPIREAT".to_string())));
        assert_eq!(arr[1], RespValue::BulkString(Some("mykey".to_string())));
        let RespValue::BulkString(Some(deadline_str)) = &arr[2] else {
            panic!("expected bulk string");
        };
        let deadline_ms: u64 = deadline_str.parse().unwrap();
        assert!(deadline_ms >= before_ms + 5_000);
        assert!(deadline_ms <= after_ms + 5_000);
    }
}
