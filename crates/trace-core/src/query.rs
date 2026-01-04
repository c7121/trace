use anyhow::bail;

const MAX_STRING_LITERAL_BYTES: usize = 4096;

/// Conservative denylist of functions that can read from (or otherwise touch) external systems.
///
/// Notes:
/// - We intentionally key off *function call sites* (identifier followed by `(`, allowing
///   whitespace/comments in between). This avoids false positives for column/table names.
/// - This list is necessarily conservative. Query Service MUST also run DuckDB with external
///   access disabled and a locked-down OS/network sandbox.
const FORBIDDEN_FUNCTIONS: &[&str] = &[
    // File / URL readers (table functions)
    "READ_CSV",
    "READ_CSV_AUTO",
    "READ_PARQUET",
    "PARQUET_SCAN",
    "READ_JSON",
    "READ_JSON_AUTO",
    "JSON_SCAN",
    "SQLITE_SCAN",
    "POSTGRES_SCAN",
    "MYSQL_SCAN",
    "DELTA_SCAN",
    "ICEBERG_SCAN",
    // File-ish scalar helpers
    "READ_FILE",
    "READ_BLOB",
    "WRITE_FILE",
    "WRITE_BLOB",
    // Environment/process introspection (defense-in-depth)
    "GETENV",
];

const FORBIDDEN_KEYWORDS: &[&str] = &[
    "ATTACH", "DETACH", "INSTALL", "LOAD", // DDL/DML/side effects
    "ALTER", "ANALYZE", "CALL", "COPY", "CREATE", "DELETE", "DROP", "EXEC", "EXECUTE", "EXPORT",
    "IMPORT", "INSERT", "MERGE", "PRAGMA", "RESET", "SET", "UPDATE", "VACUUM",
    // Some engines support SELECT ... INTO (file/table). We treat INTO as a side effect.
    "INTO",
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
/// - Reject known unsafe external-reader functions (e.g. `read_csv(...)`)
/// - Reject non-standard string-literal relations (e.g. `FROM 'file.csv'`)
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

    // Minimal context to reject DuckDB-style FROM 'file.csv' and similar.
    // We track whether we're in the FROM clause at the top level and whether
    // the next token must be a relation/table factor.
    let mut paren_depth: i32 = 0;
    let mut in_from_clause = false;
    let mut expects_relation = false;

    let mut string_literal = String::new();
    let mut quoted_ident = String::new();

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
                if b == b'(' {
                    paren_depth += 1;
                    if expects_relation {
                        expects_relation = false;
                    }
                    i += 1;
                    continue;
                }
                if b == b')' {
                    paren_depth = (paren_depth - 1).max(0);
                    i += 1;
                    continue;
                }
                if b == b',' {
                    // Only treat commas as relation separators at top-level of a FROM clause.
                    if in_from_clause && paren_depth == 0 {
                        expects_relation = true;
                    }
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
                    quoted_ident.clear();
                    if expects_relation {
                        expects_relation = false;
                    }
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

                    // Track top-level FROM clause state so we can reject
                    // non-standard `FROM 'file.csv'`.
                    if paren_depth == 0 {
                        match token_upper.as_str() {
                            "FROM" => {
                                in_from_clause = true;
                                expects_relation = true;
                            }
                            "JOIN" => {
                                if in_from_clause {
                                    expects_relation = true;
                                }
                            }
                            // Heuristic: these keywords end the FROM clause in common SQL dialects.
                            "WHERE" | "GROUP" | "HAVING" | "QUALIFY" | "WINDOW" | "ORDER" | "LIMIT" => {
                                in_from_clause = false;
                                expects_relation = false;
                            }
                            _ => {
                                if expects_relation {
                                    expects_relation = false;
                                }
                            }
                        }
                    }

                    if FORBIDDEN_KEYWORDS
                        .iter()
                        .any(|kw| kw.eq_ignore_ascii_case(&token_upper))
                    {
                        bail!("sql rejected: forbidden keyword");
                    }

                    // Deny known unsafe function call sites (e.g. read_csv(...)).
                    if FORBIDDEN_FUNCTIONS
                        .iter()
                        .any(|f| f.eq_ignore_ascii_case(&token_upper))
                        && looks_like_function_call(bytes, i)
                    {
                        bail!("sql rejected: forbidden function");
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

                    if in_from_clause && expects_relation {
                        // Reject non-standard table factor syntax like:
                        //   SELECT * FROM 'file.csv'
                        bail!("sql rejected: string literal relation");
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
                        if quoted_ident.len() < MAX_STRING_LITERAL_BYTES {
                            quoted_ident.push('"');
                        } else {
                            bail!("sql rejected: quoted identifier too long");
                        }
                        i += 2;
                        continue;
                    }

                    // Quoted identifiers can be used as function names in many SQL dialects.
                    // Treat these as potential function call sites.
                    let ident_upper = quoted_ident.to_ascii_uppercase();
                    if FORBIDDEN_FUNCTIONS
                        .iter()
                        .any(|f| f.eq_ignore_ascii_case(&ident_upper))
                        && looks_like_function_call(bytes, i + 1)
                    {
                        bail!("sql rejected: forbidden function");
                    }

                    state = State::Normal;
                    i += 1;
                    continue;
                }

                if quoted_ident.len() < MAX_STRING_LITERAL_BYTES {
                    quoted_ident.push(b as char);
                } else {
                    bail!("sql rejected: quoted identifier too long");
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

fn looks_like_function_call(bytes: &[u8], mut i: usize) -> bool {
    // Skip whitespace and comments to find the next significant token.
    while i < bytes.len() {
        let b = bytes[i];

        if b.is_ascii_whitespace() {
            i += 1;
            continue;
        }
        if b == b'-' && bytes.get(i + 1) == Some(&b'-') {
            // Line comment
            i += 2;
            while i < bytes.len() && bytes[i] != b'\n' {
                i += 1;
            }
            continue;
        }
        if b == b'/' && bytes.get(i + 1) == Some(&b'*') {
            // Block comment
            i += 2;
            while i + 1 < bytes.len() {
                if bytes[i] == b'*' && bytes[i + 1] == b'/' {
                    i += 2;
                    break;
                }
                i += 1;
            }
            continue;
        }

        return b == b'(';
    }

    false
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
        validate_sql("SELECT 'https://example.com/x.parquet'").unwrap();
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
    fn rejects_forbidden_functions() {
        assert_rejected("SELECT * FROM read_csv('data')");
        assert_rejected("SELECT * FROM read_parquet('x')");
        assert_rejected("SELECT * FROM parquet_scan('x')");
        assert_rejected("SELECT * FROM \"read_parquet\"('x')");
        assert_rejected("SELECT getenv('HOME')");
    }

    #[test]
    fn rejects_string_literal_relations() {
        assert_rejected("SELECT * FROM 'local.csv'");
        assert_rejected("SELECT * FROM t, 'local.csv'");
        assert_rejected("SELECT * FROM t JOIN 'local.csv' ON 1=1");
    }
}
