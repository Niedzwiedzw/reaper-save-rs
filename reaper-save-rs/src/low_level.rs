use nom::{
    branch::alt,
    bytes::complete::{take_while, take_while1, take_while_m_n},
    combinator::opt,
    multi::{many0, separated_list1},
    sequence::{delimited, tuple},
    IResult, Parser,
};
use nom_supreme::{error::ErrorTree, tag::complete::tag, ParserExt};
use std::{any::type_name, fmt::Write, iter::once};
use tracing::{instrument, trace};

pub mod error;

macro_rules! location {
    () => {
        concat!(file!(), ":", line!())
    };
}

const INDENT_SPACES: usize = 2;

type Input<'input> = &'input str;
type Output<'output> = &'output mut String;
type Res<'input, U> = IResult<Input<'input>, U, ErrorTree<Input<'input>>>;
type Float = OrderedFloat<f64>;
use ordered_float::OrderedFloat;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ReaperUid(pub String);

impl SerializeAndDeserialize for ReaperUid {
    fn serialize<'out>(&self, out: Output<'out>, _: usize) -> error::Result<Output<'out>> {
        write!(out, "{{{}}}", self.0)
            .map_err(Into::into)
            .map(|_| out)
    }
    #[instrument(fields(location=location!(), this=type_name::<Self>(), input=input.chars().take(20).collect::<String>()), level = "TRACE")]
    fn deserialize(input: Input, indent: usize) -> Res<Self> {
        trace!(?indent, "ReaperUid");
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

#[derive(Debug, Clone, PartialEq, Eq, enum_as_inner::EnumAsInner)]
pub enum ReaperString {
    SingleQuote(String),
    DoubleQuote(String),
    Unquoted(String),
}

impl AsRef<String> for ReaperString {
    fn as_ref(&self) -> &String {
        match self {
            ReaperString::SingleQuote(v)
            | ReaperString::DoubleQuote(v)
            | ReaperString::Unquoted(v) => v,
        }
    }
}

impl AsMut<String> for ReaperString {
    fn as_mut(&mut self) -> &mut String {
        match self {
            ReaperString::SingleQuote(v)
            | ReaperString::DoubleQuote(v)
            | ReaperString::Unquoted(v) => v,
        }
    }
}

impl SerializeAndDeserialize for ReaperString {
    fn serialize<'out>(&self, out: Output<'out>, _: usize) -> error::Result<Output<'out>> {
        match self {
            ReaperString::SingleQuote(v) => write!(out, "'{v}'"),
            ReaperString::DoubleQuote(v) => write!(out, "\"{v}\""),
            ReaperString::Unquoted(v) => write!(out, "{v}"),
        }
        .map_err(Into::into)
        .map(|_| out)
    }

    fn deserialize(input: Input, indent: usize) -> Res<Self> {
        trace!(?indent, "ReaperString");
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

#[derive(Debug, Clone, PartialEq, Eq, enum_as_inner::EnumAsInner)]
pub enum Attribute {
    ReaperUid(ReaperUid),
    Int(Int),
    String(ReaperString),
    Float(Float),
    UNumber(Int),
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AnonymousParameter(pub String);

const BASE64_CHARACTERS: &[char] = &['A', 
'Q', 
'g', 
'w',
'B', 
'R', 
'h', 
'x',
'C', 
'S', 
'i', 
'y',
'D', 
'T', 
'j', 
'z',
'E', 
'U', 
'k', 
'0',
'F', 
'V', 
'l', 
'1',
'G', 
'W', 
'm', 
'2',
'H', 
'X', 
'n', 
'3',
'I', 
'Y', 
'o', 
'4',
'J', 
'Z', 
'p', 
'5',
'K', 
'a', 
'q', 
'6',
'L', 
'b', 
'r', 
'7',
'M', 
'c', 
's', 
'8',
'N', 
'd', 
't', 
'9',
'O', 
'e', 
'u', 
'+',
'P', 
'f', 
'v', 
'/',
'='];

impl SerializeAndDeserialize for AnonymousParameter {
    fn serialize<'out>(&self, out: Output<'out>, indent: usize) -> error::Result<Output<'out>> {
        write_indent(out, indent)?;
        write!(out, "{}", self.0)?;
        Ok(out)
    }

    #[instrument(fields(location=location!(), this=type_name::<Self>(), input=input.chars().take(20).collect::<String>()), level = "TRACE")]
    fn deserialize(input: Input, indent: usize) -> Res<Self> {
        trace!(?indent, "AnonymousParameter");
        
        take_while1(|c: char| c.is_alphanumeric() || BASE64_CHARACTERS.contains(&c))
            .map(|v: Input| Self(v.to_owned()))
            .preceded_by(|input| parse_indents(input, indent))
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

#[instrument(fields(input=input.chars().take(20).collect::<String>()), level = "TRACE")]
fn parse_unescaped_string(input: Input) -> Res<String> {
    take_while(|c: char| !c.is_whitespace())
        .map(|v: Input| v.to_owned())
        .context("reading string")
        .parse(input)
}

fn parse_float(input: Input) -> Res<Float> {
    take_while(|v: char| !v.is_whitespace())
        .map_res(|v: Input| v.parse::<f64>())
        .map(OrderedFloat)
        .context("reading float")
        .parse(input)
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
            Attribute::UNumber(Int(v)) => write!(out, "{}:U", v),
        }
        .map_err(Into::into)
        .map(|_| out)
    }

    #[instrument(fields(location=location!(), this=type_name::<Self>(), input=input.chars().take(20).collect::<String>()), level = "TRACE")]
    fn deserialize(input: Input, indent: usize) -> Res<Self> {
        trace!(?indent, "Attribute");
        alt((
            |v| ReaperUid::deserialize(v, 0).map(|(out, v)| (out, Self::ReaperUid(v))),
            |v| ReaperString::deserialize(v, 0).map(|(out, v)| (out, Self::String(v))),
            parse_int.map(Self::Int),
            parse_float.map(Self::Float),
            parse_u_number.map(Self::UNumber),
            parse_unescaped_string.map(|v| Self::String(ReaperString::Unquoted(v))),
        ))
        .context(type_name::<Self>())
        .parse(input)
    }
}

#[derive(
    Debug, Clone, PartialEq, Eq, derive_more::Display, derive_more::AsRef, derive_more::Constructor,
)]
pub struct AttributeName(String);

impl SerializeAndDeserialize for AttributeName {
    fn serialize<'out>(&self, out: Output<'out>, _indent: usize) -> error::Result<Output<'out>> {
        write!(out, "{}", self.0).map_err(Into::into).map(|_| out)
    }

    #[instrument(fields(location=location!(), this=type_name::<Self>(), input=input.chars().take(20).collect::<String>()), level = "TRACE")]
    fn deserialize(input: Input, indent: usize) -> Res<Self> {
        trace!(?indent, "AttributeName");
        take_while1(|c: char| (c.is_alphabetic() && c.is_uppercase()) || c.is_numeric() || c == '_')
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

    #[instrument(fields(location=location!(), this=type_name::<Self>(), input=input.chars().take(20).collect::<String>()), level = "TRACE")]
    fn deserialize(input: Input, indent: usize) -> Res<Self> {
        trace!(?indent, "Line");
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

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Object {
    pub header: Line,
    pub values: Vec<Entry>,
}

impl Object {
    pub fn child_object_mut(&mut self, name: &str) -> Option<&mut Object> {
        self.values
            .iter_mut()
            .find_map(|e| e.as_object_mut())
            .filter(|o| o.header.attribute.as_ref().eq(name))
    }
    pub fn attributes_mut(&mut self, param: &str) -> Option<&mut Vec<Attribute>> {
        self.values.iter_mut().find_map(|e| {
            e.as_line_mut()
                .and_then(|line| line.attribute.as_ref().eq(param).then_some(line))
                .map(|line| &mut line.values)
        })
    }

    pub fn single_attribute_mut(&mut self, param: &str) -> error::Result<&mut Attribute> {
        self.attributes_mut(param)
            .ok_or_else(|| self::error::Error::ObjectNoSuchParam {
                param: param.to_owned(),
            })
            .and_then(|params| {
                let params_count = params.len();
                params.first_mut().ok_or(self::error::Error::BadParamCount {
                    expected: 1,
                    found: params_count,
                })
            })
    }
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

    #[instrument(skip(input), fields(location=location!(), this=type_name::<Self>(), input=input.chars().take(20).collect::<String>()), level = "TRACE")]
    fn deserialize(input: Input, indent: usize) -> Res<Self> {
        trace!(?indent, "Object");
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

#[derive(Debug, Clone, PartialEq, Eq, enum_as_inner::EnumAsInner)]
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

    #[instrument(fields(location=location!(), this=type_name::<Self>(), input=input.chars().take(20).collect::<String>()), level = "TRACE")]
    fn deserialize(input: Input, indent: usize) -> Res<Self> {
        trace!(?indent, "Entry");
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

    ///  TODO: investigate what exactly is the difference...
    #[test]
    #[ignore]
    fn test_example_document_reserializes() -> Result<()> {
        let object = from_str(EXAMPLE_1)?;
        let serialized = to_string(object.clone())?;
        std::fs::write("/tmp/test_example_document_reserializes.rpp", &serialized)?;
        assert_eq!(EXAMPLE_1, &serialized);
        Ok(())
    }
    #[test]
    fn test_render_cfg() -> Result<()> {
        let render_cfg = r#"<RENDER_CFG
  ZXZhdxgAAQ==
>"#;
        let object = from_str(render_cfg)?;
        println!("{object:#?}");

        Ok(())
    }

    #[test]
    fn test_render_cfg_2() -> Result<()> {
        let render_cfg_2 = r#"<RENDER_CFG2
  bDNwbQABAAAAAAAAAgAAAP////8EAAAAAAEAAAAAAAA=
>"#;
        let object = from_str(render_cfg_2)?;
        println!("{object:#?}");

        Ok(())
    }

    macro_rules! assert_object {
        ($test_name:ident, $content:literal) => {
            #[test]
            fn $test_name() -> Result<()> {
                let example = $content;
                let (out, _) = Object::deserialize(example, 0).map_err(|e| eyre!("{e:#?}"))?;
                assert_eq!(out, "");
                Ok(())
            }
        }
    }

    assert_object!(test_vst_amplitube, r#"<VST "VST3: AmpliTube 5 (IK Multimedia)" "AmpliTube 5.vst3" 0 "" 1566108953{56535441746235616D706C6974756265} ""
  Ge1YXe5e7f4CAAAAAQAAAAAAAAACAAAAAAAAAAIAAAABAAAAAAAAAAIAAAAAAAAAJSgAAAEAAAD//xAAFSgAAAEAAABWc3RXAAAACAAAAAEAAAAAQ2NuSwAAJ/1GQkNo
  AAAAAkF0YjUABQcEAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAA
>"#);

    #[test]
    fn test_weird_track_2() -> Result<()> {
        let example = r#"<TRACK {C7D7917F-D94F-ED85-1D58-2F258596E414}
  NAME "GTX PRZEMEK"
  PEAKCOL 25362292
  BEAT -1
  AUTOMODE 0
  PANLAWFLAGS 3
  VOLPAN 0.45309238622556 0 -1 -1 1
  MUTESOLO 0 0 0
  IPHASE 0
  PLAYOFFS 0 1
  ISBUS 0 0
  BUSCOMP 0 0 0 0 0
  SHOWINMIX 1 0.6667 0.5 1 0.5 0 0 0
  FIXEDLANES 9 0 0 0 0
  SEL 0
  REC 0 0 0 0 0 0 0 0
  VU 16
  SPACER 1
  TRACKHEIGHT 0 0 0 0 0 0 0
  INQ 0 0 0 0.5 100 0 0 100
  NCHAN 2
  FX 1
  TRACKID {C7D7917F-D94F-ED85-1D58-2F258596E414}
  PERF 0
  MIDIOUT -1
  MAINSEND 1 0
  <FXCHAIN
    WNDRECT 2766 506 867 458
    SHOW 0
    LASTSEL 0
    DOCKED 0
    BYPASS 0 0 0
    <VST "VST: ReaComp (Cockos)" reacomp.dll 0 "" 1919247213<5653547265636D726561636F6D700000> ""
      bWNlcu9e7f4EAAAAAQAAAAAAAAACAAAAAAAAAAQAAAAAAAAACAAAAAAAAAACAAAAAQAAAAAAAAACAAAAAAAAAFwAAAAAAAAAAAAAAA==
      776t3g3wrd4KDqg9Bh7kPlboczw2LdA8AAAAAAAAAAARYKg8AAAAAAAAAAAAAAAAvTeGNTeY1D8AAAAAwcrhPocW2T0AAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAA=
      AHN0b2NrIC0gQWNvdXN0aWMgR3VpdGFyAAAAAAA=
    >
    WET 0.55996 0
    PRESETNAME "stock - Acoustic Guitar"
    FLOATPOS 0 0 0 0
    FXID {82FE96D9-2141-2257-083F-F201758870C5}
    WAK 0 0
    BYPASS 0 0 0
    <VST "VST3: AmpliTube 5 (IK Multimedia)" "AmpliTube 5.vst3" 0 "" 1566108953{56535441746235616D706C6974756265} ""
      Ge1YXe5e7f4CAAAAAQAAAAAAAAACAAAAAAAAAAIAAAABAAAAAAAAAAIAAAAAAAAAJSgAAAEAAAD//xAAFSgAAAEAAABWc3RXAAAACAAAAAEAAAAAQ2NuSwAAJ/1GQkNo
      AAAAAkF0YjUABQcEAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAA
      AAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAnZVN0YXRlAAEDUHJlc2V0RGF0YQACRCYIPD94bWwgdmVyc2lvbj0iMS4wIiBl
      bmNvZGluZz0iVVRGLTgiID8+PFByb2dyYW0gVmVyc2lvbj0iMiIgRm9ybWF0PSJhdDVwIiBHVUlEPSJjMWRjYmVjYS0wYzdlLTRiYmMtOWI5MS0yNDlmZTUyMTdiOWUi
      IFByZXNldEJQTT0iMTIwIiBQcm9ncmFtQ2hhbmdlPSItMSIgUHJlc2V0TmFtZT0ic3RyYXN6bmEtaXN0b3RhLXdvanRlayIgUHJlc2V0UGF0aD0iQzpcdXNlcnNcbmll
      ZHp3aWVkelxNeSBEb2N1bWVudHNcSUsgTXVsdGltZWRpYVxBbXBsaVR1YmUgNVxQcmVzZXRzXHN0cmFzem5hLWlzdG90YS13b2p0ZWsuYXQ1cCI+PENoYWluIFByZXNl
      dD0iQ2hhaW4xMSIgRElCZWZvcmVBbXA9IjAiIC8+PElucHV0IElucHV0PSIxIiAvPjxUdW5lciBCeXBhc3M9IjEiIE11dGU9IjAiIE91dHB1dFZvbHVtZT0iMSIgVHVu
      ZXJUeXBlPSIzNTRlY2E1MS00NTdhLTQxYjctOTE3ZC1jZTYxMTc1ODY5MDUiPjxUdW5lciBSZWZlcmVuY2U9IjQ0MCIgTm90ZVJlZmVyZW1jZT0iQSIgVHJhbnNwb3Nl
      PSIwIiBUZW1wZXJhbWVudD0iRXF1YWwiIC8+PC9UdW5lcj48U3RvbXBBMSBCeXBhc3M9IjAiIE11dGU9IjAiIE91dHB1dFZvbHVtZT0iMSIgU3RvbXAwPSI3NzNiOGVh
      Ny1iNTRhLTRhM2MtOTlkZi1mZmJiZjZkMjkyNzEiIFN0b21wMT0iNzczYjhlYTctYjU0YS00YTNjLTk5ZGYtZmZiYmY2ZDI5MjcxIiBTdG9tcDI9Ijc3M2I4ZWE3LWI1
      NGEtNGEzYy05OWRmLWZmYmJmNmQyOTI3MSIgU3RvbXAzPSI3NzNiOGVhNy1iNTRhLTRhM2MtOTlkZi1mZmJiZjZkMjkyNzEiIFN0b21wND0iNzczYjhlYTctYjU0YS00
      YTNjLTk5ZGYtZmZiYmY2ZDI5MjcxIiBTdG9tcDU9Ijc3M2I4ZWE3LWI1NGEtNGEzYy05OWRmLWZmYmJmNmQyOTI3MSI+PFNsb3QwIC8+PFNsb3QxIC8+PFNsb3QyIC8+
      PFNsb3QzIC8+PFNsb3Q0IC8+PFNsb3Q1IC8+PC9TdG9tcEExPjxTdG9tcEEyIEJ5cGFzcz0iMCIgTXV0ZT0iMCIgT3V0cHV0Vm9sdW1lPSIxIiBTdG9tcDA9Ijc3M2I4
      ZWE3LWI1NGEtNGEzYy05OWRmLWZmYmJmNmQyOTI3MSIgU3RvbXAxPSI3NzNiOGVhNy1iNTRhLTRhM2MtOTlkZi1mZmJiZjZkMjkyNzEiIFN0b21wMj0iNzczYjhlYTct
      YjU0YS00YTNjLTk5ZGYtZmZiYmY2ZDI5MjcxIiBTdG9tcDM9Ijc3M2I4ZWE3LWI1NGEtNGEzYy05OWRmLWZmYmJmNmQyOTI3MSIgU3RvbXA0PSI3NzNiOGVhNy1iNTRh
      LTRhM2MtOTlkZi1mZmJiZjZkMjkyNzEiIFN0b21wNT0iNzczYjhlYTctYjU0YS00YTNjLTk5ZGYtZmZiYmY2ZDI5MjcxIj48U2xvdDAgLz48U2xvdDEgLz48U2xvdDIg
      Lz48U2xvdDMgLz48U2xvdDQgLz48U2xvdDUgLz48L1N0b21wQTI+PFN0b21wU3RlcmVvIEJ5cGFzcz0iMCIgTXV0ZT0iMCIgT3V0cHV0Vm9sdW1lPSIxIiBTdG9tcDA9
      Ijc3M2I4ZWE3LWI1NGEtNGEzYy05OWRmLWZmYmJmNmQyOTI3MSIgU3RvbXAxPSI3NzNiOGVhNy1iNTRhLTRhM2MtOTlkZi1mZmJiZjZkMjkyNzEiIFN0b21wMj0iNzcz
      YjhlYTctYjU0YS00YTNjLTk5ZGYtZmZiYmY2ZDI5MjcxIj48U2xvdDAgLz48U2xvdDEgLz48U2xvdDIgLz48L1N0b21wU3RlcmVvPjxTdG9tcEIxIEJ5cGFzcz0iMCIg
      TXV0ZT0iMCIgT3V0cHV0Vm9sdW1lPSIxIiBTdG9tcDA9Ijc3M2I4ZWE3LWI1NGEtNGEzYy05OWRmLWZmYmJmNmQyOTI3MSIgU3RvbXAxPSI3NzNiOGVhNy1iNTRhLTRh
      M2MtOTlkZi1mZmJiZjZkMjkyNzEiIFN0b21wMj0iNzczYjhlYTctYjU0YS00YTNjLTk5ZGYtZmZiYmY2ZDI5MjcxIiBTdG9tcDM9Ijc3M2I4ZWE3LWI1NGEtNGEzYy05
      OWRmLWZmYmJmNmQyOTI3MSIgU3RvbXA0PSI3NzNiOGVhNy1iNTRhLTRhM2MtOTlkZi1mZmJiZjZkMjkyNzEiIFN0b21wNT0iNzczYjhlYTctYjU0YS00YTNjLTk5ZGYt
      ZmZiYmY2ZDI5MjcxIj48U2xvdDAgLz48U2xvdDEgLz48U2xvdDIgLz48U2xvdDMgLz48U2xvdDQgLz48U2xvdDUgLz48L1N0b21wQjE+PFN0b21wQjIgQnlwYXNzPSIw
      IiBNdXRlPSIwIiBPdXRwdXRWb2x1bWU9IjEiIFN0b21wMD0iNzczYjhlYTctYjU0YS00YTNjLTk5ZGYtZmZiYmY2ZDI5MjcxIiBTdG9tcDE9Ijc3M2I4ZWE3LWI1NGEt
      NGEzYy05OWRmLWZmYmJmNmQyOTI3MSIgU3RvbXAyPSI3NzNiOGVhNy1iNTRhLTRhM2MtOTlkZi1mZmJiZjZkMjkyNzEiIFN0b21wMz0iNzczYjhlYTctYjU0YS00YTNj
      LTk5ZGYtZmZiYmY2ZDI5MjcxIiBTdG9tcDQ9Ijc3M2I4ZWE3LWI1NGEtNGEzYy05OWRmLWZmYmJmNmQyOTI3MSIgU3RvbXA1PSI3NzNiOGVhNy1iNTRhLTRhM2MtOTlk
      Zi1mZmJiZjZkMjkyNzEiPjxTbG90MCAvPjxTbG90MSAvPjxTbG90MiAvPjxTbG90MyAvPjxTbG90NCAvPjxTbG90NSAvPjwvU3RvbXBCMj48U3RvbXBCMyBCeXBhc3M9
      IjAiIE11dGU9IjAiIE91dHB1dFZvbHVtZT0iMSIgU3RvbXAwPSI3NzNiOGVhNy1iNTRhLTRhM2MtOTlkZi1mZmJiZjZkMjkyNzEiIFN0b21wMT0iNzczYjhlYTctYjU0
      YS00YTNjLTk5ZGYtZmZiYmY2ZDI5MjcxIiBTdG9tcDI9Ijc3M2I4ZWE3LWI1NGEtNGEzYy05OWRmLWZmYmJmNmQyOTI3MSIgU3RvbXAzPSI3NzNiOGVhNy1iNTRhLTRh
      M2MtOTlkZi1mZmJiZjZkMjkyNzEiIFN0b21wND0iNzczYjhlYTctYjU0YS00YTNjLTk5ZGYtZmZiYmY2ZDI5MjcxIiBTdG9tcDU9Ijc3M2I4ZWE3LWI1NGEtNGEzYy05
      OWRmLWZmYmJmNmQyOTI3MSI+PFNsb3QwIC8+PFNsb3QxIC8+PFNsb3QyIC8+PFNsb3QzIC8+PFNsb3Q0IC8+PFNsb3Q1IC8+PC9TdG9tcEIzPjxBbXBBIEJ5cGFzcz0i
      MCIgTXV0ZT0iMCIgT3V0cHV0Vm9sdW1lPSIxIiBNb2RlbD0iOGZlOTY5MzYtNTE3OC00OTUwLTliODAtZDg5YzMyNTM0YmFkIj48QW1wIFNlbnNpdGl2aXR5X0pDTTgw
      MEFUND0iMSIgUHJlc2VuY2VfSkNNODAwQVQ0PSI2LjA0IiBCYXNzX0pDTTgwMEFUND0iNi4yODYxNiIgTWlkZGxlX0pDTTgwMEFUND0iNC44ODM1OSIgVHJlYmxlX0pD
      TTgwMEFUND0iNS4yMjk2OSIgTWFzdGVyX0pDTTgwMEFUND0iNi4xMjU4NCIgUHJlQW1wX0pDTTgwMEFUND0iNC4zNDA1IiAvPjwvQW1wQT48QW1wQiBCeXBhc3M9IjAi
      IE11dGU9IjAiIE91dHB1dFZvbHVtZT0iMSIgTW9kZWw9IjhmZTk2OTM2LTUxNzgtNDk1MC05YjgwLWQ4OWMzMjUzNGJhZCI+PEFtcCBTZW5zaXRpdml0eV9KQ004MDBB
      VDQ9IjEiIFByZXNlbmNlX0pDTTgwMEFUND0iNSIgQmFzc19KQ004MDBBVDQ9IjQiIE1pZGRsZV9KQ004MDBBVDQ9IjUiIFRyZWJsZV9KQ004MDBBVDQ9IjYiIE1hc3Rl
      cl9KQ004MDBBVDQ9IjUuNSIgUHJlQW1wX0pDTTgwMEFUND0iNSIgLz48L0FtcEI+PEFtcEMgQnlwYXNzPSIwIiBNdXRlPSIwIiBPdXRwdXRWb2x1bWU9IjEiIE1vZGVs
      PSI4ZmU5NjkzNi01MTc4LTQ5NTAtOWI4MC1kODljMzI1MzRiYWQiPjxBbXAgU2Vuc2l0aXZpdHlfSkNNODAwQVQ0PSIxIiBQcmVzZW5jZV9KQ004MDBBVDQ9IjUiIEJh
      c3NfSkNNODAwQVQ0PSI0IiBNaWRkbGVfSkNNODAwQVQ0PSI1IiBUcmVibGVfSkNNODAwQVQ0PSI2IiBNYXN0ZXJfSkNNODAwQVQ0PSI1LjUiIFByZUFtcF9KQ004MDBB
      VDQ9IjUiIC8+PC9BbXBDPjxMb29wRnhBIEJ5cGFzcz0iMCIgTXV0ZT0iMCIgT3V0cHV0Vm9sdW1lPSIxIiBTdG9tcDA9Ijc3M2I4ZWE3LWI1NGEtNGEzYy05OWRmLWZm
      YmJmNmQyOTI3MSIgU3RvbXAxPSI3NzNiOGVhNy1iNTRhLTRhM2MtOTlkZi1mZmJiZjZkMjkyNzEiIFN0b21wMj0iNzczYjhlYTctYjU0YS00YTNjLTk5ZGYtZmZiYmY2
      ZDI5MjcxIiBTdG9tcDM9Ijc3M2I4ZWE3LWI1NGEtNGEzYy05OWRmLWZmYmJmNmQyOTI3MSI+PFNsb3QwIC8+PFNsb3QxIC8+PFNsb3QyIC8+PFNsb3QzIC8+PC9Mb29w
      RnhBPjxMb29wRnhCIEJ5cGFzcz0iMCIgTXV0ZT0iMCIgT3V0cHV0Vm9sdW1lPSIxIiBTdG9tcDA9Ijc3M2I4ZWE3LWI1NGEtNGEzYy05OWRmLWZmYmJmNmQyOTI3MSIg
      U3RvbXAxPSI3NzNiOGVhNy1iNTRhLTRhM2MtOTlkZi1mZmJiZjZkMjkyNzEiIFN0b21wMj0iNzczYjhlYTctYjU0YS00YTNjLTk5ZGYtZmZiYmY2ZDI5MjcxIiBTdG9t
      cDM9Ijc3M2I4ZWE3LWI1NGEtNGEzYy05OWRmLWZmYmJmNmQyOTI3MSI+PFNsb3QwIC8+PFNsb3QxIC8+PFNsb3QyIC8+PFNsb3QzIC8+PC9Mb29wRnhCPjxMb29wRnhD
      IEJ5cGFzcz0iMCIgTXV0ZT0iMCIgT3V0cHV0Vm9sdW1lPSIxIiBTdG9tcDA9Ijc3M2I4ZWE3LWI1NGEtNGEzYy05OWRmLWZmYmJmNmQyOTI3MSIgU3RvbXAxPSI3NzNi
      OGVhNy1iNTRhLTRhM2MtOTlkZi1mZmJiZjZkMjkyNzEiIFN0b21wMj0iNzczYjhlYTctYjU0YS00YTNjLTk5ZGYtZmZiYmY2ZDI5MjcxIiBTdG9tcDM9Ijc3M2I4ZWE3
      LWI1NGEtNGEzYy05OWRmLWZmYmJmNmQyOTI3MSI+PFNsb3QwIC8+PFNsb3QxIC8+PFNsb3QyIC8+PFNsb3QzIC8+PC9Mb29wRnhDPjxDYWJBIEJ5cGFzcz0iMCIgTXV0
      ZT0iMCIgQ2FiTW9kZWw9IjdjMGI4Y2UxLWNiYjQtNGU1Yi05OTczLWE1NzIxNDNkZGIyYiIgU3BlYWtlck1vZGVsMD0iOTQyMTUzZDI4MWZiNGIwODlmYzIwZTA3YTM0
      ZTljYTciIFNwZWFrZXJNb2RlbDE9Ijk0MjE1M2QyODFmYjRiMDg5ZmMyMGUwN2EzNGU5Y2E3IiBTcGVha2VyTW9kZWwyPSI5NDIxNTNkMjgxZmI0YjA4OWZjMjBlMDdh
      MzRlOWNhNyIgU3BlYWtlck1vZGVsMz0iOTQyMTUzZDI4MWZiNGIwODlmYzIwZTA3YTM0ZTljYTciIElSRGVjaW1hdGlvbj0iMSI+PENhYiBIaWdoTGV2ZWw9IjAuNzci
      IFJvb21UeXBlPSJIYWxsIiBSb29tTWljVHlwZT0iQ29uZGVuc2VyIDg3IiBNaWMwTW9kZWw9IjFlNDFhY2M0LTg1YWYtNGU4NC1iZWU0LWVhYmMwYmU1ZmVmMSIgTWlj
      MU1vZGVsPSI5ZTQ0NDI4Ni1jYWI0LTQ2YTQtYmZhMy1hNmQ1NWIzZmZjZmIiIE1pYzBBbmdsZT0iMCIgTWljMUFuZ2xlPSIwIiBNaWMwWEF4aXM9Ii0wLjAxMzQ1NTEi
      IE1pYzFYQXhpcz0iMC4xNjQ4MTIiIE1pYzBZQXhpcz0iLTAuMjEzODYzIiBNaWMxWUF4aXM9IjAuNDE2MjY3IiBNaWMwRGlzdGFuY2U9IjAiIE1pYzFEaXN0YW5jZT0i
      MC4xMzE0MTUiIE1pYzBTcGVha2VyPSIwIiBNaWMxU3BlYWtlcj0iMSIgR1VJTG9hZENvbXBsZXRlPSIwIiAvPjwvQ2FiQT48Q2FiQiBCeXBhc3M9IjAiIE11dGU9IjAi
      IENhYk1vZGVsPSI3YzBiOGNlMS1jYmI0LTRlNWItOTk3My1hNTcyMTQzZGRiMmIiIFNwZWFrZXJNb2RlbDA9Ijk0MjE1M2QyODFmYjRiMDg5ZmMyMGUwN2EzNGU5Y2E3
      IiBTcGVha2VyTW9kZWwxPSI5NDIxNTNkMjgxZmI0YjA4OWZjMjBlMDdhMzRlOWNhNyIgU3BlYWtlck1vZGVsMj0iOTQyMTUzZDI4MWZiNGIwODlmYzIwZTA3YTM0ZTlj
      YTciIFNwZWFrZXJNb2RlbDM9Ijk0MjE1M2QyODFmYjRiMDg5ZmMyMGUwN2EzNGU5Y2E3IiBJUkRlY2ltYXRpb249IjEiPjxDYWIgSGlnaExldmVsPSIwLjc3IiBSb29t
      VHlwZT0iTGFyZ2UgU3R1ZGlvIiBSb29tTWljVHlwZT0iQ29uZGVuc2VyIDg3IiBNaWMwTW9kZWw9IjFlNDFhY2M0LTg1YWYtNGU4NC1iZWU0LWVhYmMwYmU1ZmVmMSIg
      TWljMU1vZGVsPSI5ZTQ0NDI4Ni1jYWI0LTQ2YTQtYmZhMy1hNmQ1NWIzZmZjZmIiIE1pYzBBbmdsZT0iMCIgTWljMUFuZ2xlPSIwIiBNaWMwWEF4aXM9IjAuMDEzNDU1
      MSIgTWljMVhBeGlzPSIwLjE2NDgxMiIgTWljMFlBeGlzPSItMC4yMTM4NjMiIE1pYzFZQXhpcz0iMC40MTYyNjciIE1pYzBEaXN0YW5jZT0iMCIgTWljMURpc3RhbmNl
      PSIwLjEzMTQxNSIgTWljMFNwZWFrZXI9IjAiIE1pYzFTcGVha2VyPSIxIiBHVUlMb2FkQ29tcGxldGU9IjAiIC8+PC9DYWJCPjxDYWJDIEJ5cGFzcz0iMCIgTXV0ZT0i
      MCIgQ2FiTW9kZWw9IjdjMGI4Y2UxLWNiYjQtNGU1Yi05OTczLWE1NzIxNDNkZGIyYiIgU3BlYWtlck1vZGVsMD0iOTQyMTUzZDI4MWZiNGIwODlmYzIwZTA3YTM0ZTlj
      YTciIFNwZWFrZXJNb2RlbDE9Ijk0MjE1M2QyODFmYjRiMDg5ZmMyMGUwN2EzNGU5Y2E3IiBTcGVha2VyTW9kZWwyPSI5NDIxNTNkMjgxZmI0YjA4OWZjMjBlMDdhMzRl
      OWNhNyIgU3BlYWtlck1vZGVsMz0iOTQyMTUzZDI4MWZiNGIwODlmYzIwZTA3YTM0ZTljYTciIElSRGVjaW1hdGlvbj0iMSI+PENhYiBIaWdoTGV2ZWw9IjAuNzciIFJv
      b21UeXBlPSJMYXJnZSBTdHVkaW8iIFJvb21NaWNUeXBlPSJDb25kZW5zZXIgODciIE1pYzBNb2RlbD0iMWU0MWFjYzQtODVhZi00ZTg0LWJlZTQtZWFiYzBiZTVmZWYx
      IiBNaWMxTW9kZWw9IjllNDQ0Mjg2LWNhYjQtNDZhNC1iZmEzLWE2ZDU1YjNmZmNmYiIgTWljMEFuZ2xlPSIwIiBNaWMxQW5nbGU9IjAiIE1pYzBYQXhpcz0iMC4wMTM0
      NTUxIiBNaWMxWEF4aXM9IjAuMTY0ODEyIiBNaWMwWUF4aXM9Ii0wLjIxMzg2MyIgTWljMVlBeGlzPSIwLjQxNjI2NyIgTWljMERpc3RhbmNlPSIwIiBNaWMxRGlzdGFu
      Y2U9IjAuMTMxNDE1IiBNaWMwU3BlYWtlcj0iMCIgTWljMVNwZWFrZXI9IjEiIEdVSUxvYWRDb21wbGV0ZT0iMCIgLz48L0NhYkM+PFN0dWRpbyBCeXBhc3M9IjAiIE11
      dGU9IjAiIE91dHB1dFZvbHVtZT0iMSIgT3V0cHV0UGFuPSIwLjUiIERJX0xldmVsPSItMyIgRElfUGFuPSIwLjUiIERJX011dGU9IjEiIERJX1NvbG89IjAiIERJX1Bo
      YXNlPSIwIiBESV9QaGFzZURlbGF5PSIwIiBDYWIxX01pYzFfTGV2ZWw9Ii02IiBDYWIxX01pYzFfUGFuPSIwIiBDYWIxX01pYzFfTXV0ZT0iMCIgQ2FiMV9NaWMxX1Nv
      bG89IjAiIENhYjFfTWljMV9QaGFzZT0iMCIgQ2FiMV9NaWMyX0xldmVsPSItNiIgQ2FiMV9NaWMyX1Bhbj0iMCIgQ2FiMV9NaWMyX011dGU9IjAiIENhYjFfTWljMl9T
      b2xvPSIwIiBDYWIxX01pYzJfUGhhc2U9IjAiIENhYjFfUm9vbV9MZXZlbD0iLTM0LjUyNDEiIENhYjFfUm9vbV9XaWR0aD0iNTAiIENhYjFfUm9vbV9NdXRlPSIwIiBD
      YWIxX1Jvb21fU29sbz0iMCIgQ2FiMV9Sb29tX1BoYXNlPSIwIiBDYWIxX0J1c19MZXZlbD0iMCIgQ2FiMV9CdXNfUGFuPSIwLjUiIENhYjFfQnVzX011dGU9IjAiIENh
      YjFfQnVzX1NvbG89IjAiIENhYjFfQnVzX1BoYXNlPSIwIiBDYWIyX01pYzFfTGV2ZWw9Ii02IiBDYWIyX01pYzFfUGFuPSIwIiBDYWIyX01pYzFfTXV0ZT0iMCIgQ2Fi
      Ml9NaWMxX1NvbG89IjAiIENhYjJfTWljMV9QaGFzZT0iMCIgQ2FiMl9NaWMyX0xldmVsPSItNiIgQ2FiMl9NaWMyX1Bhbj0iMCIgQ2FiMl9NaWMyX011dGU9IjAiIENh
      YjJfTWljMl9Tb2xvPSIwIiBDYWIyX01pYzJfUGhhc2U9IjAiIENhYjJfUm9vbV9MZXZlbD0iLTQwIiBDYWIyX1Jvb21fV2lkdGg9IjUwIiBDYWIyX1Jvb21fTXV0ZT0i
      MCIgQ2FiMl9Sb29tX1NvbG89IjAiIENhYjJfUm9vbV9QaGFzZT0iMCIgQ2FiMl9CdXNfTGV2ZWw9Ii02IiBDYWIyX0J1c19QYW49IjEiIENhYjJfQnVzX011dGU9IjAi
      IENhYjJfQnVzX1NvbG89IjAiIENhYjJfQnVzX1BoYXNlPSIwIiBDYWIzX01pYzFfTGV2ZWw9Ii02IiBDYWIzX01pYzFfUGFuPSIwIiBDYWIzX01pYzFfTXV0ZT0iMCIg
      Q2FiM19NaWMxX1NvbG89IjAiIENhYjNfTWljMV9QaGFzZT0iMCIgQ2FiM19NaWMyX0xldmVsPSItNiIgQ2FiM19NaWMyX1Bhbj0iMCIgQ2FiM19NaWMyX011dGU9IjAi
      IENhYjNfTWljMl9Tb2xvPSIwIiBDYWIzX01pYzJfUGhhc2U9IjAiIENhYjNfUm9vbV9MZXZlbD0iLTQwIiBDYWIzX1Jvb21fV2lkdGg9IjUwIiBDYWIzX1Jvb21fTXV0
      ZT0iMCIgQ2FiM19Sb29tX1NvbG89IjAiIENhYjNfUm9vbV9QaGFzZT0iMCIgQ2FiM19CdXNfTGV2ZWw9Ii02IiBDYWIzX0J1c19QYW49IjAiIENhYjNfQnVzX011dGU9
      IjAiIENhYjNfQnVzX1NvbG89IjAiIENhYjNfQnVzX1BoYXNlPSIwIiBDYWIxX0xlc2xpZV9Ib3JuX0xldmVsPSIwIiBDYWIxX0xlc2xpZV9Ib3JuX1dpZHRoPSIxMDAi
      IENhYjFfTGVzbGllX0hvcm5fTXV0ZT0iMCIgQ2FiMV9MZXNsaWVfSG9ybl9Tb2xvPSIwIiBDYWIxX0xlc2xpZV9Ib3JuX1BoYXNlPSIwIiBDYWIxX0xlc2xpZV9EcnVt
      X0xldmVsPSIwIiBDYWIxX0xlc2xpZV9EcnVtX1dpZHRoPSIxMDAiIENhYjFfTGVzbGllX0RydW1fTXV0ZT0iMCIgQ2FiMV9MZXNsaWVfRHJ1bV9Tb2xvPSIwIiBDYWIx
      X0xlc2xpZV9EcnVtX1BoYXNlPSIwIiBDYWIyX0xlc2xpZV9Ib3JuX0xldmVsPSIwIiBDYWIyX0xlc2xpZV9Ib3JuX1dpZHRoPSIxMDAiIENhYjJfTGVzbGllX0hvcm5f
      TXV0ZT0iMCIgQ2FiMl9MZXNsaWVfSG9ybl9Tb2xvPSIwIiBDYWIyX0xlc2xpZV9Ib3JuX1BoYXNlPSIwIiBDYWIyX0xlc2xpZV9EcnVtX0xldmVsPSIwIiBDYWIyX0xl
      c2xpZV9EcnVtX1dpZHRoPSIxMDAiIENhYjJfTGVzbGllX0RydW1fTXV0ZT0iMCIgQ2FiMl9MZXNsaWVfRHJ1bV9Tb2xvPSIwIiBDYWIyX0xlc2xpZV9EcnVtX1BoYXNl
      PSIwIiBDYWIzX0xlc2xpZV9Ib3JuX0xldmVsPSIwIiBDYWIzX0xlc2xpZV9Ib3JuX1dpZHRoPSIxMDAiIENhYjNfTGVzbGllX0hvcm5fTXV0ZT0iMCIgQ2FiM19MZXNs
      aWVfSG9ybl9Tb2xvPSIwIiBDYWIzX0xlc2xpZV9Ib3JuX1BoYXNlPSIwIiBDYWIzX0xlc2xpZV9EcnVtX0xldmVsPSIwIiBDYWIzX0xlc2xpZV9EcnVtX1dpZHRoPSIx
      MDAiIENhYjNfTGVzbGllX0RydW1fTXV0ZT0iMCIgQ2FiM19MZXNsaWVfRHJ1bV9Tb2xvPSIwIiBDYWIzX0xlc2xpZV9EcnVtX1BoYXNlPSIwIiAvPjxSYWNrQSBCeXBh
      c3M9IjAiIE11dGU9IjAiIE91dHB1dFZvbHVtZT0iMSIgU3RvbXAwPSI3NzNiOGVhNy1iNTRhLTRhM2MtOTlkZi1mZmJiZjZkMjkyNzEiIFN0b21wMT0iNzczYjhlYTct
      YjU0YS00YTNjLTk5ZGYtZmZiYmY2ZDI5MjcxIj48U2xvdDAgLz48U2xvdDEgLz48L1JhY2tBPjxSYWNrQiBCeXBhc3M9IjAiIE11dGU9IjAiIE91dHB1dFZvbHVtZT0i
      MSIgU3RvbXAwPSI3NzNiOGVhNy1iNTRhLTRhM2MtOTlkZi1mZmJiZjZkMjkyNzEiIFN0b21wMT0iNzczYjhlYTctYjU0YS00YTNjLTk5ZGYtZmZiYmY2ZDI5MjcxIj48
      U2xvdDAgLz48U2xvdDEgLz48L1JhY2tCPjxSYWNrQyBCeXBhc3M9IjAiIE11dGU9IjAiIE91dHB1dFZvbHVtZT0iMSIgU3RvbXAwPSI3NzNiOGVhNy1iNTRhLTRhM2Mt
      OTlkZi1mZmJiZjZkMjkyNzEiIFN0b21wMT0iNzczYjhlYTctYjU0YS00YTNjLTk5ZGYtZmZiYmY2ZDI5MjcxIj48U2xvdDAgLz48U2xvdDEgLz48L1JhY2tDPjxSYWNr
      REkgQnlwYXNzPSIwIiBNdXRlPSIwIiBPdXRwdXRWb2x1bWU9IjEiIFN0b21wMD0iNzczYjhlYTctYjU0YS00YTNjLTk5ZGYtZmZiYmY2ZDI5MjcxIiBTdG9tcDE9Ijc3
      M2I4ZWE3LWI1NGEtNGEzYy05OWRmLWZmYmJmNmQyOTI3MSI+PFNsb3QwIC8+PFNsb3QxIC8+PC9SYWNrREk+PFJhY2tNYXN0ZXIgQnlwYXNzPSIwIiBNdXRlPSIwIiBP
      dXRwdXRWb2x1bWU9IjEiIFN0b21wMD0iNzczYjhlYTctYjU0YS00YTNjLTk5ZGYtZmZiYmY2ZDI5MjcxIiBTdG9tcDE9Ijc3M2I4ZWE3LWI1NGEtNGEzYy05OWRmLWZm
      YmJmNmQyOTI3MSIgU3RvbXAyPSI3NzNiOGVhNy1iNTRhLTRhM2MtOTlkZi1mZmJiZjZkMjkyNzEiIFN0b21wMz0iNzczYjhlYTctYjU0YS00YTNjLTk5ZGYtZmZiYmY2
      ZDI5MjcxIiBTdG9tcDQ9Ijc3M2I4ZWE3LWI1NGEtNGEzYy05OWRmLWZmYmJmNmQyOTI3MSIgU3RvbXA1PSI3NzNiOGVhNy1iNTRhLTRhM2MtOTlkZi1mZmJiZjZkMjky
      NzEiPjxTbG90MCAvPjxTbG90MSAvPjxTbG90MiAvPjxTbG90MyAvPjxTbG90NCAvPjxTbG90NSAvPjwvUmFja01hc3Rlcj48T3V0cHV0IE91dHB1dD0iMSIgLz48TWlk
      aUFzc2lnbm1lbnRzIC8+PFByZWZlcmVuY2VzIFF1YWxpdHk9IkhpZ2giIFN0b21wc092ZXJzYW1wbGluZz0iMSIgUHJlT3ZlcnNhbXBsaW5nPSIxIiBBbXBPdmVyc2Ft
      cGxpbmc9IjEiIEhpZ2hSZXNvbHV0aW9uPSIxIiBBbXBSZXZlcmJRdWFsaXR5PSJSZWFsIiBSb29tUXVhbGl0eT0iUmVhbCIgQ2FiUmVzb2x1dGlvbj0iSGlnaCIgQ2Fi
      aW5ldEdsb2JhbEJ5cGFzcz0iMCIgQlBNU291cmNlPSJHbG9iYWwiIC8+PEF1dG9tYXRpb24gU2xvdHM9IjE2IiAvPjwvUHJvZ3JhbT4AUGFuZWxzAAFRCFZDMiFHAAAA
      PD94bWwgdmVyc2lvbj0iMS4wIiBlbmNvZGluZz0iVVRGLTgiPz4gPFBhbmVscyBHZWFyVmlzaWJpbGl0eU1vZGU9IjAiLz4AR3VpU2NhbGUAAWkIVkMyIV8AAAA8P3ht
      bCB2ZXJzaW9uPSIxLjAiIGVuY29kaW5nPSJVVEYtOCI/PiA8R3VpU2NhbGUgU2NhbGVSYXRpb1dpZHRoPSIxLjAiIFNjYWxlUmF0aW9IZWlnaHQ9IjEuMCIvPgAAAAAA
      AAAAAABKVUNFUHJpdmF0ZURhdGEAAQFCeXBhc3MAAQEDAB0AAAAAAAAASlVDRVByaXZhdGVEYXRhAAAAAAAAAAAAUHJvZ3JhbSAxABAAAAA=
    >
    FLOATPOS 0 0 0 0
    FXID {8CF093C9-2187-DDFF-99B4-75CD8CBEFC78}
    WAK 0 0
  >
  <ITEM
    POSITION 0
    SNAPOFFS 0
    LENGTH 179.18850340136058
    LOOP 1
    ALLTAKES 0
    FADEIN 1 0 0 1 0 0 0
    FADEOUT 1 0 0 1 0 0 0
    MUTE 0 0
    SEL 0
    IGUID {6D3E2C73-1554-3EDF-3703-32442A4F80D0}
    IID 532
    NAME "straszna istota - sama gitara - 1.wav"
    VOLPAN 1 0 1 -1
    SOFFS 0
    PLAYRATE 1 1 0 -1 0 0.0025
    CHANMODE 0
    GUID {A7C909DB-4DAD-B892-B4F5-41897CECF546}
    <SOURCE WAVE
      FILE "audio-files\straszna istota - sama gitara - 1.wav"
    >
  >
>"#;

        let (out, _) = Object::deserialize(example, 0).map_err(|e| eyre!("{e:#?}"))?;
        assert_eq!(out, "");
        Ok(())
    }}
