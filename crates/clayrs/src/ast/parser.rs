use miette::NamedSource;

use super::*;

lalrpop_util::lalrpop_mod!(grammar, "/ast/grammar.rs");

pub fn parse_proc(s: &str, file_name: Option<String>) -> Result<ast::Proc, miette::Report> {
    let report = grammar::ProcParser::new().parse(lexer::Lexer::new(s));
    match file_name {
        Some(file_name) => report.with_source_code(NamedSource::new(&file_name, s.to_string())),
        None => report.with_source_code(s.to_string())
    }
}

#[cfg(test)]
mod tests {
    use std::result;

    use super::*;
    
    #[test]
    fn test_literal() {
        fn parse(s: &str) {
            let lexer = lexer::Lexer::new(s);
            grammar::NumberParser::new().parse(lexer)
            .with_source_code(s.to_string()).unwrap();
        }

        parse("1231231");
        parse("12345678901234566789");
        parse("0x1234");
        parse("5'b101010");
        parse("3'o123");
        parse("15'd123");
        parse("8'hFF");
    }

    #[test]
    fn test_expr() {
        fn parse(s: &str) {
            let lexer = lexer::Lexer::new(s);
            // grammar::ExprParser::new().parse(lexer)
            // .with_source_code(s.to_string()).unwrap();
            let result = grammar::ExprParser::new().parse(lexer);
            println!("`{}` => `{:?}`", s, result);
            result.with_source_code(s.to_string()).unwrap();
        }

        parse("mdfasdf");
        parse("(a,b,c)");
        parse("(a)");
        parse("xyz(xxx)");
        parse("a[asdaadasd]");
        parse("a[1]");
        parse("vec[1:3]");
    }

    #[test]
    fn test_stmts() {
        fn parse(s: &str) {
            let lexer = lexer::Lexer::new(s);
            let result = grammar::StmtParser::new().parse(lexer);
            println!("`{}` => `{:?}`", s, result);
            result.with_source_code(s.to_string()).unwrap();
        }

        parse("const x = 123123;");
        parse("let x = 123123;");
        parse("sdfadf = 31231;");
        parse("dfasdfas;");
    }

    #[test]
    fn test_structs() {
        fn parse(s: &str) {
            let lexer = lexer::Lexer::new(s);
            println!("`{}` => `{:?}`", s, grammar::PortParser::new().parse(lexer)
            .with_source_code(s.to_string()).unwrap());
        }

        // parse("struct Foo { x: u32, y: u32 }");
        parse("asdfasdf: 3");
    }
}