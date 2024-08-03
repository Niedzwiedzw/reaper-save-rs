use crate::low_level::{self, AttributeName, Entry, Object, SerializeAndDeserialize};
use derive_more::{AsMut, AsRef};

pub mod error;
use error::Result;

fn assert_attribute_name(object: Object, attribute_name: &str) -> Result<Object> {
    object
        .header
        .attribute
        .as_ref()
        .eq(attribute_name)
        .then(|| object.clone())
        .ok_or_else(|| error::Error::InvalidObject {
            expected: AttributeName::new(attribute_name.to_owned()),
            got: object.header.attribute.clone(),
        })
}

pub trait ObjectWrapper: Sized {
    const ATTRIBUTE_NAME: &'static str;
    fn from_object_raw(inner: Object) -> Self;
    fn from_object(inner: Object) -> error::Result<Self> {
        assert_attribute_name(inner, Self::ATTRIBUTE_NAME).map(Self::from_object_raw)
    }
}

macro_rules! debug_impl {
    ($ty:ty) => {
        impl std::fmt::Debug for $ty {
            fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
                match self.inner.serialize_inline() {
                    Ok(serialized) => write!(f, "{}:\n{serialized}", std::any::type_name::<Self>()),
                    Err(_) => self.inner.fmt(f),
                }
            }
        }
    };
}

debug_impl!(ReaperProject);
debug_impl!(Track);
debug_impl!(Item);

impl ObjectWrapper for ReaperProject {
    const ATTRIBUTE_NAME: &'static str = "REAPER_PROJECT";

    fn from_object_raw(inner: Object) -> Self {
        Self { inner }
    }
}

#[derive(PartialEq, Eq, Clone, AsMut, AsRef)]
pub struct ReaperProject {
    inner: Object,
}

impl ReaperProject {
    pub fn parse_from_str(input: &str) -> Result<Self> {
        low_level::from_str(input)
            .map_err(Into::into)
            .and_then(Self::from_object)
    }
    pub fn serialize_to_string(self) -> Result<String> {
        low_level::to_string(self.inner).map_err(Into::into)
    }
    pub fn tracks(&self) -> Vec<Track> {
        self.inner
            .values
            .iter()
            .filter_map(|e| e.as_object())
            .cloned()
            .filter_map(|o| Track::from_object(o).ok())
            .collect()
    }

    pub fn modify_tracks<F: FnOnce(Vec<Track>) -> Vec<Track>>(
        &mut self,
        modifier: F,
    ) -> Result<()> {
        let value_index = || self.inner.values.iter().enumerate();
        let original_index_start = value_index()
            .find_map(|(index, entry)| entry.as_object().map(|_| index))
            .or_else(|| value_index().last().map(|(index, _)| index))
            .ok_or(error::Error::EmptyProject)?;
        let mut values = self.inner.values.clone();
        let popped_tracks = {
            values
                .extract_if(|val| {
                    val.as_object()
                        .and_then(|inner| Track::from_object(inner.clone()).ok())
                        .is_some()
                })
                .map(|inner| {
                    inner
                        .as_object()
                        .cloned()
                        .map(|inner| Track::from_object(inner).expect("this was checked above"))
                        .expect("this was also checked above")
                })
                .collect::<Vec<_>>()
        };
        let new_tracks = modifier(popped_tracks);
        new_tracks.into_iter().rev().for_each(|track| {
            values.insert(original_index_start, Entry::Object(track.inner));
        });

        self.inner.values = values;

        Ok(())
    }
}

impl ObjectWrapper for Track {
    const ATTRIBUTE_NAME: &'static str = "TRACK";

    fn from_object_raw(inner: Object) -> Self {
        Self { inner }
    }
}

#[derive(PartialEq, Eq, Clone, AsMut, AsRef)]
pub struct Track {
    inner: Object,
}

impl Track {
    pub fn name(&self) -> Result<String> {
        const NAME: &str = "NAME";
        self.inner
            .values
            .iter()
            .find_map(|entry| {
                entry
                    .as_line()
                    .and_then(|line| line.attribute.as_ref().eq(NAME).then_some(&line.values))
            })
            .and_then(|values| values.iter().next())
            .ok_or_else(|| error::Error::MissingAttribute {
                attribute: AttributeName::new(NAME.to_owned()),
            })
            .and_then(|attribute| attribute.serialize_inline().map_err(Into::into))
    }
}

#[derive(PartialEq, Eq, Clone, AsMut, AsRef)]
pub struct Item {
    inner: Object,
}

#[cfg(test)]
mod tests {
    use super::*;
    const EXAMPLE_1: &str = include_str!("../test_data/barbarah-anne.rpp");

    #[test]
    fn test_extract_tracks() -> Result<()> {
        let reaper_project = ReaperProject::parse_from_str(EXAMPLE_1)?;
        for (idx, track) in reaper_project.tracks().into_iter().enumerate() {
            println!("{}. {:?}", idx + 1, track);
        }

        Ok(())
    }
}
