#[derive(Debug, Copy, Clone, PartialEq, Eq)]
pub enum UUID {
  /// For use only with SIG defined services (i.e. registered and publicly well known services).
  Short(u16),

  /// All other BLE UUIDs must be 128-bit
  Long(u128),
}

impl UUID {
  pub fn as_u128(&self) -> u128 {
    match *self {
      UUID::Short(u) => u.into(),
      UUID::Long(u) => u,
    }
  }
}
