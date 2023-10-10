#[derive(Debug, Copy, Clone, PartialEq, Eq)]
#[repr(u8)]
pub enum AttError {
  InvalidHandle = 0x01,
  ReadNotPermitted = 0x02,
  WriteNotPermitted = 0x03,
  InvalidPdu = 0x04,
  InsufficientAuthentication = 0x05,
  RequestNotSupported = 0x06,
  InvalidOffset = 0x07,
  InsufficientAuthorization = 0x08,
  PrepareQueueFull = 0x09,
  AttributeNotFound = 0x0A,
  AttributeTooLong = 0x0B,
  InsufficientKeySize = 0x0C,
  InvalidAttributeValueLength = 0x0D,
  Unlikely = 0x0E,
  InsufficientEncryption = 0x0F,
  UnsupportedGroupType = 0x10,
  InsufficientResources = 0x11,
}
