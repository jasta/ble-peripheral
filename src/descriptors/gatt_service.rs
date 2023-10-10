use crate::descriptors::gatt_characteristic::GattCharacteristic;
use crate::descriptors::uuid::UUID;

#[derive(Debug, PartialEq)]
pub struct GattService<'a> {
  pub uuid: UUID,
  pub service_type: GattServiceType,
  pub characteristics: &'a [GattCharacteristic<'a>],
}

impl<'a> Default for GattService<'a> {
  fn default() -> Self {
    Self {
      uuid: UUID::Long(0),
      service_type: GattServiceType::Primary,
      characteristics: &[],
    }
  }
}

#[derive(Debug, Copy, Clone, PartialEq, Eq)]
pub enum GattServiceType {
  Primary,
  Secondary,
}
