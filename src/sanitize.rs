pub fn truncate_bytes(s: &str, max_bytes: usize) -> String {
    if s.len() <= max_bytes {
        return s.to_string();
    }
    let mut end = max_bytes;
    while end > 0 && !s.is_char_boundary(end) {
        end -= 1;
    }
    if let Some(pos) = s[..end].rfind('\n') {
        end = pos;
    }
    let truncated = s.len() - end;
    format!(
        "{}...\n\n[...truncated {} bytes]",
        &s[..end],
        truncated
    )
}

pub fn tail_lines(s: &str, max_lines: usize) -> String {
    let lines: Vec<&str> = s.lines().collect();
    if lines.len() <= max_lines {
        return s.to_string();
    }
    let skipped = lines.len() - max_lines;
    format!(
        "[...truncated {} lines, showing last {}]\n{}",
        skipped,
        max_lines,
        lines[lines.len() - max_lines..].join("\n")
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn tail_keeps_last_n_lines() {
        let input = "1\n2\n3\n4\n5";
        assert_eq!(
            tail_lines(input, 3),
            "[...truncated 2 lines, showing last 3]\n3\n4\n5"
        );
    }

    #[test]
    fn tail_no_op_when_within_limit() {
        let input = "1\n2\n3";
        assert_eq!(tail_lines(input, 5), "1\n2\n3");
    }
}
