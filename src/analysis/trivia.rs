/// Classify only the trivia between two syntax nodes. Blank lines inside
/// block comments do not count; trailing inline comments stay with the left
/// statement and standalone comments stay attached to the right statement.
pub(super) struct Gap {
    pub(super) has_blank: bool,
    pub(super) insert_at: Option<usize>,
}

impl Gap {
    pub(super) fn scan(source: &str, end: usize, start: usize) -> Option<Self> {
        let bytes = source.as_bytes();
        let mut comments = Vec::new();
        let mut index = end;

        while index < start {
            if bytes[index].is_ascii_whitespace() {
                index += 1;
            } else if bytes[index..start].starts_with(b"//") {
                let begin = index;

                while index < start && bytes[index] != b'\n' {
                    index += 1;
                }

                comments.push(begin..index);
            } else if bytes[index..start].starts_with(b"/*") {
                let begin = index;

                index += 2;

                let mut depth = 1;

                while index < start && depth > 0 {
                    if bytes[index..start].starts_with(b"/*") {
                        depth += 1;
                        index += 2;
                    } else if bytes[index..start].starts_with(b"*/") {
                        depth -= 1;
                        index += 2;
                    } else {
                        index += 1;
                    }
                }

                if depth != 0 {
                    return None;
                }

                comments.push(begin..index);
            } else {
                return None;
            }
        }

        let previous_line_start = source[..end].rfind('\n').map_or(0, |n| n + 1);
        let next_line_start = source[..start].rfind('\n').map_or(0, |n| n + 1);

        if previous_line_start == next_line_start {
            return Some(Self {
                has_blank: false,
                insert_at: None,
            });
        }

        let mut insert_at = source[end..].find('\n').map(|n| end + n + 1)?;

        // A multiline comment starting on the previous line belongs to it.
        for comment in &comments {
            if comment.start < insert_at && comment.end >= insert_at {
                insert_at = source[comment.end..]
                    .find('\n')
                    .map(|n| comment.end + n + 1)?;
            }
        }

        let mut line = source[end..].find('\n').map(|n| end + n + 1)?;

        while line < next_line_start {
            let line_end = source[line..].find('\n').map_or(source.len(), |n| line + n);

            if source[line..line_end].trim().is_empty()
                && !comments
                    .iter()
                    .any(|range| range.start <= line && line < range.end)
            {
                return Some(Self {
                    has_blank: true,
                    insert_at: None,
                });
            }

            line = line_end + 1;
        }

        Some(Self {
            has_blank: false,
            insert_at: (insert_at <= next_line_start).then_some(insert_at),
        })
    }
}
