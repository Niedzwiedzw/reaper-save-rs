use nom::{
    branch::alt,
    bytes::complete::{take_while, take_while1, take_while_m_n},
    combinator::opt,
    multi::{many0, separated_list1},
    sequence::{delimited, tuple},
    IResult, Parser,
};
use nom_supreme::{error::ErrorTree, tag::complete::tag, ParserExt};
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

    use thiserror::Error;
    #[derive(Debug, Error)]
    pub enum Error {
        #[error("Writing value failed")]
        WriteError {
            #[from]
            source: std::fmt::Error,
        },
        #[error("Writing whitespace failed")]
        WriteWhitespaceError,
        #[error("Failed to parse:\n{report}")]
        ParseError { report: String },
    }
    pub type Result<T> = std::result::Result<T, Error>;
}
type Input<'input> = &'input str;
type Output<'output> = &'output mut String;
type Res<'input, U> = IResult<Input<'input>, U, ErrorTree<Input<'input>>>;

type Float = OrderedFloat<f64>;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ReaperUid(pub String);

impl SerializeAndDeserialize for ReaperUid {
    fn serialize<'out>(&self, out: Output<'out>, _: usize) -> error::Result<Output<'out>> {
        write!(out, "{{{}}}", self.0)
            .map_err(Into::into)
            .map(|_| out)
    }
    #[instrument(fields(location=location!(), this=type_name::<Self>(), input=input.chars().take(20).collect::<String>()), ret, level = "TRACE")]
    fn deserialize(input: Input, indent: usize) -> Res<Self> {
        delimited(
            tag("{"),
            take_while(|c: char| c.is_ascii_hexdigit() || c == '-'),
            tag("}"),
        )
        .map(|v: Input| ReaperUid(v.to_owned()))
        .context(type_name::<Self>())
        .parse(input)
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Int(pub i64);

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum EscapedString {
    SingleQuote(String),
    DoubleQuote(String),
}

impl SerializeAndDeserialize for EscapedString {
    fn serialize<'out>(&self, out: Output<'out>, _: usize) -> error::Result<Output<'out>> {
        match self {
            EscapedString::SingleQuote(v) => write!(out, "'{v}'"),
            EscapedString::DoubleQuote(v) => write!(out, "\"{v}\""),
        }
        .map_err(Into::into)
        .map(|_| out)
    }

    fn deserialize(input: Input, _indent: usize) -> Res<Self> {
        let contents = |quote: char| take_while(move |c: char| c != quote);
        let quote = |quote: &'static str| {
            delimited(
                tag(quote),
                contents(
                    quote
                        .chars()
                        .next()
                        .expect("programming error, quotes must be at least one char"),
                ),
                tag(quote),
            )
        };
        alt((
            quote("\"")
                .map(|v: Input| v.to_owned())
                .map(Self::DoubleQuote),
            quote("'")
                .map(|v: Input| v.to_owned())
                .map(Self::SingleQuote),
        ))
        .context("reading string")
        .parse(input)
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Attribute {
    ReaperUid(ReaperUid),
    Int(Int),
    String(EscapedString),
    Float(Float),
    UnescapedString(UnescapedString),
    UNumber(Int),
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AnonymousParameter(pub String);

impl SerializeAndDeserialize for AnonymousParameter {
    fn serialize<'out>(&self, out: Output<'out>, indent: usize) -> error::Result<Output<'out>> {
        write_indent(out, indent)?;
        write!(out, "{}", self.0)?;
        Ok(out)
    }

    #[instrument(fields(location=location!(), this=type_name::<Self>(), input=input.chars().take(20).collect::<String>()), ret, level = "TRACE")]
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
    tag("\r\n")
        .or(tag("\n"))
        .context("parsing newline")
        .parse(input)
}

// #[instrument(fields(input=input.chars().take(20).collect::<String>()), ret, level = "TRACE")]
// fn parse_newline_and_opt_whitespace(input: Input) -> Res<()> {
//     parse_newlines
//         .terminated(opt(parse_spaces))
//         .map(|_| ())
//         .parse(input)
// }

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct UnescapedString(pub String);

#[instrument(fields(input=input.chars().take(20).collect::<String>()), ret, level = "TRACE")]
fn parse_unescaped_string(input: Input) -> Res<UnescapedString> {
    take_while(|c: char| !c.is_whitespace())
        // .terminated(peek(parse_newline))
        .map(|v: Input| UnescapedString(v.to_owned()))
        .context("reading string")
        .parse(input)
}

fn parse_float(input: Input) -> Res<Float> {
    take_while(|v: char| !v.is_whitespace())
        .map_res(|v: Input| v.parse::<f64>())
        .map(OrderedFloat)
        .context("reading float")
        .parse(input)
    // .map_err(Into::into).map(OrderedFloat)
}

fn parse_int(input: Input) -> Res<Int> {
    take_while(|v: char| !v.is_whitespace())
        .map_res(|v: Input| v.parse::<i64>().map(Int))
        .context("reading integer")
        .parse(input)
}

fn parse_u_number(input: Input) -> Res<Int> {
    take_while(|v: char| v == '-' || v.is_numeric())
        .terminated(tag(":U"))
        .map_res(|v: Input| v.parse::<i64>().map(Int))
        .context("reading integer")
        .parse(input)
}

impl SerializeAndDeserialize for Attribute {
    fn serialize<'out>(&self, out: Output<'out>, _: usize) -> error::Result<Output<'out>> {
        match self {
            Attribute::ReaperUid(v) => return v.serialize(out, 0),
            Attribute::String(v) => return v.serialize(out, 0),
            Attribute::Float(v) => write!(out, "{v}"),
            Attribute::Int(Int(v)) => write!(out, "{}", v),
            Attribute::UnescapedString(UnescapedString(v)) => write!(out, r#"{v}"#),
            Attribute::UNumber(Int(v)) => write!(out, "{}:U", v),
        }
        .map_err(Into::into)
        .map(|_| out)
    }

    #[instrument(fields(location=location!(), this=type_name::<Self>(), input=input.chars().take(20).collect::<String>()), ret, level = "TRACE")]
    fn deserialize(input: Input, _: usize) -> Res<Self> {
        alt((
            |v| ReaperUid::deserialize(v, 0).map(|(out, v)| (out, Self::ReaperUid(v))),
            |v| EscapedString::deserialize(v, 0).map(|(out, v)| (out, Self::String(v))),
            parse_int.map(Self::Int),
            parse_float.map(Self::Float),
            parse_u_number.map(Self::UNumber),
            parse_unescaped_string.map(Self::UnescapedString),
        ))
        .context(type_name::<Self>())
        .parse(input)
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AttributeName(pub String);

impl SerializeAndDeserialize for AttributeName {
    fn serialize<'out>(&self, out: Output<'out>, _indent: usize) -> error::Result<Output<'out>> {
        write!(out, "{}", self.0).map_err(Into::into).map(|_| out)
    }

    #[instrument(fields(location=location!(), this=type_name::<Self>(), input=input.chars().take(20).collect::<String>()), ret, level = "TRACE")]
    fn deserialize(input: Input, indent: usize) -> Res<Self> {
        take_while1(|c: char| (c.is_alphabetic() && c.is_uppercase()) || c.is_numeric() || c == '_')
            // .preceded_by(
            //     peek(take_while_m_n(1, 1, |c: char| {
            //         c.is_alphabetic() && c.is_uppercase()
            //     }))
            //     .context("making sure first character is alphabetic"),
            // )
            .map(|v: Input| AttributeName(v.to_owned()))
            .context(type_name::<Self>())
            .parse(input)
    }
}

fn to_indent(indent: usize) -> String {
    let spaces = INDENT_SPACES * indent;
    (0..spaces).map(|_| " ").collect::<Vec<_>>().join("")
}

fn write_indent(out: Output, indent: usize) -> error::Result<Output> {
    let indent = to_indent(indent);
    write!(out, "{indent}")?;
    Ok(out)
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Line {
    pub attribute: AttributeName,
    pub values: Vec<Attribute>,
}

impl SerializeAndDeserialize for Line {
    fn serialize<'out>(&self, out: Output<'out>, indent: usize) -> error::Result<Output<'out>> {
        write_indent(out, indent)?;
        once(self.attribute.serialize_inline())
            .chain(self.values.iter().map(|v| v.serialize_inline()))
            .collect::<error::Result<Vec<_>>>()
            .map(|segments| segments.join(" "))
            .and_then(|line| write!(out, "{line}").map_err(Into::into))
            .map(|()| out)
    }

    #[instrument(fields(location=location!(), this=type_name::<Self>(), input=input.chars().take(20).collect::<String>()), ret, level = "TRACE")]
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

//     #[instrument(fields(location=location!(), this=type_name::<Self>(), input=input.chars().take(20).collect::<String>()), ret, level = "TRACE")]
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
    pub header: Line,
    pub values: Vec<Entry>,
}

impl SerializeAndDeserialize for Object {
    fn serialize<'out>(&self, out: Output<'out>, indent: usize) -> error::Result<Output<'out>> {
        write_indent(out, indent)?;
        write!(out, "<")?;
        self.header.serialize(out, 0)?;
        writeln!(out)?;
        for entry in self.values.iter() {
            entry.serialize(out, indent + 1)?;
            writeln!(out)?;
        }
        write_indent(out, indent)?;
        write!(out, ">")?;
        Ok(out)
    }

    #[instrument(fields(location=location!(), this=type_name::<Self>(), input=input.chars().take(20).collect::<String>()), ret, level = "TRACE")]
    fn deserialize(input: Input, indent: usize) -> Res<Self> {
        let object_initializer = tag("<")
            .preceded_by(|input| parse_indents(input, indent))
            .context("object initializer");

        let object_finalizer = (|input| parse_indents(input, indent))
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

    #[instrument(fields(location=location!(), this=type_name::<Self>(), input=input.chars().take(20).collect::<String>()), ret, level = "TRACE")]
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

pub fn to_string(save_file: Object) -> error::Result<String> {
    save_file
        .serialize_inline()
        .map(|v| [v.as_str(), "\r\n"].join(""))
}

pub fn from_str(input: &str) -> error::Result<Object> {
    Object::deserialize(input, 0)
        .map_err(|report| error::Error::ParseError {
            report: format!("{report:#?}"),
        })
        .map(|(_, object)| object)
}

#[cfg(test)]
mod tests {
    use eyre::{eyre, Result};
    use pretty_assertions::assert_eq;

    use test_log::test;

    use super::*;
    const EXAMPLE_1: &str = include_str!("../test_data/barbarah-anne.rpp");

    #[test]
    fn test_single_param_object() -> Result<()> {
        let example = "<METRONOME 6 2\r\n  VOL 0.25 0.125\r\n>";
        Object::deserialize(example, 0).map_err(|e| eyre!("{e:#?}"))?;

        Ok(())
    }

    #[test]
    fn test_two_param_object() -> Result<()> {
        let example = "<METRONOME 6 2\r\n  VOL 0.25 0.125\r\n  FREQ 800 1600 1\r\n>";
        Object::deserialize(example, 0).map_err(|e| eyre!("{e:#?}"))?;

        Ok(())
    }
    #[test]
    fn test_bigger_object() -> Result<()> {
        let example = "<METRONOME 6 2\r\n  VOL 0.25 0.125\r\n  FREQ 800 1600 1\r\n  BEATLEN 4\r\n  SAMPLES \"\" \"\"\r\n  PATTERN 2863311530 2863311529\r\n  MULT 1\r\n>";
        Object::deserialize(example, 0).map_err(|e| eyre!("{e:#?}"))?;

        Ok(())
    }

    #[test]
    fn test_line() -> Result<()> {
        let (empty, _) =
            Line::deserialize("GROUPOVERRIDE 0 0 0", 0).map_err(|e| eyre!("{e:#?}"))?;
        assert_eq!(empty, "");
        Ok(())
    }

    #[test]
    fn test_parse_auxrecv() -> Result<()> {
        let (out, _) = Line::deserialize("AUXRECV 0 0 1 0 0 0 0 0 0 -1:U 0 -1 ''", 0)
            .map_err(|e| eyre!("{e:#?}"))?;

        assert_eq!(out, "");

        Ok(())
    }
    #[test]
    fn test_entry() -> Result<()> {
        Entry::deserialize("GROUPOVERRIDE 0 0 0\r\n", 0).map_err(|e| eyre!("{e:#?}"))?;
        Ok(())
    }
    #[test]
    fn test_empty_object() -> Result<()> {
        Object::deserialize("<EMPTY\r\n>", 0)
            .map_err(|e| eyre!("{e:#?}"))
            .map_err(|e| eyre!("{e:#?}"))?;
        Ok(())
    }

    #[test]
    fn test_string() -> Result<()> {
        let input = "<REAPER_PROJECT 0.1 \"6.80/linux-x86_64\" 1691227194\r\n>";
        Object::deserialize(input, 0).map_err(|e| eyre!("{e:#?}"))?;
        Ok(())
    }

    #[test]
    fn test_notes() -> Result<()> {
        Object::deserialize("<NOTES 0 2\r\n>", 0).map_err(|e| eyre!("{e:#?}"))?;
        Ok(())
    }
    #[test]
    fn test_hash_anonymous_parameter() -> Result<()> {
        AnonymousParameter::deserialize("ZXZhdxgAAQ==", 0).map_err(|e| eyre!("{e:#?}"))?;
        Ok(())
    }
    #[test]
    fn test_record_cfg() -> Result<()> {
        Object::deserialize("<RENDER_CFG\r\n  ZXZhdxgAAQ==\r\n>", 0)
            .map_err(|e| eyre!("{e:#?}"))?;
        Ok(())
    }

    #[test]
    fn test_really_big_item() -> Result<()> {
        let example = "<ITEM\r\n  POSITION 0\r\n  SNAPOFFS 0\r\n  LENGTH 188.04\r\n  LOOP 1\r\n  ALLTAKES 0\r\n  FADEIN 1 0.01 0 1 0 0 0\r\n  FADEOUT 1 0.01 0 1 0 0 0\r\n  MUTE 0 0\r\n  SEL 1\r\n  IGUID {2F6AD700-840B-EFB6-D384-7F8316E1C1E7}\r\n  IID 21\r\n  NAME barbarah-anne---2023-07-31--20-51-57.mov\r\n  VOLPAN 1 0 1 -1\r\n  SOFFS 0\r\n  PLAYRATE 1 0 0 -1 0 0.0025\r\n  CHANMODE 0\r\n  GUID {A365E92F-3BF8-24E8-1FF4-8FDF30208BCB}\r\n  <SOURCE VIDEO\r\n    FILE \"video-recordings/barbarah-anne---2023-07-31--20-51-57.mov\"\r\n  >\r\n>";
        Object::deserialize(example, 0).map_err(|e| eyre!("{e:#?}"))?;

        Ok(())
    }
    #[test]
    fn test_nested_object() -> Result<()> {
        let nested = r#"<ITEM
  POSITION 0
  SNAPOFFS 0
  LENGTH 188.04
  LOOP 1
  ALLTAKES 0
  FADEIN 1 0.01 0 1 0 0 0
  FADEOUT 1 0.01 0 1 0 0 0
  MUTE 0 0
  SEL 1
  IGUID {2F6AD700-840B-EFB6-D384-7F8316E1C1E7}
  IID 21
  NAME barbarah-anne---2023-07-31--20-51-57.mov
  VOLPAN 1 0 1 -1
  SOFFS 0
  PLAYRATE 1 0 0 -1 0 0.0025
  CHANMODE 0
  GUID {A365E92F-3BF8-24E8-1FF4-8FDF30208BCB}
  <SOURCE VIDEO
    FILE "video-recordings/barbarah-anne---2023-07-31--20-51-57.mov"
  >
>"#;
        Object::deserialize(nested, 0).map_err(|e| eyre!("{e:#?}"))?;

        Ok(())
    }

    #[test]
    fn test_weird_track() -> Result<()> {
        let example = r#"<TRACK {7E81B987-2285-6CDD-D836-6728BF78773C}
  NAME PLATE
  PEAKCOL 16576
  BEAT -1
  AUTOMODE 0
  VOLPAN 2.15306599269332 0 -1 -1 1
  MUTESOLO 0 0 0
  IPHASE 0
  PLAYOFFS 0 1
  ISBUS 0 0
  BUSCOMP 0 0 0 0 0
  SHOWINMIX 1 0.558065 0.5 1 0.5 0 0 0
  SEL 0
  REC 0 0 0 0 0 0 0 0
  VU 2
  TRACKHEIGHT 94 0 0 0 0 0
  INQ 0 0 0 0.5 100 0 0 100
  NCHAN 2
  FX 1
  TRACKID {7E81B987-2285-6CDD-D836-6728BF78773C}
  PERF 0
  AUXRECV 0 0 1 0 0 0 0 0 0 -1:U 0 -1 ''
  AUXRECV 1 0 1 0 0 0 0 0 0 -1:U 0 -1 ''
  AUXRECV 2 0 1 0 0 0 0 0 0 -1:U 0 -1 ''
  AUXRECV 3 0 1 0 0 0 0 0 0 -1:U 0 -1 ''
  AUXRECV 4 0 1 0 0 0 0 0 0 -1:U 0 -1 ''
  AUXRECV 5 0 1 0 0 0 0 0 0 -1:U 0 -1 ''
  AUXRECV 6 0 1 0 0 0 0 0 0 -1:U 0 -1 ''
  AUXRECV 7 0 1 0 0 0 0 0 0 -1:U 0 -1 ''
  AUXRECV 8 0 1 0 0 0 0 0 0 -1:U 0 -1 ''
  AUXRECV 9 0 1 0 0 0 0 0 0 -1:U 0 -1 ''
  AUXRECV 11 0 1 0 0 0 0 0 0 -1:U 0 -1 ''
  AUXRECV 12 0 1 0 0 0 0 0 0 -1:U 0 -1 ''
  AUXRECV 13 0 1 0 0 0 0 0 0 -1:U 0 -1 ''
  AUXRECV 14 0 1 0 0 0 0 0 0 -1:U 0 -1 ''
  AUXRECV 15 0 1 0 0 0 0 0 0 -1:U 0 -1 ''
  AUXRECV 16 0 1 0 0 0 0 0 0 -1:U 0 -1 ''
  AUXRECV 17 0 1 0 0 0 0 0 0 -1:U 0 -1 ''
  AUXRECV 18 0 1 0 0 0 0 0 0 -1:U 0 -1 ''
  AUXRECV 19 0 1 0 0 0 0 0 0 -1:U 0 -1 ''
  AUXRECV 20 0 1 0 0 0 0 0 0 -1:U 0 -1 ''
  MIDIOUT -1
  MAINSEND 1 0
  <FXCHAIN
    WNDRECT 1952 77 943 422
    SHOW 0
    LASTSEL 0
    DOCKED 0
    BYPASS 0 0 0
    <VST "VST: Dragonfly Plate Reverb (Michael Willis)" DragonflyPlateReverb-vst.so 0 "" 1684434995<56535464667033647261676F6E666C79> ""
      M3BmZO5e7f4CAAAAAQAAAAAAAAACAAAAAAAAAAIAAAABAAAAAAAAAAIAAAAAAAAAkgAAAAEAAAAAABAA
      cHJlc2V0AENsZWFyIFBsYXRlAABkcnlfbGV2ZWwAMABlYXJseV9sZXZlbAAxMDAAYWxnb3JpdGhtADEAd2lkdGgAMTAwAHByZWRlbGF5ADAAZGVjYXkAMC40MDAwMDAw
      MDU5NgBsb3dfY3V0ADIwMABoaWdoX2N1dAAxNjAwMABlYXJseV9kYW1wADEzMDAwAAA=
      AERlZmF1bHQAEAAAAA==
    >
    PRESETNAME Default
    FLOATPOS 0 0 0 0
    FXID {51DB3976-E446-E2A5-4F8A-00667D8BE496}
    WAK 0 0
  >
>"#;

        let (out, _) = Object::deserialize(example, 0).map_err(|e| eyre!("{e:#?}"))?;
        assert_eq!(out, "");
        Ok(())
    }

    #[test]
    fn test_vst() -> Result<()> {
        let example = r#"<VST "VST: Dragonfly Plate Reverb (Michael Willis)" DragonflyPlateReverb-vst.so 0 "" 1684434995<56535464667033647261676F6E666C79> ""
  M3BmZO5e7f4CAAAAAQAAAAAAAAACAAAAAAAAAAIAAAABAAAAAAAAAAIAAAAAAAAAkgAAAAEAAAAAABAA
  cHJlc2V0AENsZWFyIFBsYXRlAABkcnlfbGV2ZWwAMABlYXJseV9sZXZlbAAxMDAAYWxnb3JpdGhtADEAd2lkdGgAMTAwAHByZWRlbGF5ADAAZGVjYXkAMC40MDAwMDAw
  MDU5NgBsb3dfY3V0ADIwMABoaWdoX2N1dAAxNjAwMABlYXJseV9kYW1wADEzMDAwAAA=
  AERlZmF1bHQAEAAAAA==
>"#;

        assert_eq!(Object::deserialize(example, 0)?.0, "");
        Ok(())
    }
    #[test]
    fn test_up_to_render_1x() -> Result<()> {
        let example = r#"<REAPER_PROJECT 0.1 "6.80/linux-x86_64" 1691227194
  RENDER_1X 0
>"#;
        Object::deserialize(example, 0).map_err(|e| eyre!("{e:#?}"))?;
        Ok(())
    }
    #[test]
    fn test_example_document_parses() -> Result<()> {
        let (_, object) = Object::deserialize(EXAMPLE_1, 0).map_err(|e| eyre!("{e:#?}"))?;
        println!("{object:#?}");
        Ok(())
    }
    #[test]
    fn test_example_document_reserializes() -> Result<()> {
        let object = from_str(EXAMPLE_1)?;
        let serialized = to_string(object.clone())?;
        std::fs::write("/tmp/test_example_document_reserializes.rpp", &serialized)?;
        assert_eq!(EXAMPLE_1, &serialized);
        Ok(())
    }
}
