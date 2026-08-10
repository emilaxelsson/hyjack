pub type InstrumentIndex = u8;
pub type InstrumentName = String;

pub enum Instruments {
  DirectMapped(Vec<InstrumentName>),
}

impl Instruments {
  pub fn get_name(&self, index: InstrumentIndex) -> Option<InstrumentName> {
    match self {
      Instruments::DirectMapped(names) => names.get(index as usize).cloned(),
    }
  }
}
