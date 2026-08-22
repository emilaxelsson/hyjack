use serde::{Deserialize, Serialize};

pub type InstrumentIndex = u8;
pub type InstrumentName = String;

#[derive(Serialize, Deserialize)]
pub enum Instruments {
  DirectMapped(Vec<InstrumentName>),
}

impl Instruments {
  pub fn get_name(&self, index: InstrumentIndex) -> Option<InstrumentName> {
    match self {
      Instruments::DirectMapped(names) => names.get(index as usize).cloned(),
    }
  }

  pub fn max_index(&self) -> Option<InstrumentIndex> {
    match self {
      Instruments::DirectMapped(names) => {
        if names.is_empty() {
          None
        } else {
          Some((names.len() - 1).min(InstrumentIndex::MAX as usize) as InstrumentIndex)
        }
      }
    }
  }
}
