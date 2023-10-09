use crate::advertisement::{AdvertisementRequest};

pub trait GapAdvertiser {
  /// Request that BLE advertising begins, which is necessary to allow incoming connections (and using
  /// an advertisement that is connectable).  Note that
  /// advertising is typically automatically disabled when a GAP connection is received by
  /// [crate::prelude::GattServerCallback].  Some controllers allow re-enabling advertising which would allow
  /// another incoming connection.
  ///
  /// More information about start/stop reasons will be delivered asynchronously through
  /// the callback provided when the GATT server was configured.
  fn request_start(&self, request: AdvertisementRequest);

  /// Request that BLE advertising is stopped.  It is expected that this action is confirmed
  /// through [crate::prelude::GattServerCallback].
  fn request_stop(&self);
}
