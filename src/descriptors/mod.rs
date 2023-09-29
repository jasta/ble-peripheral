/// Descriptors just define the structure and data types of services/characteristics/etc.  These
/// are pure data types and do not have functional APIs.
pub mod gatt_service;
pub mod gatt_characteristic;
pub mod uuid;
pub mod gatt_descriptor;
pub mod attribute_handle;

pub use gatt_characteristic::*;
pub use gatt_descriptor::*;
pub use gatt_service::*;
pub use uuid::*;
pub use attribute_handle::*;
