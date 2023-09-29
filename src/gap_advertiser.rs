use alloc::collections::BTreeMap;
use alloc::vec::Vec;
use core::fmt::Debug;

pub trait GapAdvertiser {
  /// Request that BLE advertising begins, which is necessary to allow incoming connections (and using
  /// an advertisement that is connectable).  Note that
  /// advertising is typically automatically disabled when a GAP connection is received by
  /// [crate::prelude::GattServerCallback].  Some controllers allow re-enabling advertising which would allow
  /// another incoming connection.
  ///
  /// More information about start/stop reasons will be delivered asynchronously through
  /// the callback provided when the GATT server was configured.
  fn request_start(&self, advertisement: Advertisement);

  /// Request that BLE advertising is stopped.  It is expected that this action is confirmed
  /// through [crate::prelude::GattServerCallback].
  fn request_stop(&self);
}

#[derive(Debug)]
pub struct Advertisement {
  pub is_connectable: bool,
  pub is_discoverable: bool,
  pub manufacturer_data: BTreeMap<u16, Vec<u8>>,
  // TODO: THE REST AND WITH A BETTER API!
}
