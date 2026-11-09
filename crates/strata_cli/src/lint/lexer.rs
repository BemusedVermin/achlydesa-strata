//! Splits Lua 5.4 source into tokens for `strata lint`.
//!
//! Error messages and positions follow `full_moon`, the parser the checks used before,
//! so syntax-error diagnostics read the same.

/// What kind of token this is.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Kind {
    /// An identifier that is not a keyword.
    Name,
    /// A keyword or an operator, spelled in `text`.
    Symbol,
    /// A numeric literal.
    Number,
    /// A string literal. `text` is the raw body between its delimiters, escapes untouched.
    Str,
    /// Source the tokenizer could not read. `text` is the error message; it is always the last token.
    Bad,
    /// The end of the source; always the last token when there is no `Bad` one.
    Eof,
}

/// One token with the position the checks report.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Token {
    /// What kind of token this is.
    pub kind: Kind,
    /// The token's text (see [`Kind`]).
    pub text: String,
    /// 1-based line of the token's first character.
    pub line: usize,
    /// Index of the token's first character in the source.
    pub offset: usize,
}

const KEYWORDS: [&str; 22] = [
    "and", "break", "do", "else", "elseif", "end", "false", "for", "function", "goto", "if", "in",
    "local", "nil", "not", "or", "repeat", "return", "then", "true", "until", "while",
];

/// Operators, longest first so that the first match is the right one.
const SYMBOLS: [&str; 33] = [
    "...", "..", "==", "~=", "<=", ">=", "<<", ">>", "//", "::", "+", "-", "*", "/", "%", "^", "#",
    "&", "~", "|", "<", ">", "=", "(", ")", "{", "}", "[", "]", ";", ":", ",", ".",
];

/// Tokenizes `source`. The result always ends with an `Eof` token, or with a `Bad` token
/// at the first character sequence that is not valid Lua.
pub fn tokenize(source: &str) -> Vec<Token> {
    let mut lexer = Lexer {
        chars: source.chars().collect(),
        pos: 0,
        line: 1,
        column: 1,
    };
    if lexer.starts_with("#!") {
        lexer.take_while(|c| c != '\n');
    }
    let mut tokens = Vec::new();
    loop {
        let (line, column, offset) = (lexer.line, lexer.column, lexer.pos);
        let Some(c) = lexer.at(0) else {
            tokens.push(Token {
                kind: Kind::Eof,
                text: String::new(),
                line,
                offset,
            });
            return tokens;
        };
        match lexer.read(c) {
            Ok(None) => {}
            Ok(Some((kind, text))) => tokens.push(Token {
                kind,
                text,
                line,
                offset,
            }),
            Err(error) => {
                let end = (lexer.line, lexer.column);
                let text = format!("{error} ({line}:{column} to {}:{})", end.0, end.1);
                tokens.push(Token {
                    kind: Kind::Bad,
                    text,
                    line,
                    offset,
                });
                return tokens;
            }
        }
    }
}

struct Lexer {
    chars: Vec<char>,
    pos: usize,
    line: usize,
    column: usize,
}

impl Lexer {
    fn at(&self, ahead: usize) -> Option<char> {
        self.chars.get(self.pos + ahead).copied()
    }

    fn bump(&mut self) {
        if let Some(c) = self.at(0) {
            self.pos += 1;
            if c == '\n' {
                self.line += 1;
                self.column = 1;
            } else {
                self.column += 1;
            }
        }
    }

    fn skip(&mut self, count: usize) {
        for _ in 0..count {
            self.bump();
        }
    }

    fn starts_with(&self, text: &str) -> bool {
        text.chars().enumerate().all(|(i, c)| self.at(i) == Some(c))
    }

    fn since(&self, start: usize) -> String {
        self.chars[start..self.pos].iter().collect()
    }

    fn take_while(&mut self, keep: impl Fn(char) -> bool) -> String {
        let start = self.pos;
        while self.at(0).is_some_and(&keep) {
            self.bump();
        }
        self.since(start)
    }

    /// Reads one token starting at `c`. `Ok(None)` means whitespace or a comment.
    fn read(&mut self, c: char) -> Result<Option<(Kind, String)>, String> {
        if matches!(c, ' ' | '\t' | '\r' | '\n') {
            self.bump();
            return Ok(None);
        }
        if self.starts_with("--") {
            self.skip(2);
            return self.comment().map(|()| None);
        }
        let token = if c.is_ascii_alphabetic() || c == '_' {
            let word = self.take_while(|c| c.is_ascii_alphanumeric() || c == '_');
            let kind = if KEYWORDS.contains(&word.as_str()) {
                Kind::Symbol
            } else {
                Kind::Name
            };
            (kind, word)
        } else if c.is_ascii_digit() || (c == '.' && self.at(1).is_some_and(|d| d.is_ascii_digit()))
        {
            (Kind::Number, self.number()?)
        } else if c == '"' || c == '\'' {
            (Kind::Str, self.quoted(c)?)
        } else if let Some(level) = (c == '[').then(|| self.long_open()).flatten() {
            (Kind::Str, self.long_body(level).ok_or("unclosed string")?)
        } else if let Some(symbol) = SYMBOLS.iter().find(|s| self.starts_with(s)) {
            self.skip(symbol.len());
            (Kind::Symbol, (*symbol).to_owned())
        } else {
            self.bump();
            return Err(format!("unexpected character {c}"));
        };
        Ok(Some(token))
    }

    /// Skips a comment after its `--`.
    fn comment(&mut self) -> Result<(), String> {
        if self.at(0) == Some('[') {
            if let Some(level) = self.long_open() {
                let body = self.long_body(level);
                return body.map(drop).ok_or_else(|| "unclosed comment".to_owned());
            }
        }
        self.take_while(|c| c != '\n');
        Ok(())
    }

    /// At `[`, consumes a long-bracket opener such as `[==[` and returns its level.
    /// Consumes nothing when the `[` does not open a long bracket.
    fn long_open(&mut self) -> Option<usize> {
        let level = (1..).take_while(|&i| self.at(i) == Some('=')).count();
        if self.at(level + 1) != Some('[') {
            return None;
        }
        self.skip(level + 2);
        Some(level)
    }

    /// Reads a long bracket's body up to its matching close. `None` when the source ends first.
    fn long_body(&mut self, level: usize) -> Option<String> {
        let close = format!("]{}]", "=".repeat(level));
        let start = self.pos;
        while !self.starts_with(&close) {
            self.at(0)?;
            self.bump();
        }
        let body = self.since(start);
        self.skip(close.len());
        Some(body)
    }

    /// Reads a quoted string. Like `full_moon`, a line break is allowed right after any
    /// escape sequence (which covers `\` at the end of a line and `\z`).
    fn quoted(&mut self, quote: char) -> Result<String, String> {
        self.bump();
        let start = self.pos;
        let (mut escape, mut continued) = (false, false);
        loop {
            let Some(c) = self.at(0) else {
                return Err("unclosed string".to_owned());
            };
            let end = self.pos;
            self.bump();
            if escape {
                escape = false;
                continued = true;
            } else if c == '\\' {
                escape = true;
            } else if c == '\n' || c == '\r' {
                if !continued {
                    return Err("unclosed string".to_owned());
                }
                continued = false;
            } else if c == quote {
                return Ok(self.chars[start..end].iter().collect());
            }
        }
    }

    /// Reads a decimal or hexadecimal number, with the same acceptance rules as `full_moon`.
    fn number(&mut self) -> Result<String, String> {
        let start = self.pos;
        let hex = self.starts_with("0x") || self.starts_with("0X");
        self.skip(if hex { 2 } else { 1 });
        let mut decimal = false;
        loop {
            match self.at(0) {
                Some(c) if c.is_ascii_digit() || (hex && c.is_ascii_hexdigit()) => self.bump(),
                Some('.') if !decimal => {
                    decimal = true;
                    self.bump();
                }
                Some('.') => return self.invalid_number(),
                Some('e' | 'E') if !hex => return self.exponent(start),
                Some('p' | 'P') if hex && self.pos - start > 2 => return self.exponent(start),
                Some('p' | 'P') if hex => return self.invalid_number(),
                _ => break,
            }
        }
        if hex && self.pos - start == 2 {
            return self.invalid_number();
        }
        Ok(self.since(start))
    }

    fn exponent(&mut self, start: usize) -> Result<String, String> {
        self.bump();
        if matches!(self.at(0), Some('+' | '-')) {
            self.bump();
        }
        if !self.at(0).is_some_and(|c| c.is_ascii_digit()) {
            return self.invalid_number();
        }
        self.take_while(|c| c.is_ascii_digit());
        Ok(self.since(start))
    }

    /// Skips the rest of a malformed number, up to whitespace.
    fn invalid_number(&mut self) -> Result<String, String> {
        self.take_while(|c| !c.is_ascii_whitespace());
        Err("invalid number".to_owned())
    }
}

#[cfg(test)]
mod tests;
