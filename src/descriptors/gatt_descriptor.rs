use enumset::EnumSet;
use crate::descriptors::uuid::UUID;
use crate::prelude::GattCharacteristicPermission;

#[derive(Debug, PartialEq)]
pub struct GattDescriptor {
  pub uuid: UUID,
  pub permissions: EnumSet<GattDescriptorPermission>,
}

impl Default for GattDescriptor {
  fn default() -> Self {
    Self {
      uuid: UUID::Long(0),
      permissions: EnumSet::new(),
    }
  }
}

pub type GattDescriptorPermission = GattCharacteristicPermission;
