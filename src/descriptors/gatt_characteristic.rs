use crate::descriptors::gatt_descriptor::GattDescriptor;
use crate::descriptors::uuid::UUID;
use enumset::EnumSet;

#[derive(Debug, PartialEq)]
pub struct GattCharacteristic<'a> {
  pub uuid: UUID,
  pub properties: EnumSet<GattCharacteristicProperty>,
  pub permissions: EnumSet<GattCharacteristicPermission>,
  pub descriptors: &'a [GattDescriptor],
}

impl<'a> Default for GattCharacteristic<'a> {
  fn default() -> Self {
    Self {
      uuid: UUID::Long(0),
      properties: EnumSet::new(),
      permissions: EnumSet::new(),
      descriptors: &[],
    }
  }
}

#[derive(Debug, enumset::EnumSetType)]
pub enum GattCharacteristicProperty {
  Broadcast,
  ExtendedProps,

  /// Note that setting this property will cause a CCCD descriptor to automatically be added
  /// to the characteristic.
  Indicate,

  /// Note that setting this property will cause a CCCD descriptor to automatically be added
  /// to the characteristic.
  Notify,

  Read,
  Write,
  WriteSigned,
  WriteNoResponse,
}

#[derive(Debug, enumset::EnumSetType)]
pub enum GattCharacteristicPermission {
  Read,
  ReadEncrypted,
  Write,
  WriteEncrypted,
  WriteEncryptedMitm,
  WriteSigned,
  WriteSignedMitm,
}
