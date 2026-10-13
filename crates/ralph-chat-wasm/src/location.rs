//! Query-string helper for `?daemon=`.
//!
//! The page uses the same origin when the mock serves the built files, and
//! this parameter when `trunk serve` is on another port.

/// Read one query parameter. `search` may include the leading `?`.
pub fn query_param(search: &str, key: &str) -> Option<String> {
    let search = search.trim_start_matches('?');
    if search.is_empty() {
        return None;
    }
    for pair in search.split('&') {
        let Some((name, value)) = pair.split_once('=') else {
            continue;
        };
        if name == key {
            return Some(percent_decode(value));
        }
    }
    None
}

fn percent_decode(input: &str) -> String {
    let bytes = input.as_bytes();
    let mut out = Vec::with_capacity(bytes.len());
    let mut index = 0;
    while index < bytes.len() {
        if bytes[index] == b'+' {
            out.push(b' ');
            index += 1;
        } else if bytes[index] == b'%' && index + 2 < bytes.len() {
            let hex = &input[index + 1..index + 3];
            if let Ok(byte) = u8::from_str_radix(hex, 16) {
                out.push(byte);
                index += 3;
            } else {
                out.push(bytes[index]);
                index += 1;
            }
        } else {
            out.push(bytes[index]);
            index += 1;
        }
    }
    String::from_utf8_lossy(&out).into_owned()
}

#[cfg(test)]
mod tests {
    use super::query_param;

    #[test]
    fn reads_a_daemon_url() {
        let value = query_param("?daemon=http%3A%2F%2F127.0.0.1%3A8788&x=1", "daemon");
        assert_eq!(value.as_deref(), Some("http://127.0.0.1:8788"));
    }

    #[test]
    fn missing_key_is_none() {
        assert_eq!(query_param("", "daemon"), None);
        assert_eq!(query_param("?other=1", "daemon"), None);
    }
}
