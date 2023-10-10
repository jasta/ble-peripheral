use core::fmt::{Display, Formatter};

/// Holder type to clarify a frequent gotcha with BLE around the true ATT MTU size.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord)]
pub struct Mtu {
  negotiated_mtu: u16,
}

impl Display for Mtu {
  fn fmt(&self, f: &mut Formatter<'_>) -> core::fmt::Result {
    write!(f, "{:?}", self.negotiated_mtu)
  }
}

impl Mtu {
  pub fn new(negotiated_mtu: u16) -> Self {
    Self { negotiated_mtu }
  }

  /// Return the actual negotiated value for the _total_ BLE payload, which doesn't take into
  /// account the ATT header (which is 3 bytes).  Most customers should avoid this method as user
  /// provided payloads at this size will be truncated.
  pub fn negotiated_value(&self) -> u16 {
    self.negotiated_mtu
  }

  /// Return the usable maximum payload size for writes.  This is the size at which packets
  /// must be fragmented by the caller.  To be extremely clear: this is the actual maximum size
  /// of a user initiated write that will be allowed without truncation.
  pub fn usable_value(&self) -> u16 {
    self.negotiated_mtu - 3
  }
}
