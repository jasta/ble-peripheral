use core::fmt::Display;
use core::fmt::Formatter;
use core::num::NonZeroU16;

#[derive(Debug, Copy, Clone, PartialEq, Eq, PartialOrd, Ord)]
pub struct AttributeHandle(pub NonZeroU16);

impl Display for AttributeHandle {
  fn fmt(&self, f: &mut Formatter<'_>) -> core::fmt::Result {
    write!(f, "{:?}", self.0)
  }
}
