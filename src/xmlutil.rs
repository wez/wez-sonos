use crate::upnp::DecodeXml;
use instant_xml::{Deserializer, FromXml, Id, Kind};

/// This is a wrapper container that can be used to adapt a
/// scalar embedded xml string value into a more rich Rust
/// type representation.
#[derive(Debug, PartialEq, Clone)]
pub struct DecodeXmlString<T: DecodeXml>(pub T);

impl<'xml, T> FromXml<'xml> for DecodeXmlString<T>
where
    T: DecodeXml,
{
    #[inline]
    fn matches(id: Id<'_>, field: Option<Id<'_>>) -> bool {
        match field {
            Some(field) => id == field,
            None => false,
        }
    }

    fn deserialize<'cx>(
        target: &mut <Self as FromXml<'_>>::Accumulator,
        field: &'static str,
        deserializer: &mut Deserializer<'cx, '_>,
    ) -> std::result::Result<(), instant_xml::Error> {
        if target.is_some() {
            return Err(instant_xml::Error::DuplicateValue);
        }

        match deserializer.take_str()? {
            Some(value) => {
                let parsed = T::decode_xml(&value).map_err(|err| {
                    instant_xml::Error::Other(format!(
                        "failed to decode_xml for {field}: `{err:#}` {value}"
                    ))
                })?;
                target.replace(DecodeXmlString(parsed));
                Ok(())
            }
            None => Err(instant_xml::Error::MissingValue(field)),
        }
    }

    type Accumulator = Option<Self>;
    // We appear to be a string in the doc
    const KIND: Kind = Kind::Scalar;
}

impl<T: DecodeXml> DecodeXmlString<T> {
    pub fn into_inner(self) -> T {
        self.0
    }
}

impl<T: DecodeXml> std::ops::Deref for DecodeXmlString<T> {
    type Target = T;
    fn deref(&self) -> &T {
        &self.0
    }
}
