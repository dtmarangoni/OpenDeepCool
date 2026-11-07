//! The client (Phase 10 plan, Task 4; `docs/design/settings-app.md`, Talking to the service): the
//! app's side of `odc-ipc`, on the D-Bus system bus, or on the bus `--service-address` names for
//! the tests. zbus's default runtime runs a connection's I/O on two threads of its own (zbus
//! README, Compatibility with async runtimes), so the app awaits its replies and signals on
//! GLib's main loop, where the window lives, with no thread or channel of the app's own
//! (decisions log D45).
//!
//! It watches the state (`Watch()`), then reads all of it (`GetState()`), merges each
//! `StateChanged` in the order they arrive, and does both again whenever the service's name gets
//! a new owner, the window becomes active, or the app's logind session turns active, one at a
//! time and never polling. Frames are watched while the window is mapped.

use std::cell::{Cell, OnceCell, RefCell};
use std::collections::HashMap;
use std::fs::{File, OpenOptions};
use std::future::poll_fn;
use std::io::{self, Write};
use std::path::Path;
use std::pin::{Pin, pin};
use std::rc::Rc;
use std::task::Poll;

use futures_core::Stream;
use gtk::glib;
use odc_ipc::{
    BUS_NAME, Error, INTERFACE, Input, OBJECT_PATH, OpenDeepCoolProxy, State, StateError,
    frame_from_pixels,
};
use zbus::fdo::DBusProxy;
use zbus::message::Type;
use zbus::names::BusName;
use zbus::wire::{OwnedObjectPath, OwnedVariant};
use zbus::{Connection, MatchRule, Message, MessageStream};

use crate::model::reason_text;
use crate::ui::window::Window;
use crate::ui::{Reason, Refused, Request};

const BUS_DRIVER: &str = "org.freedesktop.DBus";
const LOGIND: &str = "org.freedesktop.login1";

/// The service's proxy, which holds the connection.
struct Bus {
    service: OpenDeepCoolProxy<'static>,
}

/// The client; the window's requests reach it through [`Client::sink`].
#[derive(Default)]
pub struct Client {
    window: OnceCell<Window>,
    bus: OnceCell<Bus>,
    /// The service's unique name, while it's on the bus: only its signals count.
    owner: RefCell<Option<String>>,
    /// The state, merged from `GetState()` and every `StateChanged` since.
    state: RefCell<Option<State>>,
    /// The window is mapped: frames are wanted.
    frames: Cell<bool>,
    /// A connect runs; another was asked meanwhile.
    connecting: Cell<bool>,
    again: Cell<bool>,
    /// `--frame-log`: each frame the service sends is added to it (decisions log D84).
    frame_log: RefCell<Option<File>>,
}

/// How a state dictionary is read: as `odc-ipc` reads it, except that a `CoolerState` the app
/// doesn't know is left out, and the window shows that as `not-found` (decisions log D33).
fn read_state(mut dict: HashMap<String, OwnedVariant>) -> Result<(State, bool), StateError> {
    match State::from_dict(&dict) {
        Err(StateError::BadValue("CoolerState", _)) => {
            dict.remove("CoolerState");
            State::from_dict(&dict).map(|state| (state, true))
        }
        other => other.map(|state| (state, false)),
    }
}

/// Why the service refused, as the window says it.
fn reason(error: &Error) -> Reason {
    match error {
        Error::NotAuthorized(_) => Reason::NotAuthorized,
        Error::ModeUnavailable(text) => Reason::Message(reason_text(text)),
        Error::NotYetAvailable(text)
        | Error::InvalidArgument(text)
        | Error::TooManyWatchers(text)
        | Error::Busy(text)
        | Error::AudioNotWanted(text)
        | Error::Failed(text) => Reason::Message(text.clone()),
        Error::ZBus(error) => Reason::Message(error.to_string()),
    }
}

/// An input as `SetCoolerFanInput` and `SetPumpInput` take it: all empty for none.
fn input_args(input: Option<&Input>) -> (&str, &str, u32) {
    match input {
        Some(input) => (&input.chip, &input.device, input.input),
        None => ("", "", 0),
    }
}

/// Whether `error` says nobody owns the service's name.
fn no_service(error: &Error) -> bool {
    match error {
        Error::ZBus(zbus::Error::MethodError(name, _, _)) => {
            matches!(
                name.as_str(),
                "org.freedesktop.DBus.Error.ServiceUnknown"
                    | "org.freedesktop.DBus.Error.NameHasNoOwner"
            )
        }
        _ => false,
    }
}

async fn next(stream: &mut MessageStream) -> Option<Message> {
    loop {
        match poll_fn(|cx| Pin::new(&mut *stream).poll_next(cx)).await {
            Some(Ok(message)) => return Some(message),
            // A message that couldn't be read is skipped.
            Some(Err(_)) => continue,
            None => return None,
        }
    }
}

fn signal(
    sender: &'static str,
    path: &'static str,
    interface: &'static str,
    member: &'static str,
) -> zbus::match_rule::Builder<'static> {
    MatchRule::builder()
        .msg_type(Type::Signal)
        .sender(sender)
        .path(path)
        .interface(interface)
        .member(member)
}

/// What a stream brought.
enum Event {
    State(Message),
    Frame(Message),
    Owner(Message),
    Session(Message),
    Closed,
}

impl Client {
    pub fn new() -> Rc<Client> {
        Rc::new(Client::default())
    }

    /// Where the window sends its requests.
    pub fn sink(self: &Rc<Self>) -> impl Fn(Request) + 'static {
        let weak = Rc::downgrade(self);
        move |request| {
            if let Some(client) = weak.upgrade() {
                client.request(request);
            }
        }
    }

    /// Adds each frame the service sends to `path`, one line each: the time it came, in
    /// nanoseconds of `CLOCK_MONOTONIC` as the dev recording's, and its 196 dots as `0` and `1`,
    /// row by row (`--frame-log`, for the end-to-end run's comparison; decisions log D84).
    ///
    /// # Errors
    ///
    /// When the file can't be opened.
    pub fn log_frames(&self, path: &Path) -> io::Result<()> {
        let file = OpenOptions::new().create(true).append(true).open(path)?;
        *self.frame_log.borrow_mut() = Some(file);
        Ok(())
    }

    /// Connects, on the system bus or at `address`, and serves `window` from then on.
    pub fn start(self: &Rc<Self>, window: Window, address: Option<String>) {
        if self.window.set(window).is_err() {
            return;
        }
        let client = Rc::clone(self);
        glib::spawn_future_local(async move {
            if let Err(error) = client.run(address).await {
                eprintln!("opendeepcool: the bus: {error}");
                client.window().down();
            }
        });
    }

    fn window(&self) -> &Window {
        self.window.get().expect("a window")
    }

    fn request(self: &Rc<Self>, request: Request) {
        match request {
            Request::WatchFrames(on) => {
                self.frames.set(on);
                let client = Rc::clone(self);
                glib::spawn_future_local(async move { client.watch_frames().await });
            }
            Request::Reconnect => {
                let client = Rc::clone(self);
                glib::spawn_future_local(async move { client.connect().await });
            }
            Request::SetMode(mode) => {
                self.change(move |service| async move { service.set_mode(mode).await })
            }
            Request::SetTemperatureUnit(unit) => self.change(move |service| async move {
                service.set_temperature_unit(unit.as_str()).await
            }),
            Request::SetRotation(rotation) => {
                self.change(move |service| async move { service.set_rotation(rotation).await })
            }
            Request::ChooseGpu(pci) => {
                self.change(move |service| async move { service.choose_gpu(&pci).await })
            }
            Request::SetCoolerFanInput(input) => self.change(move |service| async move {
                let (chip, device, number) = input_args(input.as_ref());
                service.set_cooler_fan_input(chip, device, number).await
            }),
            Request::SetPumpInput(input) => self.change(move |service| async move {
                let (chip, device, number) = input_args(input.as_ref());
                service.set_pump_input(chip, device, number).await
            }),
            Request::SetMusicPeaks(on) => {
                self.change(move |service| async move { service.set_music_peaks(on).await })
            }
            Request::SetMusicBarFall(stop) => self.change(move |service| async move {
                service.set_music_bar_fall(stop.as_str()).await
            }),
            Request::SetMusicPeakFall(stop) => self.change(move |service| async move {
                service.set_music_peak_fall(stop.as_str()).await
            }),
            Request::Refresh => {
                let client = Rc::clone(self);
                glib::spawn_future_local(async move {
                    let Some(bus) = client.bus.get() else {
                        return;
                    };
                    match bus.service.refresh().await {
                        Ok(()) => client.window().refreshed(),
                        Err(error) => client.window().refusal(Refused::Refresh, &reason(&error)),
                    }
                });
            }
        }
    }

    /// A change: the service's call, and the refusal shown if it refuses. The control shows the
    /// new value once `StateChanged` confirms it (Refused changes).
    fn change<F, Fut>(self: &Rc<Self>, call: F)
    where
        F: FnOnce(OpenDeepCoolProxy<'static>) -> Fut + 'static,
        Fut: std::future::Future<Output = Result<(), Error>>,
    {
        let client = Rc::clone(self);
        glib::spawn_future_local(async move {
            let Some(bus) = client.bus.get() else {
                return;
            };
            if let Err(error) = call(bus.service.clone()).await {
                client.window().refusal(Refused::Change, &reason(&error));
            }
        });
    }

    async fn run(self: &Rc<Self>, address: Option<String>) -> zbus::Result<()> {
        let conn = match address {
            Some(address) => {
                zbus::connection::Builder::address(address.as_str())
                    .build()
                    .await?
            }
            None => Connection::system().await?,
        };
        // Subscribed first, so no change is missed.
        let mut states = MessageStream::for_match_rule(
            signal(BUS_NAME, OBJECT_PATH, INTERFACE, "StateChanged").build()?,
            &conn,
            None,
        )
        .await?;
        let mut frames = MessageStream::for_match_rule(
            signal(BUS_NAME, OBJECT_PATH, INTERFACE, "Frame").build()?,
            &conn,
            None,
        )
        .await?;
        let mut owners = MessageStream::for_match_rule(
            signal(
                BUS_DRIVER,
                "/org/freedesktop/DBus",
                BUS_DRIVER,
                "NameOwnerChanged",
            )
            .arg(0, BUS_NAME)
            .build()?,
            &conn,
            None,
        )
        .await?;
        let mut session = self.session_stream(&conn).await;
        let dbus = DBusProxy::new(&conn).await?;
        let owner = match BusName::try_from(BUS_NAME) {
            Ok(name) => dbus.get_name_owner(name).await.ok().map(|o| o.to_string()),
            Err(_) => None,
        };
        let service = OpenDeepCoolProxy::new(&conn).await?;
        let _ = self.bus.set(Bus { service });
        *self.owner.borrow_mut() = owner;
        self.connect().await;
        loop {
            let event = {
                let (mut state, mut frame, mut owner) = (
                    pin!(next(&mut states)),
                    pin!(next(&mut frames)),
                    pin!(next(&mut owners)),
                );
                let mut seat = session.as_mut().map(|s| Box::pin(next(s)));
                poll_fn(|cx| {
                    for (future, wrap) in [
                        (state.as_mut(), Event::State as fn(Message) -> Event),
                        (frame.as_mut(), Event::Frame),
                        (owner.as_mut(), Event::Owner),
                    ] {
                        if let Poll::Ready(message) = future.poll(cx) {
                            return Poll::Ready(message.map_or(Event::Closed, wrap));
                        }
                    }
                    if let Some(seat) = seat.as_mut()
                        && let Poll::Ready(message) = seat.as_mut().poll(cx)
                    {
                        return Poll::Ready(message.map_or(Event::Closed, Event::Session));
                    }
                    Poll::Pending
                })
                .await
            };
            match event {
                Event::State(message) => self.state_changed(&message),
                Event::Frame(message) => self.frame(&message),
                Event::Owner(message) => self.owner_changed(&message).await,
                Event::Session(message) => self.session_changed(&message).await,
                Event::Closed => return Err(zbus::Error::Failure("the bus closed".into())),
            }
        }
    }

    /// logind's PropertiesChanged on this app's session, on the system bus only.
    async fn session_stream(&self, conn: &Connection) -> Option<MessageStream> {
        let reply = conn
            .call_method(
                Some(LOGIND),
                "/org/freedesktop/login1",
                Some("org.freedesktop.login1.Manager"),
                "GetSession",
                &("auto",),
            )
            .await
            .ok()?;
        let path: OwnedObjectPath = reply.body().deserialize().ok()?;
        let rule = MatchRule::builder()
            .msg_type(Type::Signal)
            .sender(LOGIND)
            .path(path.as_str())
            .interface("org.freedesktop.DBus.Properties")
            .member("PropertiesChanged")
            .build()
            .ok()?
            .to_owned();
        MessageStream::for_match_rule(rule, conn, None).await.ok()
    }

    fn sent_by_owner(&self, message: &Message) -> bool {
        let header = message.header();
        header.sender().map(|s| s.as_str()) == self.owner.borrow().as_deref()
    }

    /// `Watch()`, then `GetState()`, then the frames when wanted; one at a time.
    async fn connect(self: &Rc<Self>) {
        if self.connecting.replace(true) {
            self.again.set(true);
            return;
        }
        loop {
            self.again.set(false);
            self.connect_once().await;
            if !self.again.get() {
                break;
            }
        }
        self.connecting.set(false);
    }

    async fn connect_once(self: &Rc<Self>) {
        let Some(bus) = self.bus.get() else {
            return;
        };
        if self.owner.borrow().is_none() {
            *self.state.borrow_mut() = None;
            self.window().down();
            return;
        }
        let result = match bus.service.watch().await {
            Ok(()) => bus.service.get_state().await,
            Err(error) => Err(error),
        };
        match result {
            Ok(dict) => match read_state(dict) {
                Ok((state, _)) => {
                    *self.state.borrow_mut() = Some(state.clone());
                    self.window().state(state);
                    if self.frames.get() {
                        self.watch_frames().await;
                    }
                }
                Err(error) => {
                    // A state this app can't read: shown as no service (decisions log D46).
                    eprintln!("opendeepcool: the service's state can't be read: {error}");
                    *self.state.borrow_mut() = None;
                    self.window().down();
                }
            },
            Err(Error::NotAuthorized(_)) => {
                *self.state.borrow_mut() = None;
                self.window().refused();
            }
            Err(error) => {
                if !no_service(&error) {
                    eprintln!("opendeepcool: the service: {error}");
                }
                *self.state.borrow_mut() = None;
                self.window().down();
            }
        }
    }

    async fn watch_frames(&self) {
        let Some(bus) = self.bus.get() else {
            return;
        };
        if self.state.borrow().is_none() {
            return;
        }
        let result = if self.frames.get() {
            bus.service.watch_frames().await
        } else {
            bus.service.unwatch_frames().await
        };
        if let Err(error) = result {
            eprintln!("opendeepcool: the preview's frames: {error}");
        }
    }

    /// A `StateChanged` from the service, merged in the order they arrive.
    fn state_changed(&self, message: &Message) {
        if !self.sent_by_owner(message) {
            return;
        }
        let Ok((changes,)) = message
            .body()
            .deserialize::<(HashMap<String, OwnedVariant>,)>()
        else {
            return;
        };
        let (changes, unknown_cooler) = match read_state(changes) {
            Ok(read) => read,
            Err(error) => {
                eprintln!("opendeepcool: a state change can't be read: {error}");
                return;
            }
        };
        let mut state = self.state.borrow_mut();
        let Some(state) = state.as_mut() else {
            return;
        };
        state.merge(changes);
        if unknown_cooler {
            state.cooler_state = None;
        }
        let state = state.clone();
        self.window().state(state);
    }

    fn frame(&self, message: &Message) {
        if !self.sent_by_owner(message) {
            return;
        }
        if let Ok((pixels,)) = message.body().deserialize::<(Vec<u8>,)>()
            && let Some(frame) = frame_from_pixels(&pixels)
        {
            if let Some(log) = self.frame_log.borrow_mut().as_mut() {
                let dots: String = pixels.iter().map(|&p| char::from(b'0' + p)).collect();
                if let Err(e) = writeln!(log, "{} {dots}", monotonic_ns()) {
                    eprintln!("opendeepcool: the frame log: {e}");
                }
            }
            self.window().frame(frame);
        }
    }

    /// The service's name changed owner: connect to the new one, or show it's gone.
    async fn owner_changed(self: &Rc<Self>, message: &Message) {
        let header = message.header();
        if header.sender().map(|s| s.as_str()) != Some(BUS_DRIVER) {
            return;
        }
        let Ok((name, _, new_owner)) = message.body().deserialize::<(String, String, String)>()
        else {
            return;
        };
        if name != BUS_NAME {
            return;
        }
        *self.owner.borrow_mut() = (!new_owner.is_empty()).then_some(new_owner);
        self.connect().await;
    }

    /// The session's `Active` turned true: connect again.
    async fn session_changed(self: &Rc<Self>, message: &Message) {
        let Ok((interface, changed, _)) =
            message
                .body()
                .deserialize::<(String, HashMap<String, OwnedVariant>, Vec<String>)>()
        else {
            return;
        };
        if interface != "org.freedesktop.login1.Session" {
            return;
        }
        let active = changed
            .get("Active")
            .and_then(|v| v.try_clone().ok())
            .and_then(|v| bool::try_from(v).ok());
        if active == Some(true) {
            self.connect().await;
        }
    }
}

/// Now, in nanoseconds of `CLOCK_MONOTONIC`, the dev recording's clock (`odc_device`).
fn monotonic_ns() -> u64 {
    let now = rustix::time::clock_gettime(rustix::time::ClockId::Monotonic);
    let secs = u64::try_from(now.tv_sec).unwrap_or(0);
    let nanos = u64::try_from(now.tv_nsec).unwrap_or(0);
    secs * 1_000_000_000 + nanos
}

#[cfg(test)]
mod tests {
    use super::*;
    use zbus::wire::Variant;

    fn variant<'a>(value: impl Into<Variant<'a>>) -> OwnedVariant {
        OwnedVariant::try_from(value.into()).expect("no file descriptor")
    }

    // A `CoolerState` a later service might send is left out and flagged, so the window shows
    // `not-found`; the rest of the state still reads (D33). Any other unknown value refuses it.
    #[test]
    fn an_unknown_cooler_state_is_left_out_and_the_rest_reads() {
        let dict = HashMap::from([
            ("Mode".to_owned(), variant(4_u32)),
            ("CoolerState".to_owned(), variant("on fire")),
        ]);
        let (state, unknown) = read_state(dict).expect("read");
        assert!(unknown);
        assert_eq!((state.mode, state.cooler_state), (Some(4), None));
        let dict = HashMap::from([("CoolerState".to_owned(), variant("no-access"))]);
        let (state, unknown) = read_state(dict).expect("read");
        assert!(!unknown);
        assert_eq!(state.cooler_state, Some(odc_ipc::CoolerState::NoAccess));
        let dict = HashMap::from([("TemperatureUnit".to_owned(), variant("kelvin"))]);
        assert!(read_state(dict).is_err());
    }

    #[test]
    fn refusals_name_their_reason_as_the_window_says_it() {
        assert_eq!(
            reason(&Error::NotAuthorized("polkit said no".to_owned())),
            Reason::NotAuthorized
        );
        assert_eq!(
            reason(&Error::ModeUnavailable(
                "Gpu usage: no device found".to_owned()
            )),
            Reason::Message("GPU usage: no device found".to_owned())
        );
        assert_eq!(
            reason(&Error::Failed(
                "the settings file can't be written".to_owned()
            )),
            Reason::Message("the settings file can't be written".to_owned())
        );
        assert!(!no_service(&Error::Failed("x".to_owned())));
    }
}
