/// Bluetooth errors defined in the protocol.
#[derive(Debug, Copy, Clone, PartialEq, Eq)]
pub enum BluetoothError {
  Timeout,
  AuthFailure,
  PinOrKeyMissing,
  ClosedByPeer(RemoteShutdownReason),
  ClosedLocally,
  ConnectionLimitExceeded,
  PairingWithUnitKeyNotSupported,
  EncryptionNotAcceptable,
  Other(u8),
}

impl From<u8> for BluetoothError {
  fn from(value: u8) -> Self {
    match value {
      0x05 => BluetoothError::Timeout,
      0x06 => BluetoothError::PinOrKeyMissing,
      0x08 => BluetoothError::Timeout,
      0x09 => BluetoothError::ConnectionLimitExceeded,
      0x13 => BluetoothError::ClosedByPeer(RemoteShutdownReason::NoneGiven),
      0x14 => BluetoothError::ClosedByPeer(RemoteShutdownReason::LowResources),
      0x15 => BluetoothError::ClosedByPeer(RemoteShutdownReason::PowerOff),
      0x16 => BluetoothError::ClosedLocally,
      0x25 => BluetoothError::EncryptionNotAcceptable,
      0x29 => BluetoothError::PairingWithUnitKeyNotSupported,
      o => BluetoothError::Other(o),
    }
  }
}

impl From<BluetoothError> for u8 {
  fn from(value: BluetoothError) -> Self {
    match value {
      BluetoothError::AuthFailure => 0x05,
      BluetoothError::PinOrKeyMissing => 0x06,
      BluetoothError::Timeout => 0x08,
      BluetoothError::ConnectionLimitExceeded => 0x09,
      BluetoothError::ClosedByPeer(r) => match r {
        RemoteShutdownReason::NoneGiven => 0x13,
        RemoteShutdownReason::LowResources => 0x14,
        RemoteShutdownReason::PowerOff => 0x15,
      },
      BluetoothError::ClosedLocally => 0x16,
      BluetoothError::EncryptionNotAcceptable => 0x25,
      BluetoothError::PairingWithUnitKeyNotSupported => 0x29,
      BluetoothError::Other(o) => o,
    }
  }
}

#[derive(Debug, Copy, Clone, PartialEq, Eq)]
pub enum RemoteShutdownReason {
  NoneGiven,
  LowResources,
  PowerOff,
}
