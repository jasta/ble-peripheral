use std::collections::{BTreeMap,};
use std::collections::btree_map::Entry;
use std::fmt::Debug;
use enumset::enum_set;
use log::{debug, error, info, trace, warn};
use tokio::io::{AsyncBufReadExt, BufReader};
use tokio::select;
use tokio::sync::oneshot;
use tokio::task::LocalSet;
use ble_peripheral::gatt_server_cb::{GattServerCallback, GattServerEvent};
use ble_peripheral::peripheral::Peripheral;
use ble_peripheral::prelude::*;
use crate::bluer_adapter::{BluerPeripheral};

const ECHO_SERVICE_UUID: UUID = UUID::Long(0xFEEDC0DE);
const ECHO_CHARACTERISTIC_UUID: UUID = UUID::Long(0xF00DC0DE00001);
const MY_MANUFACTURER_ID: u16 = 0xf00d;

#[tokio::main(flavor = "current_thread")]
async fn main() -> Result<(), bluer::Error> {
  let local_set = LocalSet::new();
  local_set.run_until(do_main()).await
}

async fn do_main() -> Result<(), bluer::Error> {
  env_logger::init();
  let session = bluer::Session::new().await?;
  let adapter = session.default_adapter().await?;
  adapter.set_powered(true).await?;

  // Show what it's like to implement entirely against the traits...
  run_server_from_trait(BluerPeripheral::new(adapter)).await
}

async fn run_server_from_trait<P>(mut peripheral: P) -> Result<(), P::SystemError>
where
    P: Peripheral + Debug + 'static,
{
  peripheral.set_name("gatt_server")?;


  let service = GattService {
    uuid: ECHO_SERVICE_UUID,
    characteristics: vec![
      GattCharacteristic {
        uuid: ECHO_CHARACTERISTIC_UUID,
        properties: enum_set!(GattCharacteristicProperty::Read | GattCharacteristicProperty::Write),
        permissions: enum_set!(GattCharacteristicPermission::Read | GattCharacteristicPermission::Write),
        ..Default::default()
      }
    ],
    ..Default::default()
  };

  let (shutdown_tx, mut shutdown_rx) = oneshot::channel();
  let sm = EchoServer::new(shutdown_tx);
  let handle = peripheral.configure_gatt_server(&[service], sm)?;

  println!("Started echo server. Press Enter to exit.");
  let stdin = BufReader::new(tokio::io::stdin());
  let mut lines = stdin.lines();

  // TODO: Why doesn't this actually work to stop when we get an internal error!?!?
  select! {
    _ = lines.next_line() => {},
    _ = &mut shutdown_rx => println!("Internal shutdown, stopping!"),
  }

  // Important to move this to the end so we don't accidentally shut down the server!
  drop(handle);

  Ok(())
}

struct EchoServer<P: Peripheral> {
  shutdown_tx: Option<oneshot::Sender<()>>,
  echo_handle: Option<AttributeHandle>,
  advertiser: Option<P::Advertiser>,
  is_advertising: bool,
  last_value: Vec<u8>,
  writers: WritersManager<<P::Connection as GattConnection>::Writer>,
}

impl<P: Peripheral> EchoServer<P> {
  pub fn new(shutdown_tx: oneshot::Sender<()>) -> Self {
    Self {
      shutdown_tx: Some(shutdown_tx),
      echo_handle: None,
      advertiser: None,
      is_advertising: false,
      last_value: Vec::new(),
      writers: WritersManager::new(),
    }
  }

  fn start_advertising(&self) {
    let mut manufacturer_data = BTreeMap::new();
    manufacturer_data.insert(MY_MANUFACTURER_ID, vec![0x21, 0x22, 0x23, 0x24]);
    let adv = Advertisement {
      is_connectable: true,
      is_discoverable: true,
      manufacturer_data,
    };
    self.advertiser.clone().unwrap().request_start(adv);
  }

  fn set_last_value(&mut self, value: &[u8]) {
    self.last_value.clear();
    self.last_value.extend(value);

    if let Some(writers) = self.writers.lookup(&self.echo_handle.unwrap()) {
      for writer in writers {
        debug!("Updating active listener...");
        if let Err(e) = writer.write(&value) {
          warn!("Error updating listener: {e:?}");
        }
      }
    }
  }
}

impl<P: Peripheral + Debug> GattServerCallback<P> for EchoServer<P> {
  fn on_event(&mut self, event: GattServerEvent<'_, P>) {
    trace!("event: {event:?}");
    match event {
      GattServerEvent::ServerShutdown { error } => {
        error!("Server shutdown: {error:?}");
        let _ = self.shutdown_tx.take().map(|tx| tx.send(()));
      }
      GattServerEvent::ServerStarted { advertiser, handle_mapping } => {
        info!("Server started!");

        for (uuid, handle) in handle_mapping {
          if uuid == ECHO_CHARACTERISTIC_UUID {
            self.echo_handle = Some(handle);
          }
        }

        self.advertiser = Some(advertiser);
        self.start_advertising();
      }
      GattServerEvent::AdvertisingStarted { remaining_connections } => {
        info!("Advertising started");
        debug!("remaining_connections={remaining_connections:?}");
        self.is_advertising = true;
      }
      GattServerEvent::AdvertisingStopped { reason } => {
        info!("Advertising stopped: reason={reason:?}");
        self.is_advertising = false;
      }
      GattServerEvent::AdvertisingStartFail { reason } => {
        error!("Advertising start failed: {reason:?}");
        let _ = self.shutdown_tx.take().map(|tx| tx.send(()));
      }
      GattServerEvent::Connected { connection } => {
        info!("Accepted incoming connection: {connection:?}");
      }
      GattServerEvent::Disconnected { connection, reason } => {
        info!("Disconnection from {connection:?}: {reason:?}");
        if !self.is_advertising {
          self.start_advertising();
        }
      }
      GattServerEvent::MtuChanged { connection, mtu } => {
        info!("MTU changed on {connection:?}: mtu={mtu}");
      }
      GattServerEvent::ReadRequest { connection, handle, responder } => {
        debug!("Got read request on {connection:?}: handle={handle}");
        if handle == self.echo_handle.unwrap() {
          if let Err(e) = responder.respond(Ok(Response::complete(&self.last_value))) {
            error!("Responder failed: {e:?}");
          }
        } else {
          warn!("Unknown handle={handle}");
        }
      }
      GattServerEvent::WriteRequest { connection, handle, responder, action, offset, value } => {
        debug!("Got write request on {connection:?}: handle={handle}, action={action:?}");
        if handle == self.echo_handle.unwrap() {
          if responder.is_some() {
            error!("Shouldn't be a responder here!");
          }
          if offset > 0 {
            warn!("Ignoring write offset={offset}!");
          }
          self.set_last_value(value);
        } else {
          warn!("Unknown handle={handle}");
        }
      }
      GattServerEvent::ExecuteWrite { connection, handle, responder, action } => {
        debug!("Got execute write on {connection:?}: handle={handle}, action={action:?}");
        if handle == self.echo_handle.unwrap() {
          if let Err(e) = responder.respond(Err(AttError::WriteNotPermitted)) {
            error!("Responder failed: {e:?}");
          }
        }
      }
      GattServerEvent::Subscribe { connection, handle, writer } => {
        debug!("Got subscribe on {connection:?}: handle={handle}");
        if handle == self.echo_handle.unwrap() {
          self.writers.register(connection.peer_address().clone(), handle, writer.to_owned());
        }
      }
      GattServerEvent::Unsubscribe { connection, handle } => {
        debug!("Got unsubscribe on {connection:?}: handle={handle}");
        if handle == self.echo_handle.unwrap() {
          self.writers.unregister(connection.peer_address(), &handle);
        }
      }
    }
  }
}

#[derive(Debug, Default, Clone)]
struct WritersManager<W> {
  data: BTreeMap<AttributeHandle, BTreeMap<BluetoothAddress, W>>,
}

impl<W> WritersManager<W> {
  pub fn new() -> Self {
    Self { data: BTreeMap::new() }
  }

  pub fn register(&mut self, address: BluetoothAddress, handle: AttributeHandle, writer: W) -> bool {
    let entries = self.data.entry(handle).or_default();

    match entries.entry(address) {
      Entry::Vacant(e) => {
        e.insert(writer);
        true
      },
      Entry::Occupied(_) => false,
    }
  }

  pub fn unregister(&mut self, address: &BluetoothAddress, handle: &AttributeHandle) {
    if let Some(entries) = self.data.get_mut(handle) {
      entries.remove(address);
    }
  }

  pub fn lookup(&mut self, handle: &AttributeHandle) -> Option<impl Iterator<Item=&mut W>> {
    self.data.get_mut(handle)
        .map(|e| e.values_mut())
  }
}

mod bluer_adapter {
  use std::collections::{BTreeSet};
  use std::fmt::{Debug, Formatter};
  use std::future::Future;
  use std::num::NonZeroU16;
  use std::ops::Deref;
  use std::pin::Pin;
  use std::sync::Arc;
  use bluer::{Adapter, Address, Uuid};
  use bluer::adv::{AdvertisementHandle, Type};
  use bluer::gatt::local::{Application, ApplicationHandle, Characteristic, CharacteristicNotify, CharacteristicNotifyFun, CharacteristicNotifyMethod, CharacteristicRead, CharacteristicReadRequest, CharacteristicWrite, CharacteristicWriteMethod, CharacteristicWriteRequest, Descriptor, DescriptorRead, DescriptorReadRequest, DescriptorWrite, DescriptorWriteRequest, ReqError, ReqResult, Service};
  use futures_util::FutureExt;
  use log::{debug, warn};
  use tokio::sync::{mpsc, oneshot};
  use ble_peripheral::peripheral::Peripheral;
  use ble_peripheral::prelude::*;

  pub struct BluerPeripheral {
    adapter: Adapter,
    ident: DeviceIdent,
  }

  impl BluerPeripheral {
    pub fn new(adapter: Adapter) -> Self {
      Self {
        adapter,
        ident: Default::default(),
      }
    }
  }

  impl Debug for BluerPeripheral {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
      f.debug_struct("BluerPeripheral")
          .field("ident", &self.ident)
          .finish_non_exhaustive()
    }
  }

  impl Peripheral for BluerPeripheral {
    type SystemError = bluer::Error;
    type Handle = BluerHandle;
    type Advertiser = BluerAdvertiser;
    type Connection = BluerConnection;

    fn set_name(&mut self, name: &str) -> Result<(), Self::SystemError> {
      self.ident.name = Some(name.to_owned());
      Ok(())
    }

    fn set_appearance(&mut self, appearance: u16) -> Result<(), Self::SystemError> {
      self.ident.appearance = Some(appearance);
      Ok(())
    }

    fn configure_gatt_server(
        self,
        services: &[GattService],
        callback: impl GattServerCallback<Self> + 'static,
    ) -> Result<Self::Handle, Self::SystemError> {
      let (tx, rx) = mpsc::unbounded_channel();
      let advertiser = BluerAdvertiser {
        tx: tx.clone(),
        service_uuids: Arc::new(services.iter().map(|s| convert_uuid(s.uuid)).collect()),
        ident: Arc::new(self.ident),
      };

      let adapter_for_start = self.adapter.clone();
      let tx_for_start = tx.clone();
      let app_holder = ApplicationFactory::convert(services, tx.clone())?;
      tokio::spawn(async move {
        println!("{:?}", app_holder.app);
        let r = adapter_for_start.serve_gatt_application(app_holder.app).await;
        let _ = tx_for_start.send(Event::OnStartResult(
          r.map(|handle| (handle, advertiser, app_holder.handle_mapping))
        ));
      });

      let adapter_for_loop = self.adapter;
      let tx_for_loop = tx.clone();
      tokio::task::spawn_local(async move {
        run_event_loop(adapter_for_loop, rx, tx_for_loop, callback).await;
      });

      Ok(BluerHandle { tx })
    }
  }

  struct ApplicationFactory {
    tx: mpsc::UnboundedSender<Event>,
    app: Application,
    handle_allocator: HandleAllocator,
  }

  struct ApplicationHolder {
    app: Application,
    handle_mapping: Vec<(UUID, AttributeHandle)>,
  }

  impl ApplicationFactory {
    pub fn convert(services: &[GattService], tx: mpsc::UnboundedSender<Event>) -> Result<ApplicationHolder, bluer::Error> {
      let mut me = ApplicationFactory {
        tx,
        app: Default::default(),
        handle_allocator: HandleAllocator::new(),
      };

      for service_spec in services {
        let service = me.new_service(service_spec)?;
        me.app.services.push(service);
      }
      Ok(ApplicationHolder {
        app: me.app,
        handle_mapping: me.handle_allocator.mapping,
      })
    }

    fn new_service(&mut self, service_spec: &GattService) -> Result<Service, bluer::Error> {
      let mut service = self.new_service_base(service_spec)?;
      for characteristic_spec in &service_spec.characteristics {
        let mut characteristic = self.new_characteristic_base(characteristic_spec)?;
        for descriptor_spec in &characteristic_spec.descriptors {
          let descriptor = self.new_descriptor(descriptor_spec)?;
          characteristic.descriptors.push(descriptor);
        }
        service.characteristics.push(characteristic);
      }
      Ok(service)
    }

    fn new_service_base(&mut self, spec: &GattService) -> Result<Service, bluer::Error> {
      Ok(Service {
        uuid: convert_uuid(spec.uuid),
        handle: None,
        primary: spec.service_type == GattServiceType::Primary,
        ..Default::default()
      })
    }

    fn new_characteristic_base(&mut self, spec: &GattCharacteristic) -> Result<Characteristic, bluer::Error> {
      let handle_type = self.handle_allocator.next(spec.uuid);

      let mut read_op = None;
      let mut write_op = None;
      let mut notify_op = None;

      for prop in spec.properties {
        match prop {
          GattCharacteristicProperty::Indicate |
              GattCharacteristicProperty::Notify => {
            notify_op.get_or_insert_with(|| CharacteristicNotify {
              method: CharacteristicNotifyMethod::Fun(self.new_notify_handler(handle_type)),
              ..Default::default()
            });
          }
          GattCharacteristicProperty::Read => {
            read_op.get_or_insert_with(|| CharacteristicRead {
              fun: self.new_read_handler::<CharacteristicReadRequestType>(handle_type),
              ..Default::default()
            });
          }
          GattCharacteristicProperty::Write |
              GattCharacteristicProperty::WriteSigned |
              GattCharacteristicProperty::WriteNoResponse => {
            write_op.get_or_insert_with(|| CharacteristicWrite {
              method: CharacteristicWriteMethod::Fun(
                  self.new_write_handler::<CharacteristicWriteRequestType>(handle_type)
              ),
              ..Default::default()
            });
          }
          _ => {}
        }
      }

      for prop in spec.properties {
        match prop {
          GattCharacteristicProperty::Indicate => {
            notify_op.as_mut().map(|o| o.indicate = true);
          }
          GattCharacteristicProperty::Notify => {
            notify_op.as_mut().map(|o| o.notify = true);
          }
          GattCharacteristicProperty::WriteSigned => {
            write_op.as_mut().map(|o| o.authenticated_signed_writes = true);
          }
          GattCharacteristicProperty::WriteNoResponse => {
            write_op.as_mut().map(|o| o.write_without_response = true);
          }
          _ => {}
        }
      }

      for perm in spec.permissions {
        match perm {
          GattCharacteristicPermission::Read => {
            read_op.as_mut().map(|o| o.read = true);
          }
          GattCharacteristicPermission::ReadEncrypted => {
            read_op.as_mut().map(|o| o.encrypt_read = true);
          }
          GattCharacteristicPermission::Write => {
            write_op.as_mut().map(|o| o.write = true);
          }
          GattCharacteristicPermission::WriteEncrypted => {
            write_op.as_mut().map(|o| o.encrypt_write = true);
          }
          GattCharacteristicPermission::WriteEncryptedMitm => Err(generic_err("write encrypted mitm not supported"))?,
          GattCharacteristicPermission::WriteSigned => Err(generic_err("write signed not supported"))?,
          GattCharacteristicPermission::WriteSignedMitm => Err(generic_err("write signed mitm not supported"))?,
        }
      }

      Ok(Characteristic {
        uuid: convert_uuid(spec.uuid),
        broadcast: spec.properties.contains(GattCharacteristicProperty::Broadcast),
        authorize: false, // TODO,
        writable_auxiliaries: false, // TODO,
        read: read_op,
        write: write_op,
        notify: notify_op,
        ..Default::default()
      })
    }

    fn new_descriptor(&mut self, spec: &GattDescriptor) -> Result<Descriptor, bluer::Error> {
      let handle_type = self.handle_allocator.next(spec.uuid);

      let mut read_op = None;
      let mut write_op = None;

      for perm in spec.permissions {
        match perm {
          GattDescriptorPermission::Read |
              GattDescriptorPermission::ReadEncrypted => {
            read_op.get_or_insert_with(|| DescriptorRead {
              fun: self.new_read_handler::<DescriptorReadRequestType>(handle_type),
              ..Default::default()
            });
          }
          GattDescriptorPermission::Write |
              GattDescriptorPermission::WriteEncrypted |
              GattDescriptorPermission::WriteEncryptedMitm |
              GattDescriptorPermission::WriteSigned |
              GattDescriptorPermission::WriteSignedMitm => {
            write_op.get_or_insert_with(|| DescriptorWrite {
              fun: self.new_write_handler::<DescriptorWriteRequestType>(handle_type),
              ..Default::default()
            });
          }
        }
      }

      for perm in spec.permissions {
        match perm {
          GattDescriptorPermission::Read => {
            read_op.as_mut().map(|o| o.read = true);
          },
          GattDescriptorPermission::ReadEncrypted => {
            read_op.as_mut().map(|o| o.encrypt_read = true);
          },
          GattDescriptorPermission::Write => {
            write_op.as_mut().map(|o| o.write = true);
          },
          GattDescriptorPermission::WriteEncrypted => {
            write_op.as_mut().map(|o| o.encrypt_write = true);
          },
          GattDescriptorPermission::WriteEncryptedMitm => Err(generic_err("write encrypted mitm not supported"))?,
          GattDescriptorPermission::WriteSigned => Err(generic_err("write signed not supported"))?,
          GattDescriptorPermission::WriteSignedMitm => Err(generic_err("write signed mitm not supported"))?,
        }
      }

      Ok(Descriptor {
        uuid: convert_uuid(spec.uuid),
        read: read_op,
        write: write_op,
        ..Default::default()
      })
    }

    fn new_read_handler<T>(&self, handle: AttributeHandle) -> ReadRequest<T::Inner>
    where
        T: HasConnectionFields,
        T::Inner: Send + 'static,
    {
      let local_tx = self.tx.clone();
      Box::new(move |req| {
        let tx = local_tx.clone();
        async move {
          let (responder_tx, responder_rx) = oneshot::channel();
          tx.send(Event::OnHandleRead {
            conn: BluerConnection::new(T::address(&req)),
            handle,
            responder: BluerResponder { tx: Some(responder_tx) },
          })
              .map_err(|_| ReqError::Failed)?;
          let response = responder_rx.await
              .map_err(|_| ReqError::Failed)?
              .map_err(|e| att_to_req_error(e))?;
          if response.0 > 0 {
            return Err(ReqError::InvalidOffset);
          }
          Ok(response.1)
        }.boxed()
      })
    }

    fn new_write_handler<T>(&self, handle: AttributeHandle) -> WriteRequest<<T as HasWriteFields>::Inner>
    where
        T: HasConnectionFields + HasWriteFields<Inner=<T as HasConnectionFields>::Inner>,
        <T as HasWriteFields>::Inner: Send + 'static,
    {
      let local_tx = self.tx.clone();
      Box::new(move |value, req| {
        let tx = local_tx.clone();
        async move {
          // TODO: How is the responder supposed to work for characteristic writes!?!?
          tx.send(Event::OnHandleWrite {
            conn: BluerConnection::new(T::address(&req)),
            handle,
            responder: None,
            action: WriteAction::Normal,
            offset: T::offset(&req),
            value,
          })
              .map_err(|_| ReqError::Failed)?;
          Ok(())
        }.boxed()
      })
    }

    fn new_notify_handler(&self, handle: AttributeHandle) -> CharacteristicNotifyFun {
      let local_tx = self.tx.clone();
      Box::new(move |mut notifier| {
        let tx = local_tx.clone();
        async move {
          let (notify_tx, mut notify_rx) = mpsc::unbounded_channel();
          let writer = BluerWriter { tx: notify_tx };
          if let Err(_) = tx.send(Event::OnSubscribe {
            // TODO: File a feature request upstream to get the address of the peer!
            conn: BluerConnection::new(&Address::any()),
            handle,
            writer,
          }) {
            warn!("Server shutdown race!");
            return;
          }
          let unsubscribe_tx = tx.clone();
          tokio::spawn(async move {
            while let Some(value) = notify_rx.recv().await {
              if let Err(e) = notifier.notify(value).await {
                if notifier.is_stopped() {
                  if let Err(_) = unsubscribe_tx.send(Event::OnUnsubscribe {
                    conn: BluerConnection::new(&Address::any()),
                    handle,
                  }) {
                    warn!("Server shutdown race!");
                  }
                } else {
                  warn!("Error sending notify: {e}");
                }
              }
            }
          });
        }.boxed()
      })
    }
  }

  type ReadRequest<R> = Box<
    dyn (Fn(R) -> Pin<Box<dyn Future<Output = ReqResult<Vec<u8>>> + Send>>) + Send + Sync,
  >;

  type WriteRequest<R> = Box<
    dyn Fn(Vec<u8>, R) -> Pin<Box<dyn Future<Output = ReqResult<()>> + Send>> + Send + Sync,
  >;

  trait HasConnectionFields {
    type Inner;

    fn address(inner: &Self::Inner) -> &Address;
  }

  trait HasWriteFields {
    type Inner;

    fn offset(inner: &Self::Inner) -> u16;
  }

  struct DescriptorReadRequestType;
  impl HasConnectionFields for DescriptorReadRequestType {
    type Inner = DescriptorReadRequest;

    fn address(inner: &Self::Inner) -> &Address {
      &inner.device_address
    }
  }

  struct DescriptorWriteRequestType;
  impl HasConnectionFields for DescriptorWriteRequestType {
    type Inner = DescriptorWriteRequest;

    fn address(inner: &Self::Inner) -> &Address {
      &inner.device_address
    }
  }

  impl HasWriteFields for DescriptorWriteRequestType {
    type Inner = DescriptorWriteRequest;

    fn offset(inner: &Self::Inner) -> u16 {
      inner.offset
    }
  }

  struct CharacteristicReadRequestType;
  impl HasConnectionFields for CharacteristicReadRequestType {
    type Inner = CharacteristicReadRequest;

    fn address(inner: &Self::Inner) -> &Address {
      &inner.device_address
    }
  }

  struct CharacteristicWriteRequestType;
  impl HasConnectionFields for CharacteristicWriteRequestType {
    type Inner = CharacteristicWriteRequest;

    fn address(inner: &Self::Inner) -> &Address {
      &inner.device_address
    }
  }

  impl HasWriteFields for CharacteristicWriteRequestType {
    type Inner = CharacteristicWriteRequest;

    fn offset(inner: &Self::Inner) -> u16 {
      inner.offset
    }
  }

  fn att_to_req_error(e: AttError) -> ReqError {
    match e {
      AttError::InvalidHandle => ReqError::Failed,
      AttError::ReadNotPermitted => ReqError::NotPermitted,
      AttError::WriteNotPermitted => ReqError::NotPermitted,
      AttError::InvalidPdu => ReqError::NotSupported,
      AttError::InsufficientAuthentication => ReqError::NotAuthorized,
      AttError::RequestNotSupported => ReqError::NotSupported,
      AttError::InvalidOffset => ReqError::InvalidOffset,
      AttError::InsufficientAuthorization => ReqError::NotAuthorized,
      AttError::PrepareQueueFull => ReqError::Failed,
      AttError::AttributeNotFound => ReqError::Failed,
      AttError::AttributeTooLong => ReqError::Failed,
      AttError::InsufficientKeySize => ReqError::Failed,
      AttError::InvalidAttributeValueLength => ReqError::InvalidValueLength,
      AttError::Unlikely => ReqError::Failed,
      AttError::InsufficientEncryption => ReqError::Failed,
      AttError::UnsupportedGroupType => ReqError::Failed,
      AttError::InsufficientResources => ReqError::Failed,
    }
  }

  struct HandleAllocator {
    next: NonZeroU16,
    mapping: Vec<(UUID, AttributeHandle)>,
  }

  impl HandleAllocator {
    pub fn new() -> Self {
      Self {
        next: NonZeroU16::new(1).unwrap(),
        mapping: Vec::new(),
      }
    }

    pub fn next(&mut self, uuid: UUID) -> AttributeHandle {
      let next = AttributeHandle(self.next);
      self.mapping.push((uuid, next));
      self.next = self.next.checked_add(1).unwrap();
      next
    }
  }

  fn generic_err(debug_message: &str) -> bluer::Error {
    bluer::Error::from(std::io::Error::new(std::io::ErrorKind::Other, debug_message))
  }

  async fn run_event_loop(
      adapter: Adapter,
      mut rx: mpsc::UnboundedReceiver<Event>,
      self_tx: mpsc::UnboundedSender<Event>,
      mut callback: impl GattServerCallback<BluerPeripheral>,
  ) {
    let mut handles = KeepAliveHandles::default();

    while let Some(event) = rx.recv().await {
      match event {
        Event::OnStartResult(r) => {
          let event = match r {
            Ok((handle, advertiser, handle_mapping)) => {
              handles.app = Some(handle);
              GattServerEvent::ServerStarted {
                advertiser,
                handle_mapping,
              }
            }
            Err(error) => {
              GattServerEvent::ServerShutdown {
                error,
              }
            }
          };
          callback.on_event(event);
        },
        Event::OnHandleDrop => {
          debug!("Advertiser dropped, shutting down!");
          return;
        }
        Event::RequestAdvStart(adv) => {
          let self_tx = self_tx.clone();
          let adapter_clone = adapter.clone();
          tokio::spawn(async move {
            let r = adapter_clone.advertise(adv).await;
            let _ = self_tx.send(Event::OnAdvStartResult(r));
          });
        },
        Event::OnAdvStartResult(r) => {
          let event = match r {
            Ok(handle) => {
              handles.advertisement = Some(handle);
              GattServerEvent::AdvertisingStarted {
                remaining_connections: None,
              }
            },
            Err(error) => {
              GattServerEvent::AdvertisingStartFail {
                reason: AdvStartFailedReason::SystemError(error),
              }
            }
          };
          callback.on_event(event);
        }
        Event::RequestAdvStop => {
          drop(handles.advertisement.take());
          callback.on_event(GattServerEvent::AdvertisingStopped {
            reason: AdvStopReason::Requested,
          });
        },
        Event::OnHandleRead { conn, handle, mut responder } => {
          callback.on_event(GattServerEvent::ReadRequest {
            connection: &conn,
            handle,
            responder: &mut responder,
          });
        }
        Event::OnHandleWrite { conn, handle, mut responder, action, offset, value } => {
          callback.on_event(GattServerEvent::WriteRequest {
            connection: &conn,
            handle,
            responder: responder.as_mut(),
            action,
            offset,
            value: &value,
          });
        }
        Event::OnSubscribe { conn, handle, mut writer } => {
          callback.on_event(GattServerEvent::Subscribe {
            connection: &conn,
            handle,
            writer: &mut writer,
          });
        }
        Event::OnUnsubscribe { conn, handle } => {
          callback.on_event(GattServerEvent::Unsubscribe {
            connection: &conn,
            handle,
          });
        }
      };
    }
  }

  fn convert_uuid(trait_uuid: UUID) -> Uuid {
    Uuid::from_u128(trait_uuid.as_u128())
  }

  #[derive(Debug, Default)]
  struct DeviceIdent {
    name: Option<String>,
    appearance: Option<u16>,
  }

  pub struct BluerHandle {
    tx: mpsc::UnboundedSender<Event>,
  }

  impl Drop for BluerHandle {
    fn drop(&mut self) {
      let _ = self.tx.send(Event::OnHandleDrop);
    }
  }

  #[derive(Clone)]
  pub struct BluerAdvertiser {
    tx: mpsc::UnboundedSender<Event>,
    service_uuids: Arc<BTreeSet<Uuid>>,
    ident: Arc<DeviceIdent>,
  }

  impl Debug for BluerAdvertiser {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
      f.debug_struct("BluerAdvertiser")
          .field("service_uuids", &self.service_uuids)
          .field("ident", &self.ident)
          .finish_non_exhaustive()
    }
  }

  impl GapAdvertiser for BluerAdvertiser {
    fn request_start(&self, advertisement: Advertisement) {
      let adv = bluer::adv::Advertisement {
        advertisement_type: Type::Peripheral,
        service_uuids: self.service_uuids.deref().clone(),
        manufacturer_data: advertisement.manufacturer_data,
        discoverable: Some(advertisement.is_discoverable),
        local_name: self.ident.name.clone(),
        ..Default::default()
      };

      let _ = self.tx.send(Event::RequestAdvStart(adv));
    }

    fn request_stop(&self) {
      let _ = self.tx.send(Event::RequestAdvStop);
    }
  }

  #[derive(Debug, Clone)]
  pub struct BluerConnection {
    address: BluetoothAddress,
  }

  impl BluerConnection {
    pub fn new(address: &Address) -> Self {
      Self {
        address: BluetoothAddress(address.0),
      }
    }
  }

  impl GattConnection for BluerConnection {
    type SystemError = bluer::Error;
    type Responder = BluerResponder;
    type Writer = BluerWriter;

    fn peer_address(&self) -> &BluetoothAddress {
      &self.address
    }
  }

  #[derive(Debug)]
  pub struct BluerResponder {
    tx: Option<oneshot::Sender<Result<(u8, Vec<u8>), AttError>>>,
  }

  impl GattResponder for BluerResponder {
    type SystemError = bluer::Error;

    fn respond(&mut self, response: Result<Response<'_>, AttError>) -> Result<(), Self::SystemError> {
      let mapped = response.map(|r| (r.offset, r.value.to_owned()));
      self.tx.take()
          .ok_or_else(|| generic_err("Already sent response!"))?
          .send(mapped)
          .map_err(|_| generic_err("Server shutdown"))
    }
  }

  impl Drop for BluerResponder {
    fn drop(&mut self) {
      if let Some(tx) = self.tx.take() {
        warn!("Failed to call respond on GattResponder!");
        let _ = tx.send(Err(AttError::AttributeNotFound));
      }
    }
  }

  #[derive(Clone)]
  pub struct BluerWriter {
    tx: mpsc::UnboundedSender<Vec<u8>>,
  }

  impl Debug for BluerWriter {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
      f.debug_struct("BluerWriter")
          .finish_non_exhaustive()
    }
  }

  impl GattWriter for BluerWriter {
    type SystemError = bluer::Error;

    fn write(&mut self, value: &[u8]) -> Result<(), Self::SystemError> {
      self.tx.send(value.to_owned())
          .map_err(|_| generic_err("unsubscribed"))
    }
  }

  pub enum Event {
    OnStartResult(Result<(ApplicationHandle, BluerAdvertiser, Vec<(UUID, AttributeHandle)>), bluer::Error>),
    OnHandleDrop,
    RequestAdvStart(bluer::adv::Advertisement),
    RequestAdvStop,
    OnAdvStartResult(Result<AdvertisementHandle, bluer::Error>),
    OnHandleRead {
      conn: BluerConnection,
      handle: AttributeHandle,
      responder: BluerResponder,
    },
    OnHandleWrite {
      conn: BluerConnection,
      handle: AttributeHandle,
      responder: Option<BluerResponder>,
      action: WriteAction,
      offset: u16,
      value: Vec<u8>,
    },
    OnSubscribe {
      conn: BluerConnection,
      handle: AttributeHandle,
      writer: BluerWriter,
    },
    OnUnsubscribe {
      conn: BluerConnection,
      handle: AttributeHandle,
    },
  }

  /// Collections of bluer handles that keep the server and advertiser alive and working.
  #[derive(Default)]
  struct KeepAliveHandles {
    app: Option<ApplicationHandle>,
    advertisement: Option<AdvertisementHandle>,
  }
}