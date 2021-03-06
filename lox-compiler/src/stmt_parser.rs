use super::ast::*;
use super::common::*;
use super::token::*;
use std::iter::{Iterator, Peekable};
use crate::position::WithSpan;

fn parse_program<'a, It>(it: &mut Peekable<It>) -> Result<Vec<Stmt>, ParseError>
where
    It: Iterator<Item = &'a WithSpan<Token>>,
{
    let mut statements = Vec::new();
    while let Some(_) = it.peek() {
        statements.push(parse_declaration(it)?);
    }
    match it.next() {
        Some(t) => Err(ParseError { error: "Expected None".into(), span: Some(t.span) }),
        None => Ok(statements),
    }
}

fn parse_declaration<'a, It>(it: &mut Peekable<It>) -> Result<Stmt, ParseError>
where
    It: Iterator<Item = &'a WithSpan<Token>>,
{
    match peek(it)? {
        &Token::Var => parse_var_declaration(it),
        &Token::Fun => parse_function_declaration(it),
        &Token::Class => parse_class_declaration(it),
        _ => parse_statement(it),
    }
}

fn parse_statement<'a, It>(it: &mut Peekable<It>) -> Result<Stmt, ParseError>
where
    It: Iterator<Item = &'a WithSpan<Token>>,
{
    match peek(it)? {
        &Token::Print => parse_print_statement(it),
        &Token::If => parse_if_statement(it),
        &Token::LeftBrace => parse_block_statement(it),
        &Token::While => parse_while_statement(it),
        &Token::Return => parse_return_statement(it),
        &Token::For => parse_for_statement(it),
        _ => parse_expr_statement(it),
    }
}

fn parse_class_declaration<'a, It>(it: &mut Peekable<It>) -> Result<Stmt, ParseError>
where
    It: Iterator<Item = &'a WithSpan<Token>>,
{
    expect(it, &Token::Class)?;
    let name = expect!(it, Token::Identifier(i) => i)?;
    let superclass = if optionally(it, &Token::Less)? {
        let name = expect!(it, Token::Identifier(i) => i)?;
        Some(name.clone())
    } else {
        None
    };
    expect(it, &Token::LeftBrace)?;
    let mut functions: Vec<Stmt> = vec![];
    while peek(it)? != &Token::RightBrace {
        functions.push(parse_function(it)?);
    }
    expect(it, &Token::RightBrace)?;

    Ok(Stmt::Class(name.clone(), superclass, functions))
}

fn parse_function_declaration<'a, It>(it: &mut Peekable<It>) -> Result<Stmt, ParseError>
where
    It: Iterator<Item = &'a WithSpan<Token>>,
{
    expect(it, &Token::Fun)?;
    parse_function(it)
}

fn parse_function<'a, It>(it: &mut Peekable<It>) -> Result<Stmt, ParseError>
where
    It: Iterator<Item = &'a WithSpan<Token>>,
{
    let name = expect!(it, Token::Identifier(i) => i)?;
    expect(it, &Token::LeftParen)?;
    let params = if peek(it)? != &Token::RightParen {
        parse_params(it)?
    } else {
        Vec::new()
    };
    expect(it, &Token::RightParen)?;
    expect(it, &Token::LeftBrace)?;
    let mut body: Vec<Stmt> = Vec::new();
    while peek(it)? != &Token::RightBrace {
        body.push(parse_declaration(it)?);
    }
    expect(it, &Token::RightBrace)?;
    Ok(Stmt::Function(name.clone(), params, body))
}

fn parse_params<'a, It>(it: &mut Peekable<It>) -> Result<Vec<Identifier>, ParseError>
where
    It: Iterator<Item = &'a WithSpan<Token>>,
{
    let mut params: Vec<Identifier> = Vec::new();
    params.push(expect!(it, Token::Identifier(i) => i.clone())?);
    while peek(it)? == &Token::Comma {
        expect(it, &Token::Comma)?;
        params.push(expect!(it, Token::Identifier(i) => i.clone())?);
    }
    Ok(params)
}

fn parse_var_declaration<'a, It>(it: &mut Peekable<It>) -> Result<Stmt, ParseError>
where
    It: Iterator<Item = &'a WithSpan<Token>>,
{
    expect(it, &Token::Var)?;
    let name = expect_with_span!(it, Token::Identifier(i) => i.clone())?;
    let mut initializer: Option<Expr> = None;

    if optionally(it, &Token::Equal)? {
        initializer = Some(parse_expr(it)?);
    }

    expect(it, &Token::Semicolon)?;

    Ok(Stmt::Var(name, initializer.map(Box::new)))
}

fn parse_expr<'a, It>(it: &mut Peekable<It>) -> Result<Expr, ParseError>
where
    It: Iterator<Item = &'a WithSpan<Token>>,
{
    super::expr_parser::parse(it).map_err(|e| e.into())
}

fn parse_for_statement<'a, It>(it: &mut Peekable<It>) -> Result<Stmt, ParseError>
where
    It: Iterator<Item = &'a WithSpan<Token>>,
{
    expect(it, &Token::For)?;
    expect(it, &Token::LeftParen)?;
    let initializer = match peek(it)? {
        &Token::Var => Some(parse_var_declaration(it)?),
        &Token::Semicolon => {
            expect(it, &Token::Semicolon)?;
            None
        }
        _ => Some(parse_expr_statement(it)?),
    };
    let condition = if peek(it)? != &Token::Semicolon {
        parse_expr(it)?
    } else {
        Expr::Boolean(true)
    };
    expect(it, &Token::Semicolon)?;
    let increment = if peek(it)? != &Token::RightParen {
        Some(parse_expr(it)?)
    } else {
        None
    };
    expect(it, &Token::RightParen)?;
    let body = parse_statement(it)?;
    // Add increment if it exists
    let body = match increment {
        Some(expr) => Stmt::Block(vec![body, Stmt::Expression(Box::new(expr))]),
        None => body,
    };
    let body = Stmt::While(Box::new(condition), Box::new(body));
    let body = match initializer {
        Some(stmt) => Stmt::Block(vec![stmt, body]),
        None => body,
    };

    Ok(body)
}

fn parse_return_statement<'a, It>(it: &mut Peekable<It>) -> Result<Stmt, ParseError>
where
    It: Iterator<Item = &'a WithSpan<Token>>,
{
    expect(it, &Token::Return)?;
    let mut expr: Option<Expr> = None;
    if peek(it)? != &Token::Semicolon {
        expr = Some(parse_expr(it)?);
    }
    expect(it, &Token::Semicolon)?;
    Ok(Stmt::Return(expr.map(Box::new)))
}

fn parse_expr_statement<'a, It>(it: &mut Peekable<It>) -> Result<Stmt, ParseError>
where
    It: Iterator<Item = &'a WithSpan<Token>>,
{
    let expr = parse_expr(it)?;
    expect(it, &Token::Semicolon)?;

    Ok(Stmt::Expression(Box::new(expr)))
}

fn parse_block_statement<'a, It>(it: &mut Peekable<It>) -> Result<Stmt, ParseError>
where
    It: Iterator<Item = &'a WithSpan<Token>>,
{
    expect(it, &Token::LeftBrace)?;
    let mut statements: Vec<Stmt> = Vec::new();
    while peek(it)? != &Token::RightBrace {
        statements.push(parse_declaration(it)?);
    }
    expect(it, &Token::RightBrace)?;
    Ok(Stmt::Block(statements))
}

fn parse_while_statement<'a, It>(it: &mut Peekable<It>) -> Result<Stmt, ParseError>
where
    It: Iterator<Item = &'a WithSpan<Token>>,
{
    expect(it, &Token::While)?;
    expect(it, &Token::LeftParen)?;
    let condition = parse_expr(it)?;
    expect(it, &Token::RightParen)?;
    let statement = parse_statement(it)?;
    Ok(Stmt::While(Box::new(condition), Box::new(statement)))
}

fn parse_if_statement<'a, It>(it: &mut Peekable<It>) -> Result<Stmt, ParseError>
where
    It: Iterator<Item = &'a WithSpan<Token>>,
{
    expect(it, &Token::If)?;
    expect(it, &Token::LeftParen)?;
    let condition = parse_expr(it)?;
    expect(it, &Token::RightParen)?;
    let if_stmt = parse_statement(it)?;
    let mut else_stmt: Option<Stmt> = None;

    if optionally(it, &Token::Else)? {
        else_stmt = Some(parse_statement(it)?);
    }

    Ok(Stmt::If(
        Box::new(condition),
        Box::new(if_stmt),
        else_stmt.map(Box::new),
    ))
}

fn parse_print_statement<'a, It>(it: &mut Peekable<It>) -> Result<Stmt, ParseError>
where
    It: Iterator<Item = &'a WithSpan<Token>>,
{
    expect(it, &Token::Print)?;
    let expr = parse_expr(it)?;
    expect(it, &Token::Semicolon)?;
    Ok(Stmt::Print(Box::new(expr)))
}

pub fn parse<'a, It>(it: &mut Peekable<It>) -> Result<Vec<Stmt>, ParseError>
where
    It: Iterator<Item = &'a WithSpan<Token>>,
{
    parse_program(it)
}

#[cfg(test)]
mod tests {
    use super::super::tokenizer::*;
    use super::*;
    fn parse_str(data: &str) -> Result<Vec<Stmt>, String> {
        let tokens = tokenize_with_context(data);
        let mut it = tokens.as_slice().into_iter().peekable();
        parse(&mut it).map_err(|e| e.error) //TODO
    }

    #[test]
    fn test_expr_stmt() {
        assert_eq!(
            parse_str("nil;"),
            Ok(vec![Stmt::Expression(Box::new(Expr::Nil)),])
        );
        assert_eq!(
            parse_str("nil;nil;"),
            Ok(vec![
                Stmt::Expression(Box::new(Expr::Nil)),
                Stmt::Expression(Box::new(Expr::Nil)),
            ])
        );
    }

    #[test]
    fn test_print_stmt() {
        assert_eq!(
            parse_str("print nil;"),
            Ok(vec![Stmt::Print(Box::new(Expr::Nil)),])
        );
    }

    fn make_span_string(string: &str, column: usize) -> WithSpan<String> {
        use crate::position::{Span, Position};
        let start = Position { line: 1, column };
        let end = Position { line: 1, column: column + string.len()-1 };
        WithSpan::new(string.to_string(), Span { start, end })
    }

    #[test]
    fn test_var_decl() {
        assert_eq!(
            parse_str("var beverage;"),
            Ok(vec![Stmt::Var(make_span_string("beverage", 5), None),])
        );
        assert_eq!(
            parse_str("var beverage = nil;"),
            Ok(vec![Stmt::Var(
                make_span_string("beverage", 5),
                Some(Box::new(Expr::Nil))
            ),])
        );

        assert_eq!(
            parse_str("var beverage = x = nil;"),
            Ok(vec![Stmt::Var(
                make_span_string("beverage", 5),
                Some(Box::new(Expr::Assign("x".into(), Box::new(Expr::Nil))))
            ),])
        );

        assert_eq!(
            parse_str("if (nil) var beverage = nil;"),
            Err("unexpected token: Var".into())
        );
    }

    #[test]
    fn test_if_stmt() {
        assert_eq!(
            parse_str("if(nil) print nil;"),
            Ok(vec![Stmt::If(
                Box::new(Expr::Nil),
                Box::new(Stmt::Print(Box::new(Expr::Nil))),
                None,
            ),])
        );
        assert_eq!(
            parse_str("if(nil) print nil; else print false;"),
            Ok(vec![Stmt::If(
                Box::new(Expr::Nil),
                Box::new(Stmt::Print(Box::new(Expr::Nil))),
                Some(Box::new(Stmt::Print(Box::new(Expr::Boolean(false))))),
            ),])
        );
    }

    #[test]
    fn test_block_stmt() {
        assert_eq!(parse_str("{}"), Ok(vec![Stmt::Block(vec![])]));
        assert_eq!(
            parse_str("{nil;}"),
            Ok(vec![Stmt::Block(vec![Stmt::Expression(Box::new(
                Expr::Nil
            )),])])
        );
        assert_eq!(
            parse_str("{nil;nil;}"),
            Ok(vec![Stmt::Block(vec![
                Stmt::Expression(Box::new(Expr::Nil)),
                Stmt::Expression(Box::new(Expr::Nil)),
            ])])
        );
    }

    #[test]
    fn test_while_stmt() {
        assert_eq!(
            parse_str("while(nil)false;"),
            Ok(vec![Stmt::While(
                Box::new(Expr::Nil),
                Box::new(Stmt::Expression(Box::new(Expr::Boolean(false)))),
            )])
        );
    }

    #[test]
    fn test_return_stmt() {
        assert_eq!(parse_str("return;"), Ok(vec![Stmt::Return(None),]));
        assert_eq!(
            parse_str("return nil;"),
            Ok(vec![Stmt::Return(Some(Box::new(Expr::Nil)))])
        );
    }

    #[test]
    fn test_function_stmt() {
        assert_eq!(
            parse_str("fun test(){}"),
            Ok(vec![Stmt::Function("test".into(), vec![], vec![]),])
        );
        assert_eq!(
            parse_str("fun test(a){}"),
            Ok(vec![Stmt::Function(
                "test".into(),
                vec!["a".into()],
                vec![]
            ),])
        );
        assert_eq!(
            parse_str("fun test(){nil;}"),
            Ok(vec![Stmt::Function(
                "test".into(),
                vec![],
                vec![Stmt::Expression(Box::new(Expr::Nil,)),]
            ),])
        );
    }

    #[test]
    fn test_class_stmt() {
        assert_eq!(
            parse_str("class test{}"),
            Ok(vec![Stmt::Class("test".into(), None, vec![]),])
        );
        assert_eq!(
            parse_str("class test{a(){}}"),
            Ok(vec![Stmt::Class(
                "test".into(),
                None,
                vec![Stmt::Function("a".into(), vec![], vec![])]
            )])
        );
    }

    #[test]
    fn test_class_inheritance() {
        assert_eq!(
            parse_str("class BostonCream < Doughnut {}"),
            Ok(vec![Stmt::Class(
                "BostonCream".into(),
                Some("Doughnut".into()),
                vec![]
            ),])
        );
        assert_eq!(
            parse_str("class BostonCream < {}"),
            Err("Unexpected LeftBrace".into())
        );
        assert_eq!(
            parse_str("class BostonCream < Doughnut < BakedGood {}"),
            Err("Expected LeftBrace got Less".into())
        );
    }

    #[test]
    fn test_for() {
        fn block(what: Vec<Stmt>) -> Stmt {
            Stmt::Block(what)
        }
        fn var_i_zero() -> Stmt {
            Stmt::Var(make_span_string("i", 9), Some(Box::new(Expr::Number(0.))))
        }
        fn nil() -> Expr {
            Expr::Nil
        }
        fn while_stmt(e: Expr, s: Stmt) -> Stmt {
            Stmt::While(Box::new(e), Box::new(s))
        }

        assert_eq!(
            parse_str("for(;;){}"),
            Ok(vec![while_stmt(Expr::Boolean(true), Stmt::Block(vec![])),])
        );
        assert_eq!(
            parse_str("for(var i=0;;){}"),
            Ok(vec![block(vec![
                var_i_zero(),
                while_stmt(Expr::Boolean(true), Stmt::Block(vec![])),
            ])])
        );
        assert_eq!(
            parse_str("for(nil;nil;nil){}"),
            Ok(vec![block(vec![
                Stmt::Expression(Box::new(nil())),
                while_stmt(
                    Expr::Nil,
                    Stmt::Block(vec![Stmt::Block(vec![]), Stmt::Expression(Box::new(nil())),])
                ),
            ])])
        );
    }
}
