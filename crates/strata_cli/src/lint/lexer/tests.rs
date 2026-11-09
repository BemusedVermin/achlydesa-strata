use super::*;

/// `(kind, text, line)` for every token, `Eof` included.
fn lex(source: &str) -> Vec<(Kind, String, usize)> {
    tokenize(source)
        .into_iter()
        .map(|t| (t.kind, t.text, t.line))
        .collect()
}

fn texts(source: &str) -> Vec<String> {
    tokenize(source).into_iter().map(|t| t.text).collect()
}

fn bad(source: &str) -> (String, usize) {
    let last = tokenize(source).pop().unwrap();
    assert_eq!(last.kind, Kind::Bad, "{source:?}");
    (last.text, last.line)
}

#[test]
fn names_keywords_and_longest_symbols() {
    assert_eq!(
        lex("local x_1 = a // b ... .. ~= :: ~"),
        vec![
            (Kind::Symbol, "local".into(), 1),
            (Kind::Name, "x_1".into(), 1),
            (Kind::Symbol, "=".into(), 1),
            (Kind::Name, "a".into(), 1),
            (Kind::Symbol, "//".into(), 1),
            (Kind::Name, "b".into(), 1),
            (Kind::Symbol, "...".into(), 1),
            (Kind::Symbol, "..".into(), 1),
            (Kind::Symbol, "~=".into(), 1),
            (Kind::Symbol, "::".into(), 1),
            (Kind::Symbol, "~".into(), 1),
            (Kind::Eof, String::new(), 1),
        ]
    );
}

#[test]
fn offsets_count_characters() {
    let offsets: Vec<usize> = tokenize("é = 1").iter().map(|t| t.offset).collect();
    assert_eq!(
        offsets,
        vec![0],
        "a non-ASCII character outside a string is unreadable"
    );
    let offsets: Vec<usize> = tokenize("s = 'é' x").iter().map(|t| t.offset).collect();
    assert_eq!(offsets, vec![0, 2, 4, 8, 9]);
}

#[test]
fn long_strings_keep_their_body_and_count_lines() {
    let tokens = lex("x = [==[ a ]] ]=] b\n]==] y");
    assert_eq!(tokens[2], (Kind::Str, " a ]] ]=] b\n".into(), 1));
    assert_eq!(tokens[3], (Kind::Name, "y".into(), 2));
    assert_eq!(lex("[[]]")[0], (Kind::Str, String::new(), 1));
}

#[test]
fn a_bracket_without_a_long_opener_is_a_symbol() {
    assert_eq!(texts("t[=x"), vec!["t", "[", "=", "x", ""]);
    assert_eq!(texts("t[ [=[s]=] ]"), vec!["t", "[", "s", "]", ""]);
}

#[test]
fn long_and_short_comments_are_skipped() {
    let tokens = lex("a --[==[ one ]] two\n]==] b -- rest [[\nc --[ not long\nd");
    let names: Vec<(String, usize)> = tokens.into_iter().map(|t| (t.1, t.2)).collect();
    assert_eq!(
        names,
        vec![
            ("a".into(), 1),
            ("b".into(), 2),
            ("c".into(), 3),
            ("d".into(), 4),
            (String::new(), 4)
        ]
    );
}

#[test]
fn escapes_stay_raw_in_the_string_body() {
    assert_eq!(texts(r#"x = "a\"b" .. '\\' .. 'it\'s'"#)[2], r#"a\"b"#);
    assert_eq!(texts(r#"x = "a\"b" .. '\\' .. 'it\'s'"#)[4], r"\\");
    assert_eq!(texts(r#"x = "a\"b" .. '\\' .. 'it\'s'"#)[6], r"it\'s");
    assert_eq!(texts(r"x = '\0657'")[2], r"\0657");
}

#[test]
fn an_escape_allows_one_line_break() {
    let tokens = lex("x = 'a\\\nb' y = 'c\\z\n   d' z");
    assert_eq!(tokens[2], (Kind::Str, "a\\\nb".into(), 1));
    assert_eq!(tokens[3], (Kind::Name, "y".into(), 2));
    assert_eq!(tokens[6], (Kind::Name, "z".into(), 3));
}

#[test]
fn numbers() {
    let cases = [
        "3", "0x1F", "1.5", ".5", "3.", "1e10", "1E+3", "2e-1", "0x1p4", "0xA.8p-1", "0xe",
    ];
    for case in cases {
        assert_eq!(lex(case)[0], (Kind::Number, case.to_owned(), 1), "{case}");
    }
    let (message, _) = bad("3..x");
    assert!(
        message.starts_with("invalid number"),
        "a second `.` makes the number invalid"
    );
    assert_eq!(texts("12abc"), vec!["12", "abc", ""]);
}

#[test]
fn invalid_numbers_name_their_range() {
    assert_eq!(
        bad("x = 1.2.3\n"),
        ("invalid number (1:5 to 1:10)".into(), 1)
    );
    assert_eq!(bad("x = 0x\n"), ("invalid number (1:5 to 1:7)".into(), 1));
    assert_eq!(bad("x = 3e\n"), ("invalid number (1:5 to 1:7)".into(), 1));
    assert_eq!(bad("x = 0xp1\n"), ("invalid number (1:5 to 1:9)".into(), 1));
}

#[test]
fn unclosed_strings_and_comments() {
    assert_eq!(
        bad("x = \"abc\n"),
        ("unclosed string (1:5 to 2:1)".into(), 1)
    );
    assert_eq!(bad("x = 'abc"), ("unclosed string (1:5 to 1:9)".into(), 1));
    assert_eq!(
        bad("x = [[abc"),
        ("unclosed string (1:5 to 1:10)".into(), 1)
    );
    assert_eq!(
        bad("x = 1 -- comment\ny = [=[\n"),
        ("unclosed string (2:5 to 3:1)".into(), 2)
    );
    assert_eq!(
        bad("--[[ never closed\nx = 1\n"),
        ("unclosed comment (1:1 to 3:1)".into(), 1)
    );
}

#[test]
fn unexpected_characters() {
    assert_eq!(
        bad("x = 1 $\n"),
        ("unexpected character $ (1:7 to 1:8)".into(), 1)
    );
    assert_eq!(
        bad("x = `a`"),
        ("unexpected character ` (1:5 to 1:6)".into(), 1)
    );
    assert_eq!(
        bad("x = \"é\" é\n"),
        ("unexpected character é (1:9 to 1:10)".into(), 1)
    );
}

#[test]
fn a_shebang_line_is_skipped() {
    assert_eq!(lex("#!/usr/bin/lua\nx")[0], (Kind::Name, "x".into(), 2));
}

#[test]
fn eof_is_on_the_last_line() {
    assert_eq!(
        lex("x\n\n"),
        vec![(Kind::Name, "x".into(), 1), (Kind::Eof, String::new(), 3)]
    );
}
