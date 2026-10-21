//! Serialization adapters for the optional checkpoint feature. The simulation performs no
//! I/O; callers choose the format and validate the document at their storage boundary.

/// Serde implements arrays only through length 32. Native skill books and random rings
/// retain their actual fixed sizes instead of becoming resizable just for serialization.
pub mod fixed_array {
    use serde::{Deserialize, Deserializer, Serialize, Serializer};

    pub fn serialize<T: Serialize, S: Serializer, const N: usize>(
        values: &[T; N],
        serializer: S,
    ) -> Result<S::Ok, S::Error> {
        values.as_slice().serialize(serializer)
    }

    pub fn deserialize<'de, T: Deserialize<'de>, D: Deserializer<'de>, const N: usize>(
        deserializer: D,
    ) -> Result<[T; N], D::Error> {
        let values = Vec::<T>::deserialize(deserializer)?;
        values.try_into().map_err(|values: Vec<T>| {
            serde::de::Error::custom(format!("expected {N} values, found {}", values.len()))
        })
    }
}
