use crate::upnp::{DecodeXml, EncodeXml};
use instant_xml::{Deserializer, FromXml, Id, Kind, ToXml};

/// This is a wrapper container that can be used to adapt a
/// scalar embedded xml string value into a more rich Rust
/// type representation.
#[derive(Debug, PartialEq, Clone, Default)]
pub struct DecodeXmlString<T>(pub Option<T>)
where
    T: DecodeXml;

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
            return Err(instant_xml::Error::DuplicateValue(field));
        }

        match deserializer.take_str()? {
            Some(value) => {
                // eprintln!("decode: {value}");

                let is_empty = value.trim().is_empty() || value == "NOT_IMPLEMENTED";

                if !is_empty {
                    let parsed = T::decode_xml(&value).map_err(|err| {
                        instant_xml::Error::Other(format!(
                            "failed to decode_xml for {field}: `{err:#}` {value}"
                        ))
                    })?;
                    target.replace(DecodeXmlString(Some(parsed)));
                }
                Ok(())
            }
            None => {
                // There is no value
                Ok(())
            }
        }
    }

    type Accumulator = Option<Self>;
    // We appear to be a string in the doc
    const KIND: Kind = Kind::Scalar;
}

impl<T> ToXml for DecodeXmlString<T>
where
    T: DecodeXml,
    T: EncodeXml,
{
    fn serialize<W>(
        &self,
        id: Option<Id<'_>>,
        serializer: &mut instant_xml::Serializer<'_, W>,
    ) -> std::result::Result<(), instant_xml::Error>
    where
        W: std::fmt::Write + ?Sized,
    {
        let encoded = match &self.0 {
            Some(inner) => inner.encode_xml()?,
            None => String::new(),
        };
        encoded.serialize(id, serializer)
    }
}

impl<T: DecodeXml> DecodeXmlString<T> {
    pub fn into_inner(self) -> Option<T> {
        self.0
    }
}

impl<T: DecodeXml> std::ops::Deref for DecodeXmlString<T> {
    type Target = Option<T>;
    fn deref(&self) -> &Option<T> {
        &self.0
    }
}

impl<T: DecodeXml> From<T> for DecodeXmlString<T> {
    fn from(value: T) -> DecodeXmlString<T> {
        DecodeXmlString(Some(value))
    }
}

impl<T: DecodeXml> From<Option<T>> for DecodeXmlString<T> {
    fn from(value: Option<T>) -> DecodeXmlString<T> {
        DecodeXmlString(value)
    }
}
