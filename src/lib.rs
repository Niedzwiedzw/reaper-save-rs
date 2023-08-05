use nom::{
    branch::alt,
    bytes::complete::{take_while, take_while1, take_while_m_n},
    character::complete::alpha0,
    combinator::{opt, peek},
    error::VerboseError,
    multi::{many0, separated_list0, separated_list1},
    number,
    sequence::{delimited, separated_pair, tuple},
    IResult, Parser,
};
use nom_supreme::{
    error::ErrorTree,
    tag::{self, complete::tag},
    ParserExt,
};
use ordered_float::OrderedFloat;
use std::{any::type_name, fmt::Write, iter::once};
use tracing::instrument;

const INDENT_SPACES: usize = 2;

macro_rules! location {
    () => {
        concat!(file!(), ":", line!())
    };
}

pub mod error {
    use std::num::ParseFloatError;

    use thiserror::Error;
    #[derive(Debug, Error)]
    pub enum Error {
        #[error("Writing value failed")]
        WriteError {
            source: std::fmt::Error,
            value: String,
        },
        #[error("Invalid float")]
        ParseFloatError {
            #[from]
            source: ParseFloatError,
        },
    }
    pub type Result<T> = std::result::Result<T, Error>;
}
type Input<'input> = &'input str;
type Output<'output> = &'output mut String;
type Res<'input, U> = IResult<Input<'input>, U, ErrorTree<Input<'input>>>;

type Float = OrderedFloat<f32>;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ReaperUid(String);

macro_rules! write_error {
    ($self:expr, $source:expr) => {
        error::Error::WriteError {
            source: $source,
            value: format!("{:?}", $self),
        }
    };
}

impl SerializeAndDeserialize for ReaperUid {
    fn serialize<'out>(&self, out: Output<'out>, _: usize) -> error::Result<Output<'out>> {
        write!(out, "{{{}}}", self.0)
            .map_err(|source| write_error!(self, source))
            .map(|_| out)
    }
    #[instrument(fields(location=location!(), this=type_name::<Self>(), input=input.chars().take(20).collect::<String>()), ret, err, level = "TRACE")]
    fn deserialize(input: Input, indent: usize) -> Res<Self> {
        delimited(
            tag("{"),
            take_while(|c: char| c.is_digit(16) || c == '-'),
            tag("}"),
        )
        .map(|v: Input| ReaperUid(v.to_owned()))
        .context(type_name::<Self>())
        .parse(input)
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Attribute {
    ReaperUid(ReaperUid),
    Int(i64),
    String(String),
    Float(Float),
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AnonymousParameter(String);

impl SerializeAndDeserialize for AnonymousParameter {
    fn serialize<'out>(&self, out: Output<'out>, indent: usize) -> error::Result<Output<'out>> {
        write_indent(out, indent)?;
        write!(out, "{}", self.0);
        Ok(out)
    }

    #[instrument(fields(location=location!(), this=type_name::<Self>(), input=input.chars().take(20).collect::<String>()), ret, err, level = "TRACE")]
    fn deserialize(input: Input, indent: usize) -> Res<Self> {
        take_while1(|c: char| c.is_alphanumeric() || c == '=')
            .map(|v: Input| Self(v.to_owned()))
            .preceded_by(|input| parse_indents(input, indent))
            // .terminated(peek(parse_newline))
            .context(type_name::<Self>())
            .parse(input)
    }
}

fn parse_space(input: Input) -> Res<Input> {
    take_while_m_n(1, 1, |c| c == ' ')
        .context("checking whitespace delimiter")
        .parse(input)
}

fn parse_indents(input: Input, indents: usize) -> Res<Input> {
    let spaces = indents * INDENT_SPACES;
    take_while_m_n(spaces, spaces, |c| c == ' ')
        .context("checking indentation")
        .parse(input)
}

fn parse_newline(input: Input) -> Res<Input> {
    tag("\r\n").context("parsing newline").parse(input)
}

// #[instrument(fields(input=input.chars().take(20).collect::<String>()), ret, err, level = "TRACE")]
// fn parse_newline_and_opt_whitespace(input: Input) -> Res<()> {
//     parse_newlines
//         .terminated(opt(parse_spaces))
//         .map(|_| ())
//         .parse(input)
// }

#[instrument(fields(input=input.chars().take(20).collect::<String>()), ret, err, level = "TRACE")]
fn parse_string(input: Input) -> Res<String> {
    delimited(tag("\""), take_while(|c: char| c != '"'), tag("\""))
        .map(|v: Input| v.to_owned())
        .context("reading string")
        .parse(input)
}

fn parse_float(input: Input) -> Res<Float> {
    take_while(|v: char| !v.is_whitespace())
        .map_res(|v: Input| v.parse::<f32>())
        .map(OrderedFloat)
        .context("reading float")
        .parse(input)
    // .map_err(Into::into).map(OrderedFloat)
}

fn parse_int(input: Input) -> Res<i64> {
    take_while(|v: char| !v.is_whitespace())
        .map_res(|v: Input| v.parse::<i64>())
        .context("reading integer")
        .parse(input)
}

impl SerializeAndDeserialize for Attribute {
    fn serialize<'out>(&self, out: Output<'out>, _: usize) -> error::Result<Output<'out>> {
        match self {
            Attribute::ReaperUid(v) => return v.serialize(out, 0),
            Attribute::String(v) => write!(out, r#""{v}""#),
            Attribute::Float(v) => write!(out, "{v}"),
            Attribute::Int(v) => write!(out, "{v}"),
        }
        .map_err(|source| write_error!(self, source))
        .map(|_| out)
    }

    #[instrument(fields(location=location!(), this=type_name::<Self>(), input=input.chars().take(20).collect::<String>()), ret, err, level = "TRACE")]
    fn deserialize(input: Input, _: usize) -> Res<Self> {
        alt((
            |v| ReaperUid::deserialize(v, 0).map(|(out, v)| (out, Self::ReaperUid(v))),
            parse_string.map(Self::String),
            parse_int.map(Self::Int),
            parse_float.map(Self::Float),
        ))
        .context(type_name::<Self>())
        .parse(input)
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct AttributeName(String);

impl SerializeAndDeserialize for AttributeName {
    fn serialize<'out>(&self, out: Output<'out>, _indent: usize) -> error::Result<Output<'out>> {
        write!(out, "{}", self.0)
            .map_err(|source| write_error!(self, source))
            .map(|_| out)
    }

    #[instrument(fields(location=location!(), this=type_name::<Self>(), input=input.chars().take(20).collect::<String>()), ret, err, level = "TRACE")]
    fn deserialize(input: Input, indent: usize) -> Res<Self> {
        take_while1(|c: char| (c.is_alphabetic() && c.is_uppercase()) || c == '_')
            .map(|v: Input| AttributeName(v.to_owned()))
            .context(type_name::<Self>())
            .parse(input)
    }
}

fn to_indent(indent: usize) -> String {
    let spaces = INDENT_SPACES * indent;
    (0..spaces).map(|_| "  ").collect::<Vec<_>>().join("")
}

fn write_indent(out: Output, indent: usize) -> error::Result<Output> {
    let indent = to_indent(indent);
    write!(out, "{indent}").map_err(|source| write_error!(indent, source))?;
    Ok(out)
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Line {
    attribute: AttributeName,
    values: Vec<Attribute>,
}

impl SerializeAndDeserialize for Line {
    fn serialize<'out>(&self, out: Output<'out>, indent: usize) -> error::Result<Output<'out>> {
        write_indent(out, indent)?;
        once(self.attribute.serialize_inline())
            .chain(self.values.iter().map(|v| v.serialize_inline()))
            .collect::<error::Result<Vec<_>>>()
            .map(|segments| segments.join(" "))
            .and_then(|line| write!(out, "{line}").map_err(|source| write_error!(self, source)))
            .map(|()| out)
    }

    #[instrument(fields(location=location!(), this=type_name::<Self>(), input=input.chars().take(20).collect::<String>()), ret, err, level = "TRACE")]
    fn deserialize(input: Input, indent: usize) -> Res<Self> {
        tuple((
            (|input| AttributeName::deserialize(input, 0)),
            opt(
                separated_list1(parse_space, move |input| Attribute::deserialize(input, 0))
                    .preceded_by(parse_space),
            ),
        ))
        .preceded_by(|input| parse_indents(input, indent))
        .context(type_name::<Self>())
        .context("making sure line ends with newline")
        .map(|(attribute, values)| Self {
            attribute,
            values: values.unwrap_or_default(),
        })
        .parse(input)
    }
}

// impl SerializeAndDeserialize for ObjectHeader {
//     fn serialize<'out>(&self, out: Output<'out>, indent: usize) -> error::Result<Output<'out>> {
//         write!(out, "<")
//             .map_err(|source| write_error!(self, source))
//             .and_then(|_| self.0.serialize(out, indent))?;
//         Ok(out)
//     }

//     fn deserialize(input: Input) -> Res<Self> {

//     }
// }

// #[derive(Debug, Clone, PartialEq, Eq)]
// pub enum ObjectValues {
//     Entries(Vec<Entry>),
// }

// impl SerializeAndDeserialize for ObjectValues {
//     fn serialize<'out>(&self, out: Output<'out>, indent: usize) -> error::Result<Output<'out>> {
//         match self {
//             ObjectValues::Entries(entries) => {
//                 for entry in entries.iter() {
//                     entry.serialize(out, indent + 1)?;
//                 }
//             }
//         }
//         writeln!(out, "");
//         Ok(out)
//     }

//     #[instrument(fields(location=location!(), this=type_name::<Self>(), input=input.chars().take(20).collect::<String>()), ret, err, level = "TRACE")]
//     fn deserialize(input: Input, indent: usize) -> Res<Self> {
//         separated_list0(
//             parse_newline,
//             (|input| Entry::deserialize(input, indent))
//                 .preceded_by(|input| parse_indents(input, indent)),
//         )
//         .context("separated list of entries")
//         .map(Self::Entries)
//         .context(type_name::<Self>())
//         .parse(input)
//     }
// }

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Object {
    header: Line,
    values: Vec<Entry>,
}

impl SerializeAndDeserialize for Object {
    fn serialize<'out>(&self, out: Output<'out>, indent: usize) -> error::Result<Output<'out>> {
        write_indent(out, indent)?;
        write!(out, "<");
        self.header.serialize(out, 0)?;
        writeln!(out, "");
        for entry in self.values.iter() {
            entry.serialize(out, indent + 1)?;
            writeln!(out, "");
        }
        write_indent(out, indent)?;
        write!(out, ">");
        writeln!(out);
        Ok(out)
    }

    #[instrument(fields(location=location!(), this=type_name::<Self>(), input=input.chars().take(20).collect::<String>()), ret, err, level = "TRACE")]
    fn deserialize(input: Input, indent: usize) -> Res<Self> {
        let object_initializer = tag("<")
            .preceded_by(|input| parse_indents(input, indent))
            .context("object initializer");

        let object_finalizer = (|input| parse_indents(input, indent + 1))
            .precedes(tag(">"))
            .context("object terminator");
        let header = (|input| Line::deserialize(input, 0)).context("parsing header");
        let entry_line = (|input| (Entry::deserialize(input, indent + 1)))
            .context("making sure Entry ends with a newline");
        let entries = many0(entry_line).context("parsing entries of object");

        let object_body =
            tuple((header.terminated(parse_newline), entries)).context("parsing object body");

        delimited(object_initializer, object_body, object_finalizer)
            .map(|(header, values)| Self { header, values })
            .context(type_name::<Self>())
            .parse(input)
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Entry {
    Object(Object),
    Line(Line),
    AnonymousParameter(AnonymousParameter),
}

impl SerializeAndDeserialize for Entry {
    fn serialize<'out>(&self, out: Output<'out>, indent: usize) -> error::Result<Output<'out>> {
        match self {
            Entry::Object(object) => object.serialize(out, indent),
            Entry::Line(line) => line.serialize(out, indent),
            Entry::AnonymousParameter(param) => param.serialize(out, indent),
        }
    }

    #[instrument(fields(location=location!(), this=type_name::<Self>(), input=input.chars().take(20).collect::<String>()), ret, err, level = "TRACE")]
    fn deserialize(input: Input, indent: usize) -> Res<Self> {
        alt((
            (|input| Object::deserialize(input, indent))
                .map(Self::Object)
                .terminated(parse_newline)
                .context("parsing object entry"),
            (|input| Line::deserialize(input, indent))
                .map(Self::Line)
                .terminated(parse_newline)
                .context("parsing line entry"),
            (|input| AnonymousParameter::deserialize(input, indent))
                .map(Self::AnonymousParameter)
                .terminated(parse_newline)
                .context("parsing anonymous parameter entry"),
        ))
        .context(type_name::<Self>())
        .context("entries must end in newline")
        .parse(input)
    }
}

pub trait SerializeAndDeserialize: Sized {
    fn serialize<'out>(&self, out: Output<'out>, indent: usize) -> error::Result<Output<'out>>;
    fn deserialize(input: Input, indent: usize) -> Res<Self>;
    fn serialize_inline(&self) -> error::Result<String> {
        let mut out = String::new();
        self.serialize(&mut out, 0)?;
        Ok(out)
    }
}

#[cfg(test)]
mod tests {
    use eyre::{eyre, Result, WrapErr};
    use std::error::Error;
    use test_log::test;

    use super::*;
    const EXAMPLE_1: &str = include_str!("../test_data/barbarah-anne.rpp");

    #[test]
    fn test_single_param_object() -> Result<()> {
        let example = "<METRONOME 6 2\r\n  VOL 0.25 0.125\r\n  >";
        Object::deserialize(example, 0).map_err(|e| eyre!("{e:#?}"))?;

        Ok(())
    }

    #[test]
    fn test_two_param_object() -> Result<()> {
        let example = "<METRONOME 6 2\r\n  VOL 0.25 0.125\r\n  FREQ 800 1600 1\r\n  >";
        Object::deserialize(example, 0).map_err(|e| eyre!("{e:#?}"))?;

        Ok(())
    }
    #[test]
    fn test_bigger_object() -> Result<()> {
        let example = "<METRONOME 6 2\r\n  VOL 0.25 0.125\r\n  FREQ 800 1600 1\r\n  BEATLEN 4\r\n  SAMPLES \"\" \"\"\r\n  PATTERN 2863311530 2863311529\r\n  MULT 1\r\n  >";
        Object::deserialize(example, 0).map_err(|e| eyre!("{e:#?}"))?;

        Ok(())
    }

    #[test]
    fn test_line() -> Result<()> {
        Line::deserialize("GROUPOVERRIDE 0 0 0", 0).map_err(|e| eyre!("{e:#?}"))?;
        Ok(())
    }
    #[test]
    fn test_entry() -> Result<()> {
        Entry::deserialize("GROUPOVERRIDE 0 0 0\r\n", 0).map_err(|e| eyre!("{e:#?}"))?;
        Ok(())
    }
    #[test]
    fn test_empty_object() -> Result<()> {
        Object::deserialize("<EMPTY\r\n  >", 0)
            .map_err(|e| eyre!("{e:#?}"))
            .map_err(|e| eyre!("{e:#?}"))?;
        Ok(())
    }

    #[test]
    fn test_string() -> Result<()> {
        let input = "<REAPER_PROJECT 0.1 \"6.80/linux-x86_64\" 1691227194\r\n  >";
        Object::deserialize(input, 0).map_err(|e| eyre!("{e:#?}"))?;
        Ok(())
    }

    #[test]
    fn test_notes() -> Result<()> {
        Object::deserialize("<NOTES 0 2\r\n  >", 0).map_err(|e| eyre!("{e:#?}"))?;
        Ok(())
    }
    #[test]
    fn test_hash_anonymous_parameter() -> Result<()> {
        AnonymousParameter::deserialize("ZXZhdxgAAQ==", 0).map_err(|e| eyre!("{e:#?}"))?;
        Ok(())
    }
    #[test]
    fn test_record_cfg() -> Result<()> {
        Object::deserialize("<RENDER_CFG\r\n  ZXZhdxgAAQ==\r\n  >", 0)
            .map_err(|e| eyre!("{e:#?}"))?;
        Ok(())
    }

    #[test]
    fn test_example_document_parses() -> Result<()> {
        let (_, object) = Object::deserialize(EXAMPLE_1, 0).map_err(|e| eyre!("{e:#?}"))?;
        println!("{object:#?}");
        Ok(())
    }
}
