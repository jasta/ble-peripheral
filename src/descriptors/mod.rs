pub mod attribute_handle;
pub mod gatt_characteristic;
pub mod gatt_descriptor;
/// Descriptors just define the structure and data types of services/characteristics/etc.  These
/// are pure data types and do not have functional APIs.
pub mod gatt_service;
pub mod uuid;

pub use attribute_handle::*;
pub use gatt_characteristic::*;
pub use gatt_descriptor::*;
pub use gatt_service::*;
pub use uuid::*;
