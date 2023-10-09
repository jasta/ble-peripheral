#![no_std]

#[cfg(feature = "alloc")]
extern crate alloc;

pub mod att_error;
pub mod bluetooth_address;
pub mod bluetooth_error;
pub mod descriptors;
pub mod gap_advertiser;
pub mod gatt_connection;
pub mod gatt_server_cb;
pub mod mtu;
pub mod peripheral;
pub mod advertisement;

pub mod prelude {
  pub use crate::att_error::*;
  pub use crate::bluetooth_address::*;
  pub use crate::bluetooth_error::*;
  pub use crate::descriptors::*;
  pub use crate::gap_advertiser::*;
  pub use crate::gatt_connection::*;
  pub use crate::gatt_server_cb::*;
  pub use crate::peripheral::*;
  pub use crate::advertisement::*;
}
