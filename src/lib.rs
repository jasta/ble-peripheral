#![no_std]

extern crate alloc;

pub mod gatt_connection;
pub mod descriptors;
pub mod peripheral;
pub mod bluetooth_error;
pub mod gap_advertiser;
pub mod gatt_server_cb;
pub mod att_error;
pub mod bluetooth_address;
pub mod mtu;

pub mod prelude {
  pub use crate::descriptors::*;
  pub use crate::gap_advertiser::*;
  pub use crate::gatt_server_cb::*;
  pub use crate::peripheral::*;
  pub use crate::gatt_connection::*;
  pub use crate::bluetooth_error::*;
  pub use crate::att_error::*;
  pub use crate::bluetooth_address::*;
}
