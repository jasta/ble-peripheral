use core::fmt::Debug;
use crate::descriptors::gatt_service::GattService;
use crate::gap_advertiser::GapAdvertiser;
use crate::gatt_connection::GattConnection;
use crate::gatt_server_cb::GattServerCallback;

/// Trait defining the capabilities of a BLE peripheral device, typically found on embedded
/// devices.  For more information, see:
/// https://embeddedcentric.com/lesson-2-ble-profiles-services-characteristics-device-roles-and-network-topology/
///
/// Note that this trait is meant to be a low-level mapping to common BLE libraries and not
/// ergonomic and safe for Rust users.
///
/// The implementation should be embedded friendly as it is common for BLE peripherals to run on
/// embedded platforms.
pub trait Peripheral {
  type SystemError: Debug;

  type Handle;
  type Advertiser: GapAdvertiser + Debug + Clone;
  type Connection: GattConnection + Debug;

  /// Sets the device name that can be read from built-in GATT characteristics after connection,
  /// and a separate, shorter version can be sent through the advertisement packet.  There may
  /// be different limits imposed by each platform.
  fn set_name(&mut self, name: &str) -> Result<(), Self::SystemError>;

  /// Sets the device appearance (generally this refers to how the device should be visually
  /// or functionally presented to a user).  This data is presented upon connection using
  /// mandatory built-in GATT characteristics.
  fn set_appearance(&mut self, appearance: u16) -> Result<(), Self::SystemError>;

  /// Configure a new GATT server on this peripheral.  Note that only one GATT
  /// server may be configured for each peripheral but this server may contain multiple distinct
  /// GATT services.  Connections will not be accepted until [GapAdvertiser::request_start] is
  /// called and the advertisement explicitly enables connectability.
  ///
  /// The entire GATT server application is expected to be driven through the event callback.
  /// Dropping the resulting handle will shutdown the server.
  fn configure_gatt_server(
      self,
      services: &[GattService],
      callback: impl GattServerCallback<Self> + 'static,
  ) -> Result<Self::Handle, Self::SystemError>;
}
