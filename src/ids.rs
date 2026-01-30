use serde_json::{Map, Value};

pub(crate) fn short_id(mut n: usize) -> String {
    let mut chars = Vec::new();
    loop {
        let rem = n % 26;
        chars.push((b'A' + rem as u8) as char);
        n /= 26;
        if n == 0 {
            break;
        }
        n -= 1;
    }
    chars.iter().rev().collect()
}

pub(crate) fn short_for_id(id_map: &Map<String, Value>, full_id: &str) -> Option<String> {
    for (short, full) in id_map.iter() {
        if full.as_str() == Some(full_id) {
            return Some(short.clone());
        }
    }
    None
}
