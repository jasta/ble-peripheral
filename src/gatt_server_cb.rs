use crate::bluetooth_error::BluetoothError;
use crate::descriptors::{AttributeHandle, UUID};
use crate::gatt_connection::GattConnection;
use crate::mtu::Mtu;
use crate::peripheral::Peripheral;
use core::fmt::Debug;

pub trait GattServerCallback<P: Peripheral + ?Sized> {
  fn on_event(&mut self, event: GattServerEvent<'_, P>);
}

impl<P, F> GattServerCallback<P> for F
where
  F: FnMut(GattServerEvent<P>),
  P: Peripheral,
{
  fn on_event(&mut self, event: GattServerEvent<'_, P>) {
    (self)(event)
  }
}

#[derive(Debug)]
pub enum GattServerEvent<'a, P: Peripheral + ?Sized> {
  /// Internally generated event when the server has been configured.  This mechanism provides
  /// the callback with a handle to the advertiser object so that both external events (like
  /// the user pushing a button) and internal events (like when a BLE device disconnects) can
  /// be used to control advertising.
  ServerStarted {
    /// Advertiser which must be used to with a connectable advertisement in order to
    /// become open for incoming connections.
    advertiser: P::Advertiser,

    /// Mapping of UUIDs to attribute handles which will be used for subsequent events.  Note
    /// that some UUIDs in the provided services specification may be omitted if it can be
    /// determined that there will be no callback events using them.
    handle_mapping: &'a [(UUID, AttributeHandle)],
  },

  /// Server has either spuriously shutdown or configure failed to start the server asynchronously.
  /// This is usually related to programmer error misconfiguration of the bluetooth implementation
  /// in some way.  All future callback events will stop and the advertiser will no longer function.
  ServerShutdown { error: P::SystemError },

  /// Advertising has started.  New connections will now be accepted.
  AdvertisingStarted {
    /// A hint if available of the total number of connections remaining before our capacity
    /// is reached.
    remaining_connections: Option<u16>,
  },

  /// Advertising has stopped by developer request or an some external event.  New connections
  /// will not be accepted until advertising is started again and the connectable flag is set.
  /// Advertising can be restarted again using heuristics based on the stop reason.
  AdvertisingStopped { reason: AdvStopReason },

  /// An attempt to start advertising has failed.  It may be retryable based on the supplied
  /// reason and implementation behaviour.
  AdvertisingStartFail {
    reason: &'a AdvStartFailedReason<'a, P::SystemError>,
  },

  /// Peer connected.  Note that this disables advertising automatically!  For stacks that support
  /// concurrent connections, it must be re-enabled by interacting with [crate::GapAdvertiser]!
  Connected { connection: &'a P::Connection },

  /// Peer disconnected.  Note that this re-enables advertising automatically!  If this isn't
  /// desired, calls must be made to [crate::GapAdvertiser]!
  Disconnected {
    connection: &'a P::Connection,
    reason: BluetoothError,
  },

  /// MTU negotiation has completed and a new MTU value should be used.
  MtuChanged {
    connection: &'a P::Connection,

    /// New MTU value, all future writes should respect this value.  Note that the BLE standard
    /// requires that ATT writes actually observe `mtu - 3` as the limit, including all writes
    /// made available through this crate's APIs.
    mtu: Mtu,
  },

  /// Issue a read request for either a characteristic or descriptor.  Callers should compare the
  /// provided `handle` field with the value retrieved from [crate::GattCharacteristic] or
  /// [crate::GattDescriptor].
  ReadRequest {
    connection: &'a P::Connection,
    handle: AttributeHandle,

    /// Reference to a responder that is used to issue a response to the peer.
    responder: &'a mut <P::Connection as GattConnection>::Responder,
  },

  /// Issue a write request for either a characteristic or descriptor.  Callers should compare
  /// the provided `handle` field with the value retrieved from [crate::GattCharacteristic] or
  /// [crate::GattDescriptor].
  WriteRequest {
    connection: &'a P::Connection,
    handle: AttributeHandle,

    /// Reference to a responder that is used to issue a response to the peer.  Optional
    /// in this case since writes can be issued in a way that does not request (or warrant) a
    /// response.
    responder: Option<&'a mut <P::Connection as GattConnection>::Responder>,

    /// Indicates if this write operation is part of a larger sequence of writes that should be
    /// buffered somehow and applied once the final write is issued with
    /// [GattServerEvent::ExecuteWrite].
    action: WriteAction,

    /// Offset within the intended record to apply the written data.  This is often combined with
    /// `is_prepare_write` below to atomically commit a write across multiple GATT exchanges.
    offset: u16,

    /// Actual value that the client is writing.
    value: &'a [u8],
  },

  /// Atomically commit a write combining multiple separate write requests where `is_prepare_write`
  /// was true.
  ExecuteWrite {
    connection: &'a P::Connection,
    handle: AttributeHandle,

    /// Reference to a responder that is used to issue a response to the peer.
    responder: &'a mut <P::Connection as GattConnection>::Responder,

    /// Describes whether we actually will be committing or not.
    action: ExecWriteAction,
  },

  /// A particular connection is interested in receiving updates to the specified characteristic.
  /// Connections should be cloned to manage dispatching writes on the characteristic when they
  /// are observed.  Care must be taken to track subscriptions manually and to handle unsubscribe
  /// and disconnect events correctly.
  ///
  /// This is typically implemented by using the Client Characteristic Configuration Descriptor
  /// (CCCD) which is written to by the client to subscribe/unsubscribe.  Some implementations
  /// hide this transparently and emit events compatible with the Subscribe/Unsubscribe semantics
  /// here while others require the user to define the CCCD characteristics themselves and manually
  /// handle the writes to it.  The expectation is that implementations of
  /// [crate::prelude::Peripheral] do not need to worry about this and can simply the property
  /// [crate::prelude::GattCharacteristicProperty::Notify] or
  /// [crate::prelude::GattCharacteristicProperty::Indicate] to get the Subscribe/Unsubscribe
  /// behaviour.
  Subscribe {
    connection: &'a P::Connection,
    handle: AttributeHandle,

    writer: &'a mut <P::Connection as GattConnection>::Writer,
  },

  /// The peer is no longer interested in receiving updates to the specified characteristic.
  /// See notes on [GattServerEvent::Subscribe]!
  Unsubscribe {
    connection: &'a P::Connection,
    handle: AttributeHandle,
  },
}

#[derive(Debug, Clone)]
pub enum AdvStopReason {
  /// Most BLE stacks automatically disable advertising when an incoming connection is made.  Stacks
  /// that support concurrent connections allow you to start advertising again and look for a
  /// [AdvStopReason::MaxConnectionsReached] result.
  AcceptedConnection,

  /// Developer requested that advertising be stopped.
  Requested,
}

#[derive(Debug, Clone)]
pub enum AdvStartFailedReason<'a, E: Debug> {
  /// Advertisement contained a feature not supported by the current implementation.  Check
  /// the implementation documentation and/or consult the contained error message to learn more.  This
  /// should be considered programmer error.
  UnsupportedFeature(&'a str),

  /// Advertisement specified that connections were allowed but the implementation cannot support
  /// more at this time.  Try again to advertise that we are not connectable or wait for
  /// a disconnected event and try again.
  MaxConnectionsReached,

  /// Uncategorized system error from the implementation.
  SystemError(E),
}

#[derive(Debug, Copy, Clone, PartialEq, Eq)]
pub enum ExecWriteAction {
  /// Commit the prepared writes.
  Commit,

  /// Cancel and cleanup memory associated with the prepared writes.
  Cancel,
}

#[derive(Debug, Copy, Clone, PartialEq, Eq)]
pub enum WriteAction {
  /// A write that should be buffered by the client and committed when ExecuteWrite occurs.
  Prepare,

  /// Normal write.
  Normal,
}
