use anyhow::bail;

const MAX_STRING_LITERAL_BYTES: usize = 4096;

const FORBIDDEN_KEYWORDS: &[&str] = &[
    "ATTACH", "DETACH", "INSTALL", "LOAD", // DDL/DML/side effects
    "ALTER", "ANALYZE", "CALL", "COPY", "CREATE", "DELETE", "DROP", "EXEC", "EXECUTE", "EXPORT",
    "IMPORT", "INSERT", "MERGE", "PRAGMA", "RESET", "SET", "UPDATE", "VACUUM",
];

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum State {
    Normal,
    SingleQuote,
    DoubleQuote,
    LineComment,
    BlockComment,
}

/// Fail-closed SQL validator for Query Service.
///
/// v1 requirements:
/// - Allow only a single `SELECT` (or `WITH ... SELECT`) statement
/// - Reject extension install/load
/// - Reject filesystem/URL/URI literals
/// - Reject multi-statement batches
///
/// This validator is intentionally conservative and does not try to be a full SQL parser.
pub fn validate_sql(sql: &str) -> anyhow::Result<()> {
    let sql = sql.trim();
    if sql.is_empty() {
        bail!("sql rejected: empty");
    }

    let bytes = sql.as_bytes();
    let mut i = 0usize;
    let mut state = State::Normal;
    let mut first_keyword: Option<String> = None;
    let mut seen_semicolon = false;
    let mut string_literal = String::new();

    while i < bytes.len() {
        let b = bytes[i];

        match state {
            State::Normal => {
                if seen_semicolon {
                    if b.is_ascii_whitespace() {
                        i += 1;
                        continue;
                    }
                    if b == b'-' && bytes.get(i + 1) == Some(&b'-') {
                        state = State::LineComment;
                        i += 2;
                        continue;
                    }
                    if b == b'/' && bytes.get(i + 1) == Some(&b'*') {
                        state = State::BlockComment;
                        i += 2;
                        continue;
                    }
                    bail!("sql rejected: multiple statements");
                }

                if b.is_ascii_whitespace() {
                    i += 1;
                    continue;
                }
                if b == b'-' && bytes.get(i + 1) == Some(&b'-') {
                    state = State::LineComment;
                    i += 2;
                    continue;
                }
                if b == b'/' && bytes.get(i + 1) == Some(&b'*') {
                    state = State::BlockComment;
                    i += 2;
                    continue;
                }
                if b == b';' {
                    seen_semicolon = true;
                    i += 1;
                    continue;
                }
                if b == b'\'' {
                    state = State::SingleQuote;
                    string_literal.clear();
                    i += 1;
                    continue;
                }
                if b == b'"' {
                    state = State::DoubleQuote;
                    i += 1;
                    continue;
                }

                if b.is_ascii_alphabetic() || b == b'_' {
                    let start = i;
                    i += 1;
                    while i < bytes.len() && (bytes[i].is_ascii_alphanumeric() || bytes[i] == b'_')
                    {
                        i += 1;
                    }

                    let token = match std::str::from_utf8(&bytes[start..i]) {
                        Ok(s) => s,
                        Err(_) => bail!("sql rejected: invalid utf-8"),
                    };
                    let token_upper = token.to_ascii_uppercase();

                    if first_keyword.is_none() {
                        first_keyword = Some(token_upper.clone());
                    }

                    if FORBIDDEN_KEYWORDS
                        .iter()
                        .any(|kw| kw.eq_ignore_ascii_case(&token_upper))
                    {
                        bail!("sql rejected: forbidden keyword");
                    }

                    continue;
                }

                // Other characters are ignored for validation purposes.
                i += 1;
            }
            State::SingleQuote => {
                if b == b'\'' {
                    if bytes.get(i + 1) == Some(&b'\'') {
                        if string_literal.len() < MAX_STRING_LITERAL_BYTES {
                            string_literal.push('\'');
                        } else {
                            bail!("sql rejected: string literal too long");
                        }
                        i += 2;
                        continue;
                    }

                    if literal_looks_like_external_ref(&string_literal) {
                        bail!("sql rejected: forbidden string literal");
                    }

                    state = State::Normal;
                    i += 1;
                    continue;
                }

                if string_literal.len() < MAX_STRING_LITERAL_BYTES {
                    string_literal.push(b as char);
                } else {
                    bail!("sql rejected: string literal too long");
                }
                i += 1;
            }
            State::DoubleQuote => {
                if b == b'"' {
                    if bytes.get(i + 1) == Some(&b'"') {
                        i += 2;
                        continue;
                    }
                    state = State::Normal;
                    i += 1;
                    continue;
                }
                i += 1;
            }
            State::LineComment => {
                if b == b'\n' {
                    state = State::Normal;
                }
                i += 1;
            }
            State::BlockComment => {
                if b == b'*' && bytes.get(i + 1) == Some(&b'/') {
                    state = State::Normal;
                    i += 2;
                    continue;
                }
                i += 1;
            }
        }
    }

    match state {
        State::Normal | State::LineComment => {}
        State::SingleQuote => bail!("sql rejected: unterminated string literal"),
        State::DoubleQuote => bail!("sql rejected: unterminated quoted identifier"),
        State::BlockComment => bail!("sql rejected: unterminated block comment"),
    }

    let first = first_keyword.ok_or_else(|| anyhow::anyhow!("sql rejected: no statement"))?;
    if first != "SELECT" && first != "WITH" {
        bail!("sql rejected: only SELECT allowed");
    }

    Ok(())
}

fn literal_looks_like_external_ref(literal: &str) -> bool {
    let s = literal.trim();
    let lower = s.to_ascii_lowercase();

    if lower.contains("://") {
        return true;
    }
    if lower.starts_with('/') || lower.starts_with('\\') {
        return true;
    }
    if lower.starts_with("./")
        || lower.starts_with("../")
        || lower.starts_with(".\\")
        || lower.starts_with("..\\")
        || lower.starts_with('~')
    {
        return true;
    }
    if lower.contains(":\\") {
        return true;
    }

    const FORBIDDEN_EXTENSIONS: &[&str] = &[
        ".csv", ".db", ".duckdb", ".json", ".jsonl", ".parquet", ".sqlite", ".txt",
    ];
    FORBIDDEN_EXTENSIONS.iter().any(|ext| lower.ends_with(ext))
}

#[cfg(test)]
mod tests {
    use super::validate_sql;

    fn assert_rejected(sql: &str) {
        assert!(validate_sql(sql).is_err(), "expected rejection: {sql}");
    }

    #[test]
    fn allows_single_select() {
        validate_sql("SELECT 1").unwrap();
        validate_sql("SELECT 1;").unwrap();
        validate_sql("SELECT ';'").unwrap();
        validate_sql("SELECT 1; -- trailing comment").unwrap();
        validate_sql("/* leading */ SELECT 1").unwrap();
        validate_sql("WITH t AS (SELECT 1) SELECT * FROM t").unwrap();
    }

    #[test]
    fn rejects_multi_statement() {
        assert_rejected("SELECT 1; SELECT 2");
        assert_rejected("SELECT 1; /* ok */ SELECT 2");
        assert_rejected("SELECT 1;;");
    }

    #[test]
    fn rejects_non_select() {
        assert_rejected("UPDATE t SET x = 1");
        assert_rejected("CREATE TABLE t(x INT)");
        assert_rejected("DELETE FROM t");
    }

    #[test]
    fn rejects_install_load_attach() {
        assert_rejected("INSTALL httpfs");
        assert_rejected("LOAD httpfs");
        assert_rejected("ATTACH 'db.duckdb' AS other");
    }

    #[test]
    fn rejects_file_and_url_literals() {
        assert_rejected("SELECT * FROM read_csv('data.csv')");
        assert_rejected("SELECT * FROM read_parquet('/etc/passwd')");
        assert_rejected("SELECT * FROM read_parquet('https://example.com/x.parquet')");
        assert_rejected("SELECT * FROM read_parquet('s3://bucket/path/x.parquet')");
        assert_rejected("SELECT * FROM 'local.csv'");
    }
}
