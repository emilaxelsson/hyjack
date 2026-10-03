use serde::{Deserialize, Serialize};

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(transparent)]
pub struct InstrumentIndex(pub u8);

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(transparent)]
pub struct InstrumentName(pub String);

#[derive(Serialize, Deserialize)]
pub enum Instruments {
  DirectMapped(Vec<InstrumentName>),
}

impl Instruments {
  pub fn get_name(&self, index: InstrumentIndex) -> Option<InstrumentName> {
    match self {
      Instruments::DirectMapped(names) => names.get(index.0 as usize).cloned(),
    }
  }

  pub fn max_index(&self) -> Option<InstrumentIndex> {
    match self {
      Instruments::DirectMapped(names) => {
        if names.is_empty() {
          None
        } else {
          Some(InstrumentIndex(
            (names.len() - 1).min(u8::MAX as usize) as u8
          ))
        }
      }
    }
  }
}
