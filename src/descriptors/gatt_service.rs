use crate::descriptors::gatt_characteristic::GattCharacteristic;
use crate::descriptors::uuid::UUID;
use alloc::vec::Vec;

#[derive(Debug, PartialEq)]
pub struct GattService {
  pub uuid: UUID,
  pub service_type: GattServiceType,
  pub characteristics: Vec<GattCharacteristic>,
}

impl Default for GattService {
  fn default() -> Self {
    Self {
      uuid: UUID::Long(0),
      service_type: GattServiceType::Primary,
      characteristics: Vec::new(),
    }
  }
}

#[derive(Debug, Copy, Clone, PartialEq, Eq)]
pub enum GattServiceType {
  Primary,
  Secondary,
}
