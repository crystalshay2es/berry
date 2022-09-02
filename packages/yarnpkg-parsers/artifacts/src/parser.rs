use nom::{
  branch::alt,
  bytes::complete::{escaped_transform, is_not, take_while_m_n},
  character::complete::{char, line_ending, not_line_ending, space0, space1},
  combinator::{map, map_opt, map_res, opt, recognize, value},
  error::VerboseError,
  multi::{count, fold_many0, fold_many1, many0_count},
  sequence::{delimited, preceded, separated_pair, terminated},
  AsChar, IResult,
};
use serde_json::{json, Value};

// TODO:: Automatically detect indentation from input.
const INDENT_STEP: usize = 2;

pub type ParseResult<'input, O> = IResult<&'input str, O, VerboseError<&'input str>>;

pub fn parse(input: &str) -> ParseResult<Value> {
  property_statements(input, 0)
}

fn property_statements(input: &str, indent: usize) -> ParseResult<Value> {
  fold_many0(
    alt((map(comment, |_| Default::default()), |input| {
      property_statement(input, indent)
    })),
    || json!({}),
    |mut acc, (key, value)| {
      if !key.is_null() {
        // TODO: handle duplicates
        // TODO: propagate the error
        acc[key.as_str().unwrap()] = value;
      }
      acc
    },
  )(input)
}

fn property_statement(input: &str, indent: usize) -> ParseResult<(Value, Value)> {
  preceded(
    |input| indentation(input, indent),
    separated_pair(scalar, delimited(space0, char(':'), space0), |input| {
      expression(input, indent)
    }),
  )(input)
}

fn comment(input: &str) -> ParseResult<Option<&str>> {
  delimited(space0, opt(preceded(char('#'), not_line_ending)), eol_any)(input)
}

fn item_statements(input: &str, indent: usize) -> ParseResult<Value> {
  map(
    fold_many1(
      |input| item_statement(input, indent),
      Vec::new,
      |mut acc, value| {
        acc.push(value);
        acc
      },
    ),
    Value::Array,
  )(input)
}

fn item_statement(input: &str, indent: usize) -> ParseResult<Value> {
  preceded(
    |input| indentation(input, indent),
    preceded(terminated(char('-'), space1), |input| {
      expression(input, indent)
    }),
  )(input)
}

fn expression(input: &str, indent: usize) -> ParseResult<Value> {
  alt((
    preceded(line_ending, |input| {
      item_statements(input, indent + INDENT_STEP)
    }),
    preceded(line_ending, |input| {
      property_statements(input, indent + INDENT_STEP)
    }),
    terminated(scalar, eol_any),
  ))(input)
}

fn indentation(input: &str, indent: usize) -> ParseResult<Vec<char>> {
  count(char(' '), indent)(input)
}

fn scalar(input: &str) -> ParseResult<Value> {
  map(alt((double_quoted_scalar, plain_scalar)), Value::String)(input)
}

fn double_quoted_scalar(input: &str) -> ParseResult<String> {
  delimited(char('"'), double_quoted_scalar_text, char('"'))(input)
}

fn double_quoted_scalar_text(input: &str) -> ParseResult<String> {
  escaped_transform(
    // TODO: "\0-\x1F" was part of the original regexp
    is_not("\"\\\x7f"),
    '\\',
    alt((
      value('"', char('"')),
      value('\\', char('\\')),
      value('/', char('/')),
      value('\n', char('n')),
      value('\r', char('r')),
      value('\t', char('t')),
      // Rust doesn't support the following ascii escape sequences in string literals.
      value('\x08', char('b')),
      value('\x0c', char('f')),
      // Unicode escape sequences
      preceded(char('u'), unicode_escape_digits),
    )),
  )(input)
}

fn unicode_escape_digits(input: &str) -> ParseResult<char> {
  map_opt(
    map_res(take_while_m_n(4, 4, |ch: char| ch.is_hex_digit()), |hex| {
      u32::from_str_radix(hex, 16)
    }),
    char::from_u32,
  )(input)
}

fn plain_scalar(input: &str) -> ParseResult<String> {
  map(
    recognize(preceded(
      is_not("\r\n\t ?:,][{}#&*!|>'\"%@`-"),
      many0_count(preceded(space0, is_not("\r\n\t ,][{}:#\"'"))),
    )),
    |scalar: &str| scalar.to_owned(),
  )(input)
}

fn eol_any(input: &str) -> ParseResult<&str> {
  terminated(line_ending, many0_count(preceded(space0, line_ending)))(input)
}