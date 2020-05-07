use nom::branch::alt;
use nom::bytes::complete::{escaped, is_not, tag};
use nom::character::complete::{anychar, char, digit1};
use nom::combinator::{complete, map, opt};
use nom::error::ParseError;
use nom::multi::{many0, separated_list};
use nom::sequence::{delimited, pair, tuple};
use nom::{self, error_position, Compare, IResult, InputTake};
use std::str;

#[cfg(feature = "serde")]
use serde::Deserialize;

#[derive(Debug, Clone)]
pub struct Syntax {
    pub block_start: String,
    pub block_end: String,
    pub expr_start: String,
    pub expr_end: String,
}

impl Default for Syntax {
    fn default() -> Self {
        Self {
            block_start: String::from("{%"),
            block_end: String::from("%}"),
            expr_start: String::from("{{"),
            expr_end: String::from("}}"),
        }
    }
}

#[derive(Debug, PartialEq, Clone)]
pub enum Expr<'a> {
    BoolLit(&'a str),
    NumLit(&'a str),
    StrLit(&'a str),
    CharLit(&'a str),
    Var(&'a str),
    Filter(&'a str, Vec<Expr<'a>>),
    Unary(&'a str, Box<Expr<'a>>),
    BinOp(&'a str, Box<Expr<'a>>, Box<Expr<'a>>),
}

#[derive(Debug, PartialEq, Clone)]
pub enum Target<'a> {
    Name(&'a str),
    Tuple(Vec<&'a str>),
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct WS(pub bool, pub bool);

#[derive(Debug, PartialEq, Clone)]
pub enum Node<'a> {
    Lit(&'a str, &'a str, &'a str),
    Expr(WS, Expr<'a>),
    Cond(Vec<(WS, Option<Expr<'a>>, Vec<Node<'a>>)>, WS),
    Loop(WS, Target<'a>, Expr<'a>, Vec<Node<'a>>, WS),
}

pub type Cond<'a> = (WS, Option<Expr<'a>>, Vec<Node<'a>>);

fn ws<F, I, O, E>(inner: F) -> impl Fn(I) -> IResult<I, O, E>
where
    F: Fn(I) -> IResult<I, O, E>,
    I: InputTake + Clone + PartialEq + for<'a> Compare<&'a [u8; 1]>,
    E: ParseError<I>,
{
    move |i: I| {
        let i = alt::<_, _, (), _>((tag(b" "), tag(b"\t")))(i.clone())
            .map(|(i, _)| i)
            .unwrap_or(i);
        let (i, res) = inner(i)?;
        let i = alt::<_, _, (), _>((tag(b" "), tag(b"\t")))(i.clone())
            .map(|(i, _)| i)
            .unwrap_or(i);
        Ok((i, res))
    }
}

fn split_ws_parts(s: &[u8]) -> Node {
    if s.is_empty() {
        let rs = str::from_utf8(s).unwrap();
        return Node::Lit(rs, rs, rs);
    }

    let is_ws = |c: &u8| *c != b' ' && *c != b'\t' && *c != b'\r' && *c != b'\n';
    let is_ws2 = |c: &u8| *c == b'\n';
    let start = s.iter().position(&is_ws);
    let res = if let Some(start) = start {
        let end = s.iter().rposition(&is_ws2);
        
        if let Some(end) = end {
            (&s[..start], &s[start..=end], &s[end + 1..])
        } else {
            (&s[..start], &s[start..], &s[0..0])
        }
    } else {
        (s, &s[0..0], &s[0..0])
    };

    Node::Lit(
        str::from_utf8(res.0).unwrap(),
        str::from_utf8(res.1).unwrap(),
        str::from_utf8(res.2).unwrap(),
    )
}

#[derive(Debug)]
enum ContentState {
    Start,
    Any,
    Brace(usize),
    End(usize),
}

fn take_content<'a>(i: &'a [u8], s: &'a Syntax) -> ParserError<'a, Node<'a>> {
    use crate::parser::ContentState::*;
    let bs = s.block_start.as_bytes()[0];
    let be = s.block_start.as_bytes()[1];
    let es = s.expr_start.as_bytes()[0];
    let ee = s.expr_start.as_bytes()[1];

    let mut state = Start;
    for (idx, c) in i.iter().enumerate() {
        state = match state {
            Start => {
                if *c == bs || *c == es {
                    Brace(idx)
                }
                else {
                    Any
                }
            }
            Any => {
                if *c == bs || *c == es {
                    Brace(idx)
                }
                else if *c == b'\n' {
                    End(idx+1)
                } 
                else {
                    Any
                }
            }
            Brace(start) => {
                if *c == be || *c == ee {
                    End(start)
                } 
                else {
                    Any
                }
            }
            End(_) => panic!("cannot happen"),
        };
        if let End(_) = state {
            break;
        }
    }

    match state {
        Any | Brace(_) => Ok((&i[..0], split_ws_parts(i))),
        Start | End(0) => Err(nom::Err::Error(error_position!(
            i,
            nom::error::ErrorKind::TakeUntil
        ))),
        End(start) => Ok((&i[start..], split_ws_parts(&i[..start]))),
    }
}

fn identifier(input: &[u8]) -> ParserError<&str> {
    if !nom::character::is_alphabetic(input[0]) && input[0] != b'_' && !non_ascii(input[0]) {
        return Err(nom::Err::Error(error_position!(
            input,
            nom::error::ErrorKind::AlphaNumeric
        )));
    }
    for (i, ch) in input.iter().enumerate() {
        if i == 0 || nom::character::is_alphanumeric(*ch) || *ch == b'_' || non_ascii(*ch) {
            continue;
        }
        return Ok((&input[i..], str::from_utf8(&input[..i]).unwrap()));
    }
    Ok((&input[1..], str::from_utf8(&input[..1]).unwrap()))
}

#[inline]
fn non_ascii(chr: u8) -> bool {
    chr >= 0x80 && chr <= 0xFD
}

fn expr_bool_lit(i: &[u8]) -> IResult<&[u8], Expr> {
    map(alt((tag("false"), tag("true"))), |s| {
        Expr::BoolLit(str::from_utf8(s).unwrap())
    })(i)
}

fn num_lit(i: &[u8]) -> IResult<&[u8], &str> {
    map(digit1, |s| str::from_utf8(s).unwrap())(i)
}

fn expr_num_lit(i: &[u8]) -> IResult<&[u8], Expr> {
    map(num_lit, |s| Expr::NumLit(s))(i)
}

fn str_lit(i: &[u8]) -> IResult<&[u8], &str> {
    map(
        delimited(
            char('\"'),
            opt(escaped(is_not("\\\""), '\\', anychar)),
            char('\"'),
        ),
        |s| s.map(|s| str::from_utf8(s).unwrap()).unwrap_or(""),
    )(i)
}

fn expr_str_lit(i: &[u8]) -> IResult<&[u8], Expr> {
    map(str_lit, |s| Expr::StrLit(s))(i)
}

fn char_lit(i: &[u8]) -> IResult<&[u8], &str> {
    map(
        delimited(
            char('\''),
            opt(escaped(is_not("\\\'"), '\\', anychar)),
            char('\''),
        ),
        |s| s.map(|s| str::from_utf8(s).unwrap()).unwrap_or(""),
    )(i)
}

fn expr_char_lit(i: &[u8]) -> IResult<&[u8], Expr> {
    map(char_lit, |s| Expr::CharLit(s))(i)
}

fn expr_var(i: &[u8]) -> IResult<&[u8], Expr> {
    map(identifier, |s| Expr::Var(s))(i)
}

fn target_single(i: &[u8]) -> IResult<&[u8], Target> {
    map(identifier, |s| Target::Name(s))(i)
}

fn target_tuple(i: &[u8]) -> IResult<&[u8], Target> {
    let parts = separated_list(tag(","), ws(identifier));
    let trailing = opt(ws(tag(",")));
    let full = delimited(tag("("), tuple((parts, trailing)), tag(")"));

    let (i, (elems, _)) = full(i)?;
    Ok((i, Target::Tuple(elems)))
}

fn arguments(i: &[u8]) -> IResult<&[u8], Vec<Expr>> {
    delimited(tag("("), separated_list(tag(","), ws(expr_any)), tag(")"))(i)
}

fn expr_single(i: &[u8]) -> IResult<&[u8], Expr> {
    alt((
        expr_bool_lit,
        expr_num_lit,
        expr_str_lit,
        expr_char_lit,
        expr_var,
    ))(i)
}

fn filter(i: &[u8]) -> IResult<&[u8], (&str, Option<Vec<Expr>>)> {
    let (i, (_, fname, args)) = tuple((tag("|"), identifier, opt(arguments)))(i)?;
    Ok((i, (fname, args)))
}

fn expr_filtered(i: &[u8]) -> IResult<&[u8], Expr> {
    let (i, (obj, filters)) = tuple((expr_single, many0(filter)))(i)?;

    let mut res = obj;
    for (fname, args) in filters {
        res = Expr::Filter(fname, {
            let mut args = match args {
                Some(inner) => inner,
                None => Vec::new(),
            };
            args.insert(0, res);
            args
        });
    }

    Ok((i, res))
}


fn expr_unary(i: &[u8]) -> IResult<&[u8], Expr> {
    let (i, (op, expr)) = tuple((opt(alt((tag("!"), tag("-")))), expr_filtered))(i)?;
    Ok((
        i,
        match op {
            Some(op) => Expr::Unary(str::from_utf8(op).unwrap(), Box::new(expr)),
            None => expr,
        },
    ))
}

macro_rules! expr_prec_layer {
    ( $name:ident, $inner:ident, $op:expr ) => {
        fn $name(i: &[u8]) -> IResult<&[u8], Expr> {
            let (i, (left, op_and_right)) = tuple((
                $inner,
                opt(pair(
                    ws(tag($op)),
                    expr_any,
                ))
            ))(i)?;
            Ok((i, match op_and_right {
                Some((op, right)) => Expr::BinOp(
                    str::from_utf8(op).unwrap(), Box::new(left), Box::new(right)
                ),
                None => left,
            }))
        }
    };
    ( $name:ident, $inner:ident, $( $op:expr ),+ ) => {
        fn $name(i: &[u8]) -> IResult<&[u8], Expr> {
            let (i, (left, op_and_right)) = tuple((
                $inner,
                opt(pair(
                    ws(alt(($( tag($op) ),*,))),
                    expr_any
                ))
            ))(i)?;
            Ok((i, match op_and_right {
                Some((op, right)) => Expr::BinOp(
                    str::from_utf8(op).unwrap(), Box::new(left), Box::new(right)
                ),
                None => left,
            }))
        }
    }
}

expr_prec_layer!(expr_muldivmod, expr_unary, "*", "/", "%");
expr_prec_layer!(expr_addsub, expr_muldivmod, "+", "-");
expr_prec_layer!(expr_shifts, expr_addsub, ">>", "<<");
expr_prec_layer!(expr_band, expr_shifts, "&");
expr_prec_layer!(expr_bxor, expr_band, "^");
expr_prec_layer!(expr_bor, expr_bxor, "|");
expr_prec_layer!(expr_compare, expr_bor, "==", "!=", ">=", ">", "<=", "<");
expr_prec_layer!(expr_and, expr_compare, "&&");
expr_prec_layer!(expr_or, expr_and, "||");

fn expr_any(i: &[u8]) -> IResult<&[u8], Expr> {
    Ok(expr_or(i)?)
}

fn expr_node<'a>(i: &'a [u8], s: &'a Syntax) -> IResult<&'a [u8], Node<'a>> {
    let p = tuple((
        |i| tag_expr_start(i, s),
        opt(tag("-")),
        ws(expr_any),
        opt(tag("-")),
        |i| tag_expr_end(i, s),
    ));
    let (i, (_, pws, expr, nws, _)) = p(i)?;
    Ok((i, Node::Expr(WS(pws.is_some(), nws.is_some()), expr)))
}

fn cond_if(i: &[u8]) -> IResult<&[u8], Expr> {
    let (i, (_, cond)) = tuple((ws(tag("if")), ws(expr_any)))(i)?;
    Ok((i, cond))
}

fn cond_block<'a>(i: &'a [u8], s: &'a Syntax) -> IResult<&'a [u8], Cond<'a>> {
    let p = tuple((
        |i| tag_block_start(i, s),
        opt(tag("-")),
        ws(tag("else")),
        opt(cond_if),
        opt(tag("-")),
        |i| tag_block_end(i, s),
        |i| parse_template(i, s),
    ));
    let (i, (_, pws, _, cond, nws, _, block)) = p(i)?;
    Ok((i, (WS(pws.is_some(), nws.is_some()), cond, block)))
}

fn block_if<'a>(i: &'a [u8], s: &'a Syntax) -> IResult<&'a [u8], Node<'a>> {
    let p = tuple((
        opt(tag("-")),
        cond_if,
        opt(tag("-")),
        |i| tag_block_end(i, s),
        |i| parse_template(i, s),
        many0(|i| cond_block(i, s)),
        |i| tag_block_start(i, s),
        opt(tag("-")),
        ws(tag("endif")),
        opt(tag("-")),
    ));
    let (i, (pws1, cond, nws1, _, block, elifs, _, pws2, _, nws2)) = p(i)?;

    let mut res = Vec::new();
    res.push((WS(pws1.is_some(), nws1.is_some()), Some(cond), block));
    res.extend(elifs);
    Ok((i, Node::Cond(res, WS(pws2.is_some(), nws2.is_some()))))
}

fn block_for<'a>(i: &'a [u8], s: &'a Syntax) -> IResult<&'a [u8], Node<'a>> {
    let p = tuple((
        opt(tag("-")),
        ws(tag("for")),
        ws(alt((target_single, target_tuple))),
        ws(tag("in")),
        ws(expr_any),
        opt(tag("-")),
        |i| tag_block_end(i, s),
        |i| parse_template(i, s),
        |i| tag_block_start(i, s),
        opt(tag("-")),
        ws(tag("endfor")),
        opt(tag("-")),
    ));
    let (i, (pws1, _, var, _, iter, nws1, _, block, _, pws2, _, nws2)) = p(i)?;
    Ok((
        i,
        Node::Loop(
            WS(pws1.is_some(), nws1.is_some()),
            var,
            iter,
            block,
            WS(pws2.is_some(), nws2.is_some()),
        ),
    ))
}

fn block_node<'a>(i: &'a [u8], s: &'a Syntax) -> IResult<&'a [u8], Node<'a>> {
    let p = tuple((
        |i| tag_block_start(i, s),
        alt((
            |i| block_if(i, s),
            |i| block_for(i, s),
        )),
        |i| tag_block_end(i, s),
    ));
    let (i, (_, contents, _)) = p(i)?;
    Ok((i, contents))
}

pub fn parse_template<'a>(i: &'a [u8], s: &'a Syntax) -> IResult<&'a [u8], Vec<Node<'a>>> {
    many0(alt((
        complete(|i| take_content(i, s)),
        complete(|i| expr_node(i, s)),
        complete(|i| block_node(i, s)),
    )))(i)
}

fn tag_block_start<'a>(i: &'a [u8], s: &'a Syntax) -> IResult<&'a [u8], &'a [u8]> {
    tag(s.block_start.as_str())(i)
}
fn tag_block_end<'a>(i: &'a [u8], s: &'a Syntax) -> IResult<&'a [u8], &'a [u8]> {
    tag(s.block_end.as_str())(i)
}
fn tag_expr_start<'a>(i: &'a [u8], s: &'a Syntax) -> IResult<&'a [u8], &'a [u8]> {
    tag(s.expr_start.as_str())(i)
}
fn tag_expr_end<'a>(i: &'a [u8], s: &'a Syntax) -> IResult<&'a [u8], &'a [u8]> {
    tag(s.expr_end.as_str())(i)
}

#[cfg(test)]
mod tests {
    use crate::parser::Syntax;

    fn check_ws_split(s: &str, res: &(&str, &str, &str)) {
        let node = super::split_ws_parts(s.as_bytes());
        match node {
            super::Node::Lit(lws, s, rws) => {
                assert_eq!(lws, res.0);
                assert_eq!(s, res.1);
                assert_eq!(rws, res.2);
            }
            _ => {
                panic!("fail");
            }
        }
    }

    #[test]
    fn test_ws_splitter() {
        check_ws_split("", &("", "", ""));
        check_ws_split("a", &("", "a", ""));
        check_ws_split("\ta", &("\t", "a", ""));
        check_ws_split("b\n", &("", "b", "\n"));
        check_ws_split(" \t\r\n", &(" \t\r\n", "", ""));
    }
}

type ParserError<'a, T> = Result<(&'a [u8], T), nom::Err<(&'a [u8], nom::error::ErrorKind)>>;