use super::*;

pub(crate) type Map<K, V> = std::collections::BTreeMap<K, V>;
pub(crate) use std::collections::HashMap;
pub(crate) use std::collections::HashSet as Set;
pub(crate) use std::fmt::{self, Display, Formatter};

pub mod error;
pub use error::{Error, WithSourceCode};