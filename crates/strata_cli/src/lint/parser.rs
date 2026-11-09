//! A recursive-descent walk over Lua 5.4 tokens that builds no syntax tree.
//!
//! It checks the grammar, reports the structure `strata lint` needs (scopes, functions,
//! calls, tables, assignments and `/`) to an [`Events`] sink, and stops at the first syntax
//! error. Error wording and lines follow `full_moon`, including its quirks (`break` must end
//! a block, and a lone `;` is not a statement).

use super::lexer::{Kind, Token};

/// The first syntax error in a script.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct SyntaxError {
    /// 1-based line the error is reported on.
    pub line: usize,
    /// What is wrong, without the `syntax error:` prefix.
    pub message: String,
}

/// One `name = value` field of a table constructor.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Field {
    /// The field name.
    pub key: String,
    /// Line of the field name.
    pub line: usize,
    /// The raw string body when the value is a lone string literal.
    pub string: Option<String>,
    /// Digit-only number literals in the value, outside function bodies.
    pub numbers: Vec<String>,
}

/// What the walk reports, in source order. `at` values are token offsets that order
/// findings the way a tree walk visits their nodes.
pub trait Events {
    /// A scope opens, already holding `names` (function parameters or loop variables).
    fn open_scope(&mut self, names: Vec<String>);
    /// The innermost scope closes.
    fn close_scope(&mut self);
    /// A `local` statement declares `name` in the innermost scope.
    fn declare(&mut self, name: String);
    /// An anonymous `function ... end` expression starts, before its parameter scope opens.
    fn function_start(&mut self);
    /// The anonymous function that started last ends.
    fn function_end(&mut self);
    /// The argument list of a call starts.
    fn args_start(&mut self);
    /// The argument list that started last ends.
    fn args_end(&mut self);
    /// A table constructor starts.
    fn table_start(&mut self);
    /// The table constructor that started last ends.
    fn table_end(&mut self);
    /// A call whose argument is a table constructor `f { ... }`, with the table's named fields.
    fn call_table(&mut self, at: usize, fields: Vec<Field>);
    /// An assignment statement; `names` are its targets that are plain names, with their lines.
    fn assignment(&mut self, at: usize, names: Vec<(String, usize)>);
    /// A `/` operator on `line`.
    fn division(&mut self, at: usize, line: usize);
}

/// Walks `tokens` (as produced by [`tokenize`](super::lexer::tokenize)), reporting to `events`.
pub fn parse(tokens: &[Token], events: &mut dyn Events) -> Result<(), SyntaxError> {
    let mut parser = Parser {
        tokens,
        pos: 0,
        events,
        last: 0,
        depth: 0,
        numbers: Vec::new(),
    };
    parser.chunk()
}

type Parse<T = ()> = Result<T, SyntaxError>;

/// How a prefix expression such as `a.b(c)` ends.
#[derive(PartialEq, Eq)]
enum Tail {
    /// A plain name with no suffix.
    Name,
    /// A call.
    Call,
    /// A parenthesized expression or an index.
    Other,
}

/// Unary operators bind tighter than every binary operator except `^`.
const UNARY: u8 = 12;

struct Parser<'a> {
    tokens: &'a [Token],
    pos: usize,
    events: &'a mut dyn Events,
    /// Line of the last token consumed, in `full_moon`'s token order: the closing bracket of
    /// a non-empty table, argument list, index or parenthesized expression does not count.
    last: usize,
    /// Function bodies currently open.
    depth: usize,
    /// Every number literal read so far, with the function depth it was read at.
    numbers: Vec<(String, usize)>,
}

impl Parser<'_> {
    fn cur(&self) -> &Token {
        &self.tokens[self.pos]
    }

    fn is(&self, text: &str) -> bool {
        is_symbol(self.cur(), text)
    }

    /// Consumes the current token. The final `Eof` or `Bad` token is never passed.
    fn bump(&mut self) -> Token {
        let token = self.cur().clone();
        if self.pos + 1 < self.tokens.len() {
            self.pos += 1;
        }
        self.last = token.line;
        token
    }

    fn eat(&mut self, text: &str) -> Option<Token> {
        self.is(text).then(|| self.bump())
    }

    /// An error on `line`, unless the current token is unreadable: then its tokenizer
    /// error is the real cause.
    fn error(&self, line: usize, message: impl Into<String>) -> SyntaxError {
        let token = self.cur();
        if token.kind == Kind::Bad {
            return SyntaxError {
                line: token.line,
                message: token.text.clone(),
            };
        }
        SyntaxError {
            line,
            message: message.into(),
        }
    }

    fn expect(&mut self, text: &str, message: &str) -> Parse<Token> {
        let line = self.cur().line;
        self.expect_at(text, line, message)
    }

    fn expect_at(&mut self, text: &str, line: usize, message: &str) -> Parse<Token> {
        self.eat(text).ok_or_else(|| self.error(line, message))
    }

    /// Consumes the closing bracket `text`, which does not move [`Parser::last`] when the
    /// bracketed construct has `content`.
    fn close(&mut self, text: &str, line: usize, message: &str, content: bool) -> Parse {
        let last = self.last;
        self.expect_at(text, line, message)?;
        if content {
            self.last = last;
        }
        Ok(())
    }

    fn name_at(&mut self, line: usize, message: &str) -> Parse<Token> {
        if self.cur().kind != Kind::Name {
            return Err(self.error(line, message));
        }
        Ok(self.bump())
    }

    fn chunk(&mut self) -> Parse {
        self.block()?;
        let line = self.cur().line;
        if self.cur().kind == Kind::Eof {
            return Ok(());
        }
        if !self.starts_statement() {
            return Err(self.error(line, "unexpected token, this needs to be a statement"));
        }
        // The block ended at a `return` or `break`. Like `full_moon`, report a statement
        // after it only once that statement parses.
        self.statement()?;
        Err(SyntaxError {
            line,
            message: "unexpected statement after last statement".to_owned(),
        })
    }

    fn starts_statement(&self) -> bool {
        const STARTS: [&str; 10] = [
            "local", "for", "do", "if", "function", "repeat", "while", "goto", "::", "(",
        ];
        self.cur().kind == Kind::Name || STARTS.iter().any(|s| self.is(s))
    }

    /// Parses statements in a new scope until a token that cannot continue the block.
    /// Returns the first line of the last statement, where a missing `end` is reported.
    fn block(&mut self) -> Parse<Option<usize>> {
        self.events.open_scope(Vec::new());
        let mut last = None;
        loop {
            let line = self.cur().line;
            if self.is("return") || self.is("break") {
                if self.bump().text == "return" {
                    self.expression_list()?;
                }
                self.eat(";");
                last = Some(line);
                break;
            }
            if !self.starts_statement() {
                break;
            }
            self.statement()?;
            self.eat(";");
            last = Some(line);
        }
        self.events.close_scope();
        Ok(last)
    }

    /// Parses a block and the `end` that closes it. `start` is the line reported when the
    /// block is empty.
    fn block_end(&mut self, name: &str, start: usize) -> Parse {
        let line = self.block()?.unwrap_or(start);
        self.expect_at(
            "end",
            line,
            &format!("expected `end` to close {name} block"),
        )?;
        Ok(())
    }

    fn statement(&mut self) -> Parse {
        let token = self.cur().clone();
        if token.kind == Kind::Name || self.is("(") {
            return self.expression_statement();
        }
        self.bump();
        match token.text.as_str() {
            "local" => self.local()?,
            "for" => self.for_loop(token.line)?,
            "function" => {
                self.name_at(self.cur().line, "expected function name")?;
                while let Some(separator) = self.eat(".").or_else(|| self.eat(":")) {
                    let message = format!("expected name after `{}`", separator.text);
                    self.name_at(self.cur().line, &message)?;
                    if separator.text == ":" {
                        break;
                    }
                }
                self.function_body(token.line)?;
            }
            "if" => {
                self.expression(token.line, "expected condition after `if`")?;
                self.expect("then", "expected `then` after condition")?;
                self.block()?;
                while let Some(elseif) = self.eat("elseif") {
                    self.expression(elseif.line, "expected condition after `elseif`")?;
                    self.expect("then", "expected `then` after condition")?;
                    self.block()?;
                }
                if self.eat("else").is_some() {
                    self.block()?;
                }
                self.expect("end", "expected `end` to conclude `if`")?;
            }
            "while" => {
                self.expression(token.line, "expected a condition after `while`")?;
                let open = self.expect("do", "expected `do` after condition")?;
                self.block_end("while loop", open.line)?;
            }
            "repeat" => {
                self.block()?;
                let until = self.expect("until", "expected `until` after block")?;
                self.expression(until.line, "expected a condition after `until`")?;
            }
            "do" => self.block_end("do", token.line)?,
            "goto" => {
                self.name_at(token.line, "expected label name after `goto`")?;
            }
            _ => {
                self.name_at(token.line, "expected label name after `::`")?;
                self.expect("::", "expected `::` after label")?;
            }
        }
        Ok(())
    }

    /// `local function f ...` or `local a <attrib>, b = ...`, after `local`.
    fn local(&mut self) -> Parse {
        if let Some(function) = self.eat("function") {
            let name = self.name_at(self.cur().line, "expected a function name")?;
            self.events.declare(name.text);
            return self.function_body(function.line);
        }
        if self.cur().kind != Kind::Name {
            let line = self.cur().line;
            return Err(self.error(line, "expected either a variable name or `function`"));
        }
        for name in self.names(true)? {
            self.events.declare(name);
        }
        if let Some(equal) = self.eat("=") {
            if !self.expression_list()? {
                return Err(self.error(equal.line, "expected an expression"));
            }
        }
        Ok(())
    }

    /// Parses `name {, name}` starting at a name, with an optional `<attrib>` after each
    /// name when `attributes` is set.
    fn names(&mut self, attributes: bool) -> Parse<Vec<String>> {
        let mut names = Vec::new();
        loop {
            names.push(self.bump().text);
            let angle = if attributes { self.eat("<") } else { None };
            if let Some(angle) = angle {
                self.name_at(angle.line, "expected identifier after `<` for attribute")?;
                self.expect(">", "expected `>` to close attribute")?;
            }
            let Some(comma) = self.eat(",") else {
                return Ok(names);
            };
            if self.cur().kind != Kind::Name {
                return Err(self.error(comma.line, "trailing commas are not allowed"));
            }
        }
    }

    /// A numeric or generic `for` loop, after `for`. The loop variables' scope opens
    /// before the loop's expressions, as in `full_moon`'s tree.
    fn for_loop(&mut self, line: usize) -> Parse {
        if self.cur().kind != Kind::Name {
            return Err(self.error(line, "expected name after `for`"));
        }
        let names = self.names(false)?;
        if names.len() == 1 && self.is("=") {
            let equal = self.bump();
            self.events.open_scope(names);
            self.expression(equal.line, "expected start expression after `=`")?;
            let comma = self.expect(",", "expected `,` after start expression")?;
            self.expression(comma.line, "expected end expression after `,`")?;
            if self.eat(",").is_some() {
                self.expression(comma.line, "expected step expression after `,`")?;
            }
            let open = self.expect("do", "expected `do` after step expression")?;
            self.block_end("numeric for loop", open.line)?;
        } else {
            let keyword = self.expect("in", "expected `in` after name list")?;
            self.events.open_scope(names);
            if !self.expression_list()? {
                return Err(self.error(keyword.line, "expected expressions after `in`"));
            }
            let open = self.expect("do", "expected `do` after expression list")?;
            self.block_end("for loop", open.line)?;
        }
        self.events.close_scope();
        Ok(())
    }

    /// A call statement or an assignment, starting at a name or `(`.
    fn expression_statement(&mut self) -> Parse {
        let start = self.cur().offset;
        let mut names = Vec::new();
        let mut target = self.cur().clone();
        let mut tail = self.suffixed()?;
        if tail == Tail::Call {
            return Ok(());
        }
        if !self.is(",") && !self.is("=") {
            let line = self.cur().line;
            return Err(self.error(line, "unexpected expression when looking for a statement"));
        }
        loop {
            match tail {
                Tail::Call => {
                    let line = self.last;
                    return Err(self.error(line, "can't assign to the result of a call"));
                }
                Tail::Name => names.push((target.text, target.line)),
                Tail::Other => {}
            }
            let Some(comma) = self.eat(",") else {
                break;
            };
            target = self.cur().clone();
            if target.kind != Kind::Name && !self.is("(") {
                return Err(self.error(comma.line, "expected another variable"));
            }
            tail = self.suffixed()?;
        }
        let equal = self.expect("=", "expected `=` after name")?;
        self.events.assignment(start, names);
        if !self.expression_list()? {
            return Err(self.error(equal.line, "expected values to set to"));
        }
        Ok(())
    }

    /// Parses `expr {, expr}`. Returns false, consuming nothing, when no expression starts here.
    fn expression_list(&mut self) -> Parse<bool> {
        if !self.starts_expression() {
            return Ok(false);
        }
        loop {
            self.subexpression(0, 0, "")?;
            let Some(comma) = self.eat(",") else {
                return Ok(true);
            };
            if !self.starts_expression() {
                return Err(self.error(comma.line, "trailing commas are not allowed"));
            }
        }
    }

    fn starts_expression(&self) -> bool {
        const STARTS: [&str; 11] = [
            "(", "{", "function", "nil", "true", "false", "...", "-", "not", "#", "~",
        ];
        matches!(self.cur().kind, Kind::Name | Kind::Number | Kind::Str)
            || STARTS.iter().any(|s| self.is(s))
    }

    /// Parses an expression, or fails with `message` on `line` when none starts here.
    fn expression(&mut self, line: usize, message: &str) -> Parse {
        self.subexpression(0, line, message)
    }

    /// Parses an expression whose binary operators all bind tighter than `limit`.
    fn subexpression(&mut self, limit: u8, line: usize, message: &str) -> Parse {
        if !self.starts_expression() {
            return Err(self.error(line, message));
        }
        let start = self.cur().offset;
        if ["-", "not", "#", "~"].iter().any(|s| self.is(s)) {
            let op = self.bump();
            let message = format!("expected an expression after {}", op.text);
            self.subexpression(UNARY, op.line, &message)?;
        } else {
            self.simple()?;
        }
        while let Some((_, right)) = priority(self.cur()).filter(|(left, _)| *left > limit) {
            let op = self.bump();
            if op.text == "/" {
                self.events.division(start, op.line);
            }
            self.subexpression(right, op.line, "expected expression after binary operator")?;
        }
        Ok(())
    }

    /// A literal, function, table or prefix expression; the caller checked that one starts here.
    fn simple(&mut self) -> Parse {
        let token = self.cur().clone();
        if token.kind == Kind::Name || self.is("(") {
            self.suffixed()?;
        } else if self.is("{") {
            self.bump();
            self.table(&token)?;
        } else if self.is("function") {
            self.bump();
            self.events.function_start();
            self.function_body(token.line)?;
            self.events.function_end();
        } else {
            if token.kind == Kind::Number {
                self.numbers.push((token.text, self.depth));
            }
            self.bump();
        }
        Ok(())
    }

    /// A name or parenthesized expression followed by any `.name`, `[expr]`, `:name args`
    /// and call suffixes.
    fn suffixed(&mut self) -> Parse<Tail> {
        let first = self.bump();
        let mut tail = Tail::Name;
        if first.kind != Kind::Name {
            self.expression(first.line, "expected an expression after `(`")?;
            let line = self.cur().line;
            self.close(")", line, "expected `)` after expression", true)?;
            tail = Tail::Other;
        }
        loop {
            let token = self.cur().clone();
            if token.kind == Kind::Str || self.is("(") || self.is("{") {
                self.arguments()?;
                tail = Tail::Call;
            } else if self.eat(".").is_some() {
                self.name_at(token.line, "expected identifier after `.`")?;
                tail = Tail::Other;
            } else if self.eat("[").is_some() {
                self.expression(token.line, "expected expression after `[`")?;
                self.close(
                    "]",
                    token.line,
                    "expected `]` to close index expression",
                    true,
                )?;
                tail = Tail::Other;
            } else if self.eat(":").is_some() {
                let name = self.name_at(token.line, "expected identifier after `:`")?;
                let next = self.cur();
                if !(next.kind == Kind::Str || self.is("(") || self.is("{")) {
                    return Err(self.error(name.line, "expected arguments after `:`"));
                }
                self.arguments()?;
                tail = Tail::Call;
            } else {
                return Ok(tail);
            }
        }
    }

    /// Call arguments: a string, a table constructor or a parenthesized list.
    fn arguments(&mut self) -> Parse {
        let open = self.bump();
        self.events.args_start();
        if is_symbol(&open, "{") {
            let fields = self.table(&open)?;
            self.events.call_table(open.offset, fields);
        } else if is_symbol(&open, "(") {
            let content = self.expression_list()?;
            self.close(
                ")",
                open.line,
                "expected `)` to close function call",
                content,
            )?;
        }
        self.events.args_end();
        Ok(())
    }

    /// A table constructor after its `{`. Returns its `name = value` fields.
    fn table(&mut self, open: &Token) -> Parse<Vec<Field>> {
        self.events.table_start();
        let mut fields = Vec::new();
        let (mut last_line, mut content) = (open.line, false);
        while !self.is("}") {
            let token = self.cur().clone();
            let keyed = self
                .tokens
                .get(self.pos + 1)
                .is_some_and(|t| is_symbol(t, "="));
            if token.kind == Kind::Name && keyed {
                self.bump();
                let equal = self.bump();
                let (first, mark) = (self.pos, self.numbers.len());
                self.expression(equal.line, "expected an expression after `=`")?;
                let lone = self.pos == first + 1 && self.tokens[first].kind == Kind::Str;
                let numbers = self.numbers[mark..].iter().filter(|(text, depth)| {
                    *depth == self.depth && text.bytes().all(|b| b.is_ascii_digit())
                });
                fields.push(Field {
                    key: token.text,
                    line: token.line,
                    string: lone.then(|| self.tokens[first].text.clone()),
                    numbers: numbers.map(|(text, _)| text.clone()).collect(),
                });
            } else if self.eat("[").is_some() {
                self.expression(token.line, "expected an expression after `[`")?;
                self.expect_at("]", token.line, "expected `]` after expression")?;
                let equal = self.expect_at("=", token.line, "expected `=` after expression")?;
                self.expression(equal.line, "expected an expression after `=`")?;
            } else {
                self.expression(last_line, "expected a field")?;
            }
            (last_line, content) = (self.last, true);
            if self.eat(",").is_none() && self.eat(";").is_none() {
                break;
            }
        }
        let line = self.cur().line;
        self.close("}", line, "expected `}` after last field", content)?;
        self.events.table_end();
        Ok(fields)
    }

    /// A parameter list and body after `function` (and its name). `line` is the
    /// `function` keyword's line, where a missing parameter list is reported.
    fn function_body(&mut self, line: usize) -> Parse {
        self.expect_at("(", line, "expected a function body")?;
        let mut parameters = Vec::new();
        let mut comma = None;
        loop {
            let token = self.cur().clone();
            if self.eat(")").is_some() {
                break;
            }
            if self.eat("...").is_some() {
                comma = None;
                self.expect(")", "expected a `)`")?;
                break;
            }
            if token.kind != Kind::Name {
                return Err(self.error(token.line, "expected a parameter name or `)`"));
            }
            parameters.push(self.bump().text);
            comma = self.eat(",").map(|c| c.line);
            if comma.is_none() {
                self.expect(")", "expected a `)`")?;
                break;
            }
        }
        if let Some(line) = comma {
            return Err(self.error(line, "trailing commas in arguments are not allowed"));
        }
        self.depth += 1;
        self.events.open_scope(parameters);
        self.block_end("function body", self.last)?;
        self.events.close_scope();
        self.depth -= 1;
        Ok(())
    }
}

fn is_symbol(token: &Token, text: &str) -> bool {
    token.kind == Kind::Symbol && token.text == text
}

/// Left and right binding power of a binary operator, as in the Lua 5.4 reference parser.
fn priority(token: &Token) -> Option<(u8, u8)> {
    if token.kind != Kind::Symbol {
        return None;
    }
    let power = match token.text.as_str() {
        "or" => (1, 1),
        "and" => (2, 2),
        "<" | ">" | "<=" | ">=" | "~=" | "==" => (3, 3),
        "|" => (4, 4),
        "~" => (5, 5),
        "&" => (6, 6),
        "<<" | ">>" => (7, 7),
        ".." => (9, 8),
        "+" | "-" => (10, 10),
        "*" | "/" | "//" | "%" => (11, 11),
        "^" => (14, 13),
        _ => return None,
    };
    Some(power)
}

#[cfg(test)]
mod tests;
