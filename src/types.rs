use smallvec::SmallVec;
use fnv::FnvHashMap;
use smallstring::SmallString;

pub type Array = SmallVec<[Value; 4]>;
pub type Identifier = SmallString;
pub type Value = String;
pub type VariableContext = FnvHashMap<Identifier, Value>;
pub type ArrayVariableContext = FnvHashMap<Identifier, Array>;
