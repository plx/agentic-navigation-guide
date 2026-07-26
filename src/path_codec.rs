use std::ffi::OsStr;
use std::fmt::Write;

pub(crate) fn has_windows_drive_prefix(component: &str) -> bool {
    let bytes = component.as_bytes();
    bytes.len() >= 2 && bytes[0].is_ascii_alphabetic() && bytes[1] == b':'
}

pub(crate) fn contains_forbidden_control(value: &str) -> bool {
    value.chars().any(|ch| ch <= '\u{1f}' || ch == '\u{7f}')
}

pub(crate) fn serialize_component(value: &str) -> Option<String> {
    if value.is_empty() || contains_forbidden_control(value) {
        return None;
    }

    let requires_quotes = value == "..."
        || value.starts_with(' ')
        || value.ends_with(' ')
        || value
            .chars()
            .any(|ch| matches!(ch, '#' | '\\' | '[' | ']' | ',' | '"'));
    if !requires_quotes {
        return Some(value.to_string());
    }

    let mut serialized = String::with_capacity(value.len() + 2);
    serialized.push('"');
    for ch in value.chars() {
        if matches!(ch, '"' | '\\') {
            serialized.push('\\');
        }
        serialized.push(ch);
    }
    serialized.push('"');
    Some(serialized)
}

pub(crate) fn render_utf8_component(value: &str) -> String {
    let mut rendered = String::with_capacity(value.len() + 2);
    rendered.push('"');
    for ch in value.chars() {
        match ch {
            '"' => rendered.push_str("\\\""),
            '\\' => rendered.push_str("\\\\"),
            '\0' => rendered.push_str("\\0"),
            '\t' => rendered.push_str("\\t"),
            '\n' => rendered.push_str("\\n"),
            '\r' => rendered.push_str("\\r"),
            '\u{1}'..='\u{1f}' | '\u{7f}' => {
                write!(rendered, "\\u{{{:04X}}}", ch as u32)
                    .expect("writing into a String cannot fail");
            }
            _ => rendered.push(ch),
        }
    }
    rendered.push('"');
    rendered
}

pub(crate) fn render_os_component(value: &OsStr) -> String {
    if let Some(value) = value.to_str() {
        return render_utf8_component(value);
    }

    #[cfg(unix)]
    {
        use std::os::unix::ffi::OsStrExt;

        let mut rendered = String::with_capacity(value.as_bytes().len() * 4 + 2);
        rendered.push('"');
        for byte in value.as_bytes() {
            write!(rendered, "\\x{byte:02X}").expect("writing into a String cannot fail");
        }
        rendered.push('"');
        rendered
    }

    #[cfg(windows)]
    {
        use std::os::windows::ffi::OsStrExt;

        let mut rendered = String::new();
        rendered.push('"');
        for unit in value.encode_wide() {
            write!(rendered, "\\u{{{unit:04X}}}").expect("writing into a String cannot fail");
        }
        rendered.push('"');
        rendered
    }

    #[cfg(not(any(unix, windows)))]
    {
        compile_error!("exact non-UTF-8 name rendering requires a Unix or Windows target");
    }
}
