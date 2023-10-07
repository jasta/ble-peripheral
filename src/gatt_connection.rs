use crate::att_error::AttError;
use core::fmt::Debug;

use crate::bluetooth_address::BluetoothAddress;

/// Represents a GATT connection from the peripheral perspective, specifically used as the API
/// to allow the BLE peripheral device to respond to connected clients.
pub trait GattConnection {
  type SystemError: Debug;
  type Responder: GattResponder + Debug;
  type Writer: GattWriter + Debug + Clone;

  /// Access the connected peer's Bluetooth address.
  fn peer_address(&self) -> &BluetoothAddress;
}

/// Contextually aware type that manages how requests are mapped to responses internal to the
/// BLE implementation.
pub trait GattResponder {
  type SystemError: Debug;

  /// Send response to read or write request.
  fn respond(&mut self, response: Result<Response<'_>, AttError>) -> Result<(), Self::SystemError>;
}

#[derive(Debug)]
pub struct Response<'a> {
  pub offset: u8,
  pub value: &'a [u8],
}

impl<'a> Response<'a> {
  pub fn new(offset: u8, value: &'a [u8]) -> Self {
    Self { offset, value }
  }

  pub fn complete(value: &'a [u8]) -> Self {
    Self::new(0, value)
  }
}

/// Contextually aware type that manages server-initiated writes.
pub trait GattWriter {
  type SystemError: Debug;

  /// Write to a particular characteristic or descriptor.  This trait should only be
  /// given to customers after server-initiated writes have been enabled by the client.  Note
  /// that the caller doesn't have control over whether this is a notify or indicate write
  /// as the internal state can automatically determine the correct kind.
  ///
  /// Implementation note: it is
  /// expected that implementations automatically handle this behaviour in order to minimize
  /// difficult to debug implementation issues.  See here for more information:
  /// https://www.oreilly.com/library/view/getting-started-with/9781491900550/ch04.html#gatt_feat_si
  fn write(&mut self, value: &[u8]) -> Result<(), Self::SystemError>;
}
