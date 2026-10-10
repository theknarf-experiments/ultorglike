//! Small, deliberately SQL-free numeric formula language.
use crate::{Error, Result};

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Expr {
    Number(String),
    Field(String),
    Negate(Box<Expr>),
    Binary(char, Box<Expr>, Box<Expr>),
    Aggregate { count: bool, path: String },
}

pub fn parse(source: &str) -> Result<Expr> {
    if source.len() > 2048 {
        return Err(Error::Invalid("Formula exceeds 2048 bytes".into()));
    }
    let mut parser = Parser {
        source,
        pos: 0,
        terms: 0,
    };
    let expression = parser.expression(0)?;
    parser.space();
    if parser.pos != source.len() {
        return parser.error("Unexpected input");
    }
    Ok(expression)
}

struct Parser<'a> {
    source: &'a str,
    pos: usize,
    terms: usize,
}
impl Parser<'_> {
    fn error<T>(&self, message: &str) -> Result<T> {
        Err(Error::Invalid(format!(
            "{message} at formula byte {}",
            self.pos + 1
        )))
    }
    fn space(&mut self) {
        while self
            .source
            .as_bytes()
            .get(self.pos)
            .is_some_and(u8::is_ascii_whitespace)
        {
            self.pos += 1;
        }
    }
    fn take(&mut self, c: u8) -> bool {
        self.space();
        if self.source.as_bytes().get(self.pos) == Some(&c) {
            self.pos += 1;
            true
        } else {
            false
        }
    }
    fn expression(&mut self, depth: usize) -> Result<Expr> {
        let mut lhs = self.product(depth)?;
        loop {
            let op = if self.take(b'+') {
                '+'
            } else if self.take(b'-') {
                '-'
            } else {
                break;
            };
            lhs = Expr::Binary(op, Box::new(lhs), Box::new(self.product(depth)?));
        }
        Ok(lhs)
    }
    fn product(&mut self, depth: usize) -> Result<Expr> {
        let mut lhs = self.atom(depth)?;
        loop {
            let op = if self.take(b'*') {
                '*'
            } else if self.take(b'/') {
                '/'
            } else {
                break;
            };
            lhs = Expr::Binary(op, Box::new(lhs), Box::new(self.atom(depth)?));
        }
        Ok(lhs)
    }
    fn reference(&mut self) -> Result<String> {
        if !self.take(b'[') {
            return self.error("Expected [field] or [relationship.field]");
        }
        let start = self.pos;
        let Some(end) = self.source[start..].find(']') else {
            return self.error("Missing closing ]");
        };
        let name = self.source[start..start + end].trim().to_string();
        self.pos = start + end + 1;
        if name.is_empty() {
            return self.error("Empty field reference");
        }
        Ok(name)
    }
    fn atom(&mut self, depth: usize) -> Result<Expr> {
        self.terms += 1;
        if depth > 32 || self.terms > 256 {
            return self.error("Formula exceeds 32 nested levels or 256 terms");
        }
        self.space();
        if self.take(b'-') {
            return Ok(Expr::Negate(Box::new(self.atom(depth + 1)?)));
        }
        if self.take(b'+') {
            return self.atom(depth + 1);
        }
        if self.take(b'(') {
            let inner = self.expression(depth + 1)?;
            if !self.take(b')') {
                return self.error("Missing closing )");
            }
            return Ok(inner);
        }
        if self.source.as_bytes().get(self.pos) == Some(&b'[') {
            return Ok(Expr::Field(self.reference()?));
        }
        for (function, count) in [("sum", false), ("count", true)] {
            if self.source[self.pos..].starts_with(function) {
                self.pos += function.len();
                if !self.take(b'(') {
                    return self.error("Expected ( after aggregate");
                }
                let path = self.reference()?;
                if !self.take(b')') {
                    return self.error("Expected ) after aggregate reference");
                }
                return Ok(Expr::Aggregate { count, path });
            }
        }
        let start = self.pos;
        let mut digits = 0;
        while self
            .source
            .as_bytes()
            .get(self.pos)
            .is_some_and(u8::is_ascii_digit)
        {
            self.pos += 1;
            digits += 1;
        }
        if self.source.as_bytes().get(self.pos) == Some(&b'.') {
            self.pos += 1;
            while self
                .source
                .as_bytes()
                .get(self.pos)
                .is_some_and(u8::is_ascii_digit)
            {
                self.pos += 1;
                digits += 1;
            }
        }
        if digits == 0 {
            return self.error("Expected a number, [field], sum(...) or count(...)");
        }
        Ok(Expr::Number(self.source[start..self.pos].into()))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn precedence_references_and_decimal_literals() {
        assert_eq!(
            parse("1 + 2 * -[unit price]").unwrap(),
            Expr::Binary(
                '+',
                Box::new(Expr::Number("1".into())),
                Box::new(Expr::Binary(
                    '*',
                    Box::new(Expr::Number("2".into())),
                    Box::new(Expr::Negate(Box::new(Expr::Field("unit price".into()))))
                ))
            )
        );
        for source in [
            "([quantity] * [price]) / .5",
            "sum([items.amount]) + 10.25",
            "count([items])",
            "[東京]",
        ] {
            parse(source).unwrap();
        }
    }
    #[test]
    fn rejects_sql_malformed_and_excessive_input() {
        for source in [
            "",
            "1; SELECT 1",
            "sum([items.amount]",
            "sum(1)",
            "[]",
            "[x",
            "1..2",
            "pg_sleep(5)",
            "1e10",
            "1 -- comment",
        ] {
            assert!(parse(source).is_err(), "{source}");
        }
        assert!(parse(&format!("{}1{}", "(".repeat(40), ")".repeat(40))).is_err());
        assert!(parse(&"1+".repeat(300)).is_err());
    }
}
