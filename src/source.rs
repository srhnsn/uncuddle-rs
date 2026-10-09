//! Recognize opt-out markers only in actual leading comments, never in string
//! literals, macro tokens, or comments embedded later in a function.
pub(crate) fn skip_reason(source: &str) -> Option<&'static str> {
    let bytes = source.as_bytes();
    let mut index = usize::from(source.starts_with('\u{feff}')) * 3;

    if source[index..].starts_with("#!") && !source[index..].starts_with("#![") {
        index = source[index..]
            .find('\n')
            .map_or(source.len(), |n| index + n + 1);
    }

    while index < source.len() {
        if bytes[index].is_ascii_whitespace() {
            index += 1;
            continue;
        }

        let begin = index;

        if bytes[index..].starts_with(b"//") {
            index = source[index..]
                .find('\n')
                .map_or(source.len(), |n| index + n);

            if source[begin..index].trim() == "// uncuddle:skip-file" {
                return Some("uncuddle:skip-file");
            }
        } else if bytes[index..].starts_with(b"/*") {
            index += 2;

            let mut depth = 1;

            while index < source.len() && depth > 0 {
                if bytes[index..].starts_with(b"/*") {
                    depth += 1;
                    index += 2;
                } else if bytes[index..].starts_with(b"*/") {
                    depth -= 1;
                    index += 2;
                } else {
                    index += 1;
                }
            }

            if depth != 0 {
                return None;
            }
        } else {
            break;
        }

        if source[..begin].bytes().filter(|b| *b == b'\n').count() < 10
            && source[begin..index].contains("@generated")
        {
            return Some("@generated");
        }
    }

    None
}
