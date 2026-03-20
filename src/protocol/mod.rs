use anyhow::{Result, anyhow};

pub fn parse_command(buffer: &[u8]) -> Result<(Vec<String>, usize)> {
    if buffer.is_empty() {
        return Err(anyhow!("Empty buffer"));
    }

    if buffer[0] != b'*' {
        return Err(anyhow!("Expected array start '*'"));
    }

    let mut pos = 1;
    let (num_elements, consumed) = read_line(&buffer[pos..])?;
    let num_elements: usize = num_elements.parse()?;
    pos += consumed;

    let mut args = Vec::with_capacity(num_elements);
    for _ in 0..num_elements {
        if pos >= buffer.len() || buffer[pos] != b'$' {
            return Err(anyhow!("Expected bulk string start '$'"));
        }
        pos += 1;
        let (len_str, consumed) = read_line(&buffer[pos..])?;
        let len: usize = len_str.parse()?;
        pos += consumed;

        if pos + len + 2 > buffer.len() {
            return Err(anyhow!("Buffer too short for bulk string content"));
        }
        let data = &buffer[pos..pos + len];
        args.push(String::from_utf8_lossy(data).to_string());
        pos += len + 2; // +2 for \r\n
    }

    Ok((args, pos))
}

fn read_line(data: &[u8]) -> Result<(String, usize)> {
    for i in 0..data.len() - 1 {
        if data[i] == b'\r' && data[i + 1] == b'\n' {
            return Ok((String::from_utf8_lossy(&data[..i]).to_string(), i + 2));
        }
    }
    Err(anyhow!("No CRLF found"))
}

pub struct RediskProtocol {
    ok: String,
    resp_version: u8,
}

pub fn new_redisk_protocol(resp_version: u8) -> RediskProtocol {
    RediskProtocol {
        ok: String::from("OK"),
        resp_version
    }
}

impl RediskProtocol {
    pub fn serialize_simple_string(&self, str: &String) -> Vec<u8> {
        format!("+{}\r\n", str).into_bytes()
    }

    pub fn serialize_ok(&self) -> Vec<u8> {
        self.serialize_simple_string(&self.ok)
    }

    pub fn serialize_null(&self) -> Vec<u8> {
        match self.resp_version {
            3 => b"_\r\n".to_vec(),
            _ => b"$-1\r\n".to_vec(),
        }
    }

    pub fn serialize_error(&self, msg: &str) -> Vec<u8> {
        format!("-ERR {}\r\n", msg).into_bytes()
    }

    pub fn serialize_bulk_string(&self, s: &str) -> Vec<u8> {
        format!("${}\r\n{}\r\n", s.len(), s).into_bytes()
    }

    pub fn serialize_integer(&self, i: i64) -> Vec<u8> {
        format!(":{}\r\n", i).into_bytes()
    }
}


#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_set() {
        let buffer = b"*3\r\n$3\r\nSET\r\n$4\r\nkey1\r\n$4\r\nval1\r\n";
        let (args, consumed) = parse_command(buffer).unwrap();
        assert_eq!(args, vec!["SET", "key1", "val1"]);
        assert_eq!(consumed, buffer.len());
    }

    #[test]
    fn test_parse_get() {
        let buffer = b"*2\r\n$3\r\nGET\r\n$4\r\nkey1\r\n";
        let (args, consumed) = parse_command(buffer).unwrap();
        assert_eq!(args, vec!["GET", "key1"]);
        assert_eq!(consumed, buffer.len());
    }
}
