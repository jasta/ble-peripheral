use core::fmt::Debug;
use core::mem::size_of_val;
use core::ops::Deref;
use core::time::Duration;

use crate::descriptors::UUID;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AdvertisementRequest {
  pub params: AdvertisementParams,
  pub payload: AdvertisementPayload,
  pub scan_response_payload: Option<ScanResponsePayload>,
}

#[derive(Debug, Default, Clone, PartialEq, Eq)]
#[allow(clippy::manual_non_exhaustive)]
pub struct AdvertisementParams {
  /// Can a peer connect to us, and using what mechanism?
  pub connect_mode: ConnectMode,

  /// Minimum advertising interval to be used by the advertising set.  Acceptable values are
  /// in the range [20ms, 10,485s].
  pub interval_min: Option<Duration>,

  /// Maximum advertising interval to be used by the advertising set.  Acceptable values are
  /// in the range [20ms, 10,485s].
  pub interval_max: Option<Duration>,

  // Not using #[non_exhaustive] because it doesn't support construction using
  // `..Default::default()`.
  #[doc(hidden)]
  _non_exhaustive: (),
}

/// Whether and how this peripheral is connectable.
#[derive(Debug, Copy, Clone, PartialEq, Eq)]
pub enum ConnectMode {
  /// Connections from any address are allowed.  If unsure, this is probably what you want to use.
  Undirected = 0b0000,

  /// Connections are allowed only from a specific address.  This address must be specified
  /// in the advertisement payload.
  Directed = 0b0001,

  /// Connections are not allowed.  Often this is done when the maximum number of connections have
  /// been reached (and reverse when disconnects happen) or for passive sensors that include their
  /// readings unencrypted via the advertisement.
  None = 0b0010,
}

impl Default for ConnectMode {
  fn default() -> Self {
    Self::Directed
  }
}

pub type AdvertisementPayloadBuilder = RawAdvertisementBuilder<31>;

/// Helper to facilitate creating correctly structured advertisement PDUs.
#[derive(Debug, Default, Clone)]
pub struct RawAdvertisementBuilder<const N: usize> {
  raw: heapless::Vec<u8, N>,
  flags: Option<u8>,
  has_set_flags: bool,
}

impl<const N: usize> RawAdvertisementBuilder<N> {
  pub fn new() -> Self {
    Default::default()
  }

  /// Set the discover mode.
  pub fn set_discover_mode(mut self, discover_mode: DiscoverMode) -> Self {
    let flags = self.flags.get_or_insert(0);
    *flags = (*flags & DISCOVER_MODE_MASK) | (discover_mode as u8 & DISCOVER_MODE_MASK);
    self
  }

  /// Indicate that Bluetooth Classic (ER/HDR) is _NOT_ supported.
  pub fn set_classic_not_supported(mut self, classic_not_supported: bool) -> Self {
    let flags = self.flags.get_or_insert(0);
    if classic_not_supported {
      *flags |= CLASSIC_NOT_SUPPORTED_MASK;
    } else {
      *flags &= !CLASSIC_NOT_SUPPORTED_MASK;
    }
    self
  }

  pub fn push_manufacturer_data(
    mut self,
    manufacturer_id: u16,
    data: &[u8],
  ) -> Result<Self, PushError> {
    self = self.push_start_record(AdType::ManufacturerData as _, 2 + data.len())?;
    self
      .raw
      .extend_from_slice(&manufacturer_id.to_le_bytes())
      .unwrap();
    self.raw.extend_from_slice(data).unwrap();
    Ok(self)
  }

  pub fn push_service_uuids(mut self, uuids: &[UUID]) -> Result<Self, PushError> {
    let size_of_item = Self::require_equal_size(uuids)?;
    let ad_type = match uuids.first().ok_or(PushError::UuidInputError)? {
      UUID::Short(_) => AdType::ServiceData16,
      UUID::Long(_) => AdType::ServiceData128,
    };

    self = self.push_start_record(ad_type as _, size_of_item * uuids.len())?;
    for uuid in uuids {
      uuid.push_into(&mut self.raw).unwrap();
    }

    Ok(self)
  }

  fn require_equal_size(uuids: &[UUID]) -> Result<usize, PushError> {
    let mut num_bits = None;
    for uuid in uuids {
      let size_of = size_of_val(uuid);
      if num_bits.get_or_insert(size_of) != &size_of {
        return Err(PushError::UuidInputError);
      }
    }
    num_bits.ok_or(PushError::UuidInputError)
  }

  pub fn push_service_data(mut self, uuid: &UUID, data: &[u8]) -> Result<Self, PushError> {
    let (ad_type, size_per_item) = match uuid {
      UUID::Short(_) => (AdType::ServiceData16, 2),
      UUID::Long(_) => (AdType::ServiceData128, 16),
    };

    self = self.push_start_record(ad_type as _, size_per_item + data.len())?;
    uuid.push_into(&mut self.raw).unwrap();
    self.raw.extend_from_slice(data).unwrap();

    Ok(self)
  }

  /// Push the short local name.  It is presumed that this name is either the same or shorter than the
  /// name assigned to the peripheral itself, and UI treatment from external parties will likely
  /// reflect this and update the name after connection is established and the "full" name is
  /// read from the built-in characteristic.  The full name can also be manually pushed using
  /// [AdType::LongLocalName].
  pub fn push_local_name(mut self, name: &str) -> Result<Self, PushError> {
    self = self.push_start_record(AdType::ShortLocalName as _, name.len())?;
    self.raw.extend_from_slice(name.as_bytes()).unwrap();
    Ok(self)
  }

  pub fn push_raw_ad_type(mut self, ad_type: u8, data: &[u8]) -> Result<Self, PushError> {
    self = self.push_start_record(ad_type, data.len())?;
    self.raw.extend_from_slice(data).unwrap();

    if ad_type == AdType::Flags as _ {
      self.has_set_flags = true;
    }

    Ok(self)
  }

  fn push_start_record(mut self, ad_type: u8, remaining_size: usize) -> Result<Self, PushError> {
    self = self.flush_pending_record()?;

    if self.raw.len() + 2 + remaining_size >= N {
      return Err(PushError::CapacityExceeded);
    }

    self
      .raw
      .push(remaining_size.checked_add(1).unwrap().try_into().unwrap())
      .unwrap();
    self.raw.push(ad_type).unwrap();
    Ok(self)
  }

  pub fn build(mut self) -> Result<RawAdvertisement<N>, PushError> {
    self = self.ensure_defaults_set()?;
    self = self.flush_pending_record()?;
    Ok(RawAdvertisement(self.raw))
  }

  fn flush_pending_record(mut self) -> Result<Self, PushError> {
    if let Some(flags) = self.flags.take() {
      self = self.push_raw_ad_type(AdType::Flags as _, &[flags])?;
    }
    Ok(self)
  }

  fn ensure_defaults_set(mut self) -> Result<Self, PushError> {
    if !self.has_set_flags && self.flags.is_none() {
      self = self.set_discover_mode(DiscoverMode::General);
      self = self.set_classic_not_supported(true);
      self.has_set_flags = true;
    }
    Ok(self)
  }
}

/// Advertisements consist of one or more ad type units in a TLV-style format (but actually it's
/// LTV).  Note that this list is not exhaustive but is provided as a convenience.
#[derive(Debug, Copy, Clone, PartialEq, Eq)]
#[repr(u8)]
pub enum AdType {
  Flags = 0x01,
  PartialServiceUuids16 = 0x02,
  CompleteServiceUuids16 = 0x03,
  PartialServiceUuids32 = 0x04,
  CompleteServiceUuids32 = 0x05,
  PartialServiceUuids128 = 0x06,
  CompleteServiceUuids128 = 0x07,
  ShortLocalName = 0x08,
  LongLocalName = 0x09,
  TxPowerLevel = 0x0a,
  DeviceClass = 0x0d,
  ServiceData16 = 0x16,
  PublicTargetAddress = 0x17,
  RandomTargetAddress = 0x18,
  Appearance = 0x19,
  AdvertisingInterval = 0x1a,
  LeBluetoothDeviceAddress = 0x1b,
  LeRole = 0x1c,
  ServiceData32 = 0x20,
  ServiceData128 = 0x21,
  Uri = 0x24,
  ManufacturerData = 0xff,
}

#[derive(Debug)]
pub enum PushError {
  CapacityExceeded,
  UuidInputError,
}

const DISCOVER_MODE_MASK: u8 = 0b0000_0011;
const CLASSIC_NOT_SUPPORTED_MASK: u8 = 0b0000_0100;

/// Whether and how this peripheral is discovered.
#[derive(Debug, Copy, Clone, PartialEq, Eq)]
#[repr(u8)]
pub enum DiscoverMode {
  /// This device can only be discovered when a central device is following the limited
  /// discovery procedure.
  Limited = 0b0000_0001,

  /// General discovery.  This is the normal discovery mode that most customers would use.
  General = 0b0000_0010,

  /// Device is not discoverable (whether the device is connectable is determined independently).
  None = 0b0000_0000,
}

/// Represents the raw payload for an advertisement.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RawAdvertisement<const N: usize>(pub heapless::Vec<u8, N>);

impl<const N: usize> Deref for RawAdvertisement<N> {
  type Target = [u8];

  fn deref(&self) -> &Self::Target {
    &self.0
  }
}

pub type AdvertisementPayload = RawAdvertisement<31>;
pub type ScanResponsePayload = RawAdvertisement<31>;

#[cfg(test)]
mod tests {
  extern crate std;
  extern crate alloc;

  use alloc::vec;
  use alloc::vec::Vec;
  use std::io::Cursor;
  use std::io::Read;

  use byteorder::ReadBytesExt;

  use super::*;

  #[test]
  pub fn test_flags_default_encoding() {
    let adv = AdvertisementPayloadBuilder::new().build().unwrap();

    assert_eq!(adv.0, [0x02, 0x01, 0x06]);
  }

  #[test]
  pub fn test_flags_non_default_encoding() {
    let adv = AdvertisementPayloadBuilder::new()
      .set_discover_mode(DiscoverMode::Limited)
      .set_classic_not_supported(true)
      .build()
      .unwrap();

    assert_eq!(adv.0, [0x02, 0x01, 0b0000_0101]);
  }

  #[test]
  pub fn test_service_data() {
    let short_uuid = 0x1234;
    let short_data = [0x1];
    let long_uuid = 0x0123456789abcdef;
    let long_data = [0x2];

    let adv = AdvertisementPayloadBuilder::new()
      .push_service_data(&UUID::Short(short_uuid), &short_data)
      .unwrap()
      .push_service_data(&UUID::Long(long_uuid), &long_data)
      .unwrap()
      .build()
      .unwrap();

    let mut iter = AdRecordIter::new(&adv.0);
    let short_record_data = concat(&short_uuid.to_le_bytes(), &short_data);
    assert_eq!(
      iter.next(),
      Some(AdRecord::new(AdType::ServiceData16, &short_record_data))
    );
    let long_record_data = concat(&long_uuid.to_le_bytes(), &long_data);
    assert_eq!(
      iter.next(),
      Some(AdRecord::new(AdType::ServiceData128, &long_record_data))
    );
  }

  fn concat<T: Clone>(a: &[T], b: &[T]) -> Vec<T> {
    [a.to_vec(), b.to_vec()].into_iter().flatten().collect()
  }

  #[test]
  pub fn test_manufacturer_data() {
    let mfg_id = 0x1234;
    let mfg_data = "manufacturer".as_bytes();

    let adv = AdvertisementPayloadBuilder::new()
      .push_manufacturer_data(mfg_id, mfg_data)
      .unwrap()
      .build()
      .unwrap();

    let mut iter = AdRecordIter::new(&adv.0);
    let record_data = concat(&mfg_id.to_le_bytes(), mfg_data);
    assert_eq!(
      iter.next(),
      Some(AdRecord::new(AdType::ManufacturerData, &record_data))
    );
  }

  struct AdRecordIter<'a> {
    cursor: Cursor<&'a [u8]>,
  }

  impl<'a> AdRecordIter<'a> {
    pub fn new(data: &'a [u8]) -> Self {
      Self {
        cursor: Cursor::new(data),
      }
    }
  }

  impl<'a> Iterator for AdRecordIter<'a> {
    type Item = AdRecord;

    fn next(&mut self) -> Option<Self::Item> {
      let length = self.cursor.read_u8().ok()?;
      if length < 1 {
        return None;
      }
      let ad_type = self.cursor.read_u8().ok()?;
      let mut data = vec![0u8; (length - 1).into()];
      self.cursor.read_exact(&mut data).ok()?;

      Some(AdRecord { ad_type, data })
    }
  }

  #[derive(Debug, Clone, PartialEq, Eq)]
  struct AdRecord {
    ad_type: u8,
    data: Vec<u8>,
  }

  impl AdRecord {
    pub fn new(ad_type: AdType, data: &[u8]) -> Self {
      Self {
        ad_type: ad_type as _,
        data: data.to_vec(),
      }
    }
  }
}
