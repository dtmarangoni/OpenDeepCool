//! The Start at boot switch's model (`docs/design/settings-app.md`, Start at boot; Phase 11
//! plan, Task 3): the four facts read from systemd, what the switch, its note, the off screen
//! and the terminal commands show, and what a flip runs. No GTK, no D-Bus and no clock: the
//! client reads systemd and runs the flip ([`crate::startup`]), and the window draws a
//! [`Switch`] and an [`OffScreen`].

pub use odc_boot::Request as Direction;
use odc_boot::{HELPER, PROGRAM, SERVICE};

/// A unit file's state, as `GetUnitFileState` answers it.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum UnitFile {
    Enabled,
    Disabled,
    /// Any other state (`masked`, `enabled-runtime`, `linked`, `bad`, …), as systemd names it
    /// (systemctl(1), is-enabled).
    Other(String),
}

impl UnitFile {
    pub fn read(state: &str) -> UnitFile {
        match state {
            "enabled" => UnitFile::Enabled,
            "disabled" => UnitFile::Disabled,
            other => UnitFile::Other(other.to_owned()),
        }
    }
}

/// Whether the display service runs.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Run {
    Running,
    Stopped,
    /// Failed, or waiting to restart after a failure.
    Failed,
    Starting,
}

/// How the display service runs, from its `ActiveState` and `SubState` (org.freedesktop.systemd1(5),
/// Table 1; the substates are systemd's `src/basic/unit-def.c`), and whether its bus name has an
/// owner: it takes the name just before it reports ready, so a read right after the name's change
/// can find it `activating` (Where its state comes from). `None` for a state the manual doesn't list.
pub fn service_run(active: &str, sub: &str, owned: bool) -> Option<Run> {
    match active {
        "active" | "reloading" | "refreshing" => Some(Run::Running),
        "inactive" | "deactivating" | "maintenance" => Some(Run::Stopped),
        "failed" => Some(Run::Failed),
        // A service waiting to restart after a failure is `activating` (systemd's
        // `src/core/service.c`, its state table): with no start limit, a crashing service cycles
        // through it and never stays `failed`.
        "activating" => Some(match sub {
            "auto-restart" | "auto-restart-queued" => Run::Failed,
            _ if owned => Run::Running,
            _ => Run::Starting,
        }),
        _ => None,
    }
}

/// Whether the sound runs in this session, from its `ActiveState`; `None` for a state the manual
/// doesn't list.
pub fn sound_runs(active: &str) -> Option<bool> {
    match active {
        "active" | "reloading" | "refreshing" => Some(true),
        "inactive" | "deactivating" | "maintenance" | "failed" | "activating" => Some(false),
        _ => None,
    }
}

/// The four facts (owner, 2026-11-11: on means ready to use).
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Facts {
    /// `opendeepcool.service` on the system manager: starts at boot.
    pub service_file: UnitFile,
    /// It runs.
    pub service: Run,
    /// `opendeepcool-audio.service` on the user's own manager, which reports a `--global` link
    /// as `enabled`: starts at every login.
    pub sound_file: UnitFile,
    /// It runs in this session.
    pub sound: bool,
}

impl Facts {
    /// All four hold: the switch is on.
    pub fn all_on(&self) -> bool {
        self.service_file == UnitFile::Enabled
            && self.service == Run::Running
            && self.sound_file == UnitFile::Enabled
            && self.sound
    }

    /// None holds: what a flip off wants.
    pub fn all_off(&self) -> bool {
        self.service_file == UnitFile::Disabled
            && matches!(self.service, Run::Stopped | Run::Failed)
            && self.sound_file == UnitFile::Disabled
            && !self.sound
    }

    /// Only the sound in this session is missing: a flip on just starts it, with no password,
    /// since nothing changes for the whole PC (owner, 2026-11-11).
    pub fn only_sound_missing(&self) -> bool {
        self.service_file == UnitFile::Enabled
            && self.service == Run::Running
            && self.sound_file == UnitFile::Enabled
            && !self.sound
    }

    /// The first unit whose file state is neither `enabled` nor `disabled`, and that state.
    fn other(&self) -> Option<(&'static str, &str)> {
        [(SERVICE, &self.service_file), (HELPER, &self.sound_file)]
            .into_iter()
            .find_map(|(unit, file)| match file {
                UnitFile::Other(state) => Some((unit, state.as_str())),
                _ => None,
            })
    }
}

/// What's known of the four facts.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub enum Known {
    /// Not read yet: the off screen's heading and note wait, so neither flashes.
    #[default]
    NotRead,
    /// A read failed, didn't answer within 1 s, or gave a state the manual doesn't list.
    Failed,
    Facts(Facts),
}

/// A flip that runs.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum Flip {
    #[default]
    Idle,
    /// Turning on: through `pkexec` when `password`, else only the sound in this session.
    /// `was_off`: the off screen read "OpenDeepCool is off" when it began, and keeps it until the
    /// flip ends.
    On {
        password: bool,
        was_off: bool,
    },
    /// After a successful `on`, until the service's bus name has an owner or 5 s pass, so the
    /// off screen's heading and note can't flash to "isn't running" in between.
    Waiting {
        was_off: bool,
    },
    Off,
}

/// How a flip ended, when it didn't change what it was meant to.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Failure {
    /// `pkexec` 126: the password dialog was closed. Nothing changed, and the switch shows the
    /// state, so it has no note and isn't kept (owner, 2026-11-19); it only ends the flip.
    Dismissed(Direction),
    /// `pkexec` 127: not authorized, or no polkit agent, or another error of pkexec's own.
    NotAuthorized(Direction),
    /// Any other exit, with the program's message: a step failed.
    Step(Direction, String),
    /// The app's own step: the sound didn't start in this session.
    SoundNotStarted,
    /// Turned off, but the sound didn't stop in this session.
    SoundNotStopped,
}

impl Failure {
    pub fn direction(&self) -> Direction {
        match self {
            Failure::Dismissed(to) | Failure::NotAuthorized(to) | Failure::Step(to, _) => *to,
            Failure::SoundNotStarted => Direction::On,
            Failure::SoundNotStopped => Direction::Off,
        }
    }

    /// Its note (States and their text); none for a closed dialog, which changed nothing.
    pub fn text(&self) -> Option<String> {
        Some(match self {
            Failure::Dismissed(_) => return None,
            Failure::NotAuthorized(_) => {
                "Not changed: the password wasn't accepted, or no password \
                                          dialog could open in this session."
                    .to_owned()
            }
            Failure::Step(_, message) => format!(
                "Didn't finish: {message}. The switch shows what's set now; the journal says more."
            ),
            Failure::SoundNotStarted => "Didn't finish: the sound for Music didn't start in this \
                                         session. It starts at your next login; the journal says \
                                         more."
                .to_owned(),
            Failure::SoundNotStopped => {
                "Off, but the sound for Music didn't stop in this session. \
                                         It stops when you log out."
                    .to_owned()
            }
        })
    }

    /// The commands it shows: its direction's, or none once the change for the whole PC was
    /// made.
    fn commands(&self) -> Option<&'static [&'static str]> {
        match self {
            Failure::SoundNotStopped => None,
            failure => Some(commands(failure.direction())),
        }
    }
}

/// The terminal commands for a direction (settings.md, Start at boot; owner, 2026-11-11): three
/// to turn on, the third starting the sound in this session at once, and two to turn off.
pub const ON_COMMANDS: [&str; 3] = [
    "sudo systemctl enable --now opendeepcool.service",
    "sudo systemctl --global enable opendeepcool-audio.service",
    "systemctl --user start opendeepcool-audio.service",
];
pub const OFF_COMMANDS: [&str; 2] = [
    "sudo systemctl disable --now opendeepcool.service",
    "sudo systemctl --global disable opendeepcool-audio.service",
];

pub fn commands(to: Direction) -> &'static [&'static str] {
    match to {
        Direction::On => &ON_COMMANDS,
        Direction::Off => &OFF_COMMANDS,
    }
}

/// The commands' lead-ins.
pub const FROM_A_TERMINAL: &str = "From a terminal instead:";
pub const TO_TURN_ON: &str = "To turn it on from a terminal:";

/// The off screen's headings and text (The off screen).
pub const IS_OFF: &str = "OpenDeepCool is off";
pub const NOT_RUNNING: &str = "OpenDeepCool isn't running";
pub const OFF_SCREEN_TEXT: &str = "The cooler shows nothing until the display service runs.";

/// The row's words and the card's hint (Configuration page, Startup).
pub const TITLE: &str = "Start at boot";
pub const SUBTITLE: &str = "For the whole PC";
pub const HINT: &str = "When it's on, the display service starts at every boot, and the sound for \
                        the Music options at every login.";

/// A running flip's lines.
pub const TURNING_ON: &str =
    "Turning on… Your desktop asks for the administrator's password first.";
pub const TURNING_ON_SOUND: &str = "Turning on… Starting the sound for Music in this session.";
pub const TURNING_OFF: &str =
    "Turning off… Your desktop asks for the administrator's password first.";

/// The information notes.
pub const READ_FAILED: &str =
    "Can't tell whether OpenDeepCool is on: reading it from systemd failed.";
pub const STARTING: &str = "The display service is starting.";

/// The note's phrases (The note): at most one for the display service and one for the sound.
pub const RUNS_NOT_AT_BOOT: &str = "the display service runs now, but doesn't start at boot";
pub const STOPPED: &str = "the display service is stopped";
pub const FAILED: &str = "the display service failed";
pub const SOUND_OFF: &str = "the sound for Music is off";
pub const SOUND_NOT_AT_LOGIN: &str = "the sound for Music runs here, but doesn't start at login";
pub const SOUND_NOT_HERE: &str = "the sound for Music isn't running in this session";

/// What screen readers get at a flip's end.
pub const NOW_ON: &str = "Start at boot is on.";
pub const NOW_OFF: &str = "Start at boot is off.";

/// What the switch shows of systemd's state and the flips, and what a flip does.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Boot {
    pub known: Known,
    pub flip: Flip,
    /// The last flip's failure, until the next flip, or until a read shows the state it wanted.
    pub failure: Option<Failure>,
    /// logind reports this session active and local, or can't tell (polkit decides then): the
    /// off screen shows the switch (Who may flip it).
    pub may_flip: bool,
}

impl Default for Boot {
    fn default() -> Boot {
        Boot {
            known: Known::NotRead,
            flip: Flip::Idle,
            failure: None,
            may_flip: true,
        }
    }
}

/// A flip's steps, in order; a failed one ends it.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Step {
    /// `pkexec --disable-internal-agent /usr/lib/opendeepcool/opendeepcool-boot on|off`.
    Pkexec(Direction),
    /// `StartUnit` or `StopUnit` of the sound on the user's own manager, with no password.
    StartSound,
    StopSound,
}

/// How `pkexec` ended.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Exit {
    Code(i32),
    Signal(i32),
    /// It couldn't be started.
    NotStarted(String),
}

/// The program's file name, which starts each line it writes.
fn program_name() -> &'static str {
    PROGRAM.rsplit('/').next().unwrap_or(PROGRAM)
}

/// The program's message on its standard error: the last line that starts with its name, without
/// the name and without a trailing period (A flip, step by step; `systemctl`'s own lines come
/// before it).
pub fn program_message(stderr: &str) -> Option<String> {
    let prefix = format!("{}: ", program_name());
    stderr
        .lines()
        .rev()
        .find_map(|line| line.strip_prefix(prefix.as_str()))
        .map(|message| message.trim_end().trim_end_matches('.').to_owned())
        .filter(|message| !message.is_empty())
}

/// What `pkexec`'s end means for a flip toward `to` (pkexec(1): 126 is only a dismissed dialog,
/// 127 every other refusal or error of its own; the program exits 1 or 2).
pub fn pkexec_failure(to: Direction, exit: &Exit, stderr: &str) -> Option<Failure> {
    match exit {
        Exit::Code(0) => None,
        Exit::Code(126) => Some(Failure::Dismissed(to)),
        Exit::Code(127) => Some(Failure::NotAuthorized(to)),
        Exit::Code(code) => Some(Failure::Step(
            to,
            program_message(stderr).unwrap_or_else(|| format!("exit status {code}")),
        )),
        Exit::Signal(signal) => Some(Failure::Step(
            to,
            format!("pkexec ended by signal {signal}"),
        )),
        Exit::NotStarted(why) => Some(Failure::Step(to, format!("pkexec can't start: {why}"))),
    }
}

impl Boot {
    fn facts(&self) -> Option<&Facts> {
        match &self.known {
            Known::Facts(facts) => Some(facts),
            _ => None,
        }
    }

    /// Whether the switch is on: all four facts hold.
    pub fn on(&self) -> bool {
        self.facts().is_some_and(Facts::all_on)
    }

    /// Whether a flip runs.
    pub fn flipping(&self) -> bool {
        self.flip != Flip::Idle
    }

    /// A read's result. A failure's note goes once the state its flip wanted shows (the user
    /// typed the commands).
    pub fn read(&mut self, known: Known) {
        if let (Some(failure), Known::Facts(facts)) = (&self.failure, &known) {
            let reached = match failure.direction() {
                Direction::On => facts.all_on(),
                Direction::Off => facts.all_off(),
            };
            if reached {
                self.failure = None;
            }
        }
        self.known = known;
    }

    /// The off screen's heading: "is off" while the display service doesn't start at boot or a
    /// flip toward off runs, else "isn't running"; none until the first read.
    fn off_heading(&self) -> Option<&'static str> {
        let off = match self.flip {
            Flip::Off => true,
            Flip::On { was_off, .. } | Flip::Waiting { was_off } => was_off,
            Flip::Idle => match &self.known {
                Known::NotRead => return None,
                Known::Failed => false,
                Known::Facts(facts) => facts.service_file == UnitFile::Disabled,
            },
        };
        Some(if off { IS_OFF } else { NOT_RUNNING })
    }

    /// A flip toward `to` (the handle's new position): its steps, with the flip and its line
    /// set and the last failure gone; or `None` when the switch can't be flipped now, or `to`
    /// isn't the way the switch goes from where it is (from off always on, from on off).
    pub fn begin(&mut self, to: Direction) -> Option<Vec<Step>> {
        if !switch(self, Place::Card).sensitive {
            return None;
        }
        let facts = self.facts()?;
        if (to == Direction::On) == facts.all_on() {
            return None;
        }
        let was_off = self.off_heading() == Some(IS_OFF);
        let (flip, steps) = match to {
            Direction::On if facts.only_sound_missing() => (
                Flip::On {
                    password: false,
                    was_off,
                },
                vec![Step::StartSound],
            ),
            Direction::On => (
                Flip::On {
                    password: true,
                    was_off,
                },
                vec![Step::Pkexec(Direction::On), Step::StartSound],
            ),
            Direction::Off => (
                Flip::Off,
                vec![Step::Pkexec(Direction::Off), Step::StopSound],
            ),
        };
        self.flip = flip;
        self.failure = None;
        Some(steps)
    }

    /// The flip's steps ended, with `failure` when one failed. After a successful `on` through
    /// `pkexec`, the flip counts as running until the service's bus name has an owner (`owned`),
    /// or [`Boot::owned`] says 5 s passed.
    pub fn end(&mut self, failure: Option<Failure>, owned: bool) {
        self.flip = match self.flip {
            Flip::On {
                password: true,
                was_off,
            } if failure.is_none() && !owned => Flip::Waiting { was_off },
            _ => Flip::Idle,
        };
        // A closed dialog ended the flip and is forgotten: it has no note to keep.
        self.failure = failure.filter(|failure| failure.text().is_some());
    }

    /// The service's bus name has an owner, or the 5 s wait after `on` ended.
    pub fn owned(&mut self) {
        if let Flip::Waiting { .. } = self.flip {
            self.flip = Flip::Idle;
        }
    }
}

/// How a note is drawn: a running flip's line in the foreground, without an icon; information
/// muted, with the information icon; a warning yellow, with the warning icon.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Style {
    Plain,
    Info,
    Warning,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Note {
    pub style: Style,
    pub text: String,
}

/// The terminal commands: the lead-in and one command a line.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Commands {
    pub lead: &'static str,
    pub lines: &'static [&'static str],
}

/// Where the switch is: the Configuration page's Startup card, or the off screen, where the
/// terminal commands always show under the card.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Place {
    Card,
    OffScreen,
}

/// The switch, its note and the terminal commands under it.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Switch {
    /// The handle (GtkSwitch's `active`): the latest flip.
    pub handle: bool,
    /// The trough (GtkSwitch's `state`): on only when all four facts hold.
    pub trough: bool,
    /// Insensitive while a flip runs, while the state is unknown, and while the display service
    /// is starting.
    pub sensitive: bool,
    pub note: Option<Note>,
    pub commands: Option<Commands>,
}

fn warning(text: String) -> Note {
    Note {
        style: Style::Warning,
        text,
    }
}

fn info(text: impl Into<String>) -> Note {
    Note {
        style: Style::Info,
        text: text.into(),
    }
}

/// The missing parts' note (The note), or none: with everything on, and with the display service
/// neither starting at boot nor running, which the off screen's heading says.
fn missing(facts: &Facts) -> Option<String> {
    let enabled = facts.service_file == UnitFile::Enabled;
    let display = match facts.service {
        Run::Running if enabled => None,
        Run::Running => Some(RUNS_NOT_AT_BOOT),
        Run::Stopped if enabled => Some(STOPPED),
        Run::Failed if enabled => Some(FAILED),
        // Starting has a note of its own; and a service that neither starts at boot nor runs
        // means OpenDeepCool is off, whatever the sound.
        Run::Starting | Run::Stopped | Run::Failed => return None,
    };
    let sound = match (facts.sound_file == UnitFile::Enabled, facts.sound) {
        (true, true) => None,
        (false, false) => Some(SOUND_OFF),
        (false, true) => Some(SOUND_NOT_AT_LOGIN),
        (true, false) => Some(SOUND_NOT_HERE),
    };
    let phrases: Vec<&str> = display.into_iter().chain(sound).collect();
    if phrases.is_empty() {
        return None;
    }
    let mut text = format!("Not fully on: {}.", phrases.join(", and "));
    text.push_str(if phrases == [SOUND_NOT_HERE] {
        " Turn it on to start it, with no password."
    } else {
        " Turn it on to finish."
    });
    if display == Some(FAILED) {
        text.push_str(" The journal says why the service failed.");
    }
    Some(text)
}

/// The switch at `place` (States and their text). Which note shows, when more than one applies:
/// a running flip's line, then a failure's note, then unknown, then starting, then the missing
/// parts.
pub fn switch(boot: &Boot, place: Place) -> Switch {
    let facts = boot.facts();
    let on = boot.on();
    let other = facts.and_then(Facts::other);
    let starting = facts.is_some_and(|f| f.service == Run::Starting);
    let sensitive = !boot.flipping() && facts.is_some() && other.is_none() && !starting;
    let handle = match boot.flip {
        Flip::On { .. } | Flip::Waiting { .. } => true,
        Flip::Off => false,
        Flip::Idle => on,
    };
    let line = match boot.flip {
        Flip::Idle => None,
        Flip::On {
            password: false, ..
        } => Some(TURNING_ON_SOUND),
        Flip::On { .. } | Flip::Waiting { .. } => Some(TURNING_ON),
        Flip::Off => Some(TURNING_OFF),
    };
    let (note, card_commands) = if let Some(line) = line {
        let plain = Note {
            style: Style::Plain,
            text: line.to_owned(),
        };
        (Some(plain), None)
    } else if let Some((failure, text)) = boot
        .failure
        .as_ref()
        .and_then(|failure| Some((failure, failure.text()?)))
    {
        let commands = failure.commands().map(|lines| Commands {
            lead: FROM_A_TERMINAL,
            lines,
        });
        (Some(warning(text)), commands)
    } else {
        match (&boot.known, facts) {
            (Known::Failed, _) => (
                Some(info(READ_FAILED)),
                Some(Commands {
                    lead: TO_TURN_ON,
                    lines: &ON_COMMANDS,
                }),
            ),
            (_, Some(facts)) => match other {
                Some((unit, state)) => (
                    Some(info(format!(
                        "Can't change it here: systemd reports {unit} as “{state}”."
                    ))),
                    None,
                ),
                None if starting => (Some(info(STARTING)), None),
                None => (missing(facts).map(warning), None),
            },
            _ => (None, None),
        }
    };
    let commands = match place {
        Place::Card => card_commands,
        // On the off screen, always under the card: the failed flip's direction, else on.
        Place::OffScreen => Some(Commands {
            lead: FROM_A_TERMINAL,
            lines: match card_commands {
                Some(Commands { lines, .. }) if lines == OFF_COMMANDS => &OFF_COMMANDS,
                _ => &ON_COMMANDS,
            },
        }),
    };
    Switch {
        handle,
        trough: on,
        sensitive,
        note,
        commands,
    }
}

/// The off screen (The off screen): shown while the service's bus name has no owner.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct OffScreen {
    /// "OpenDeepCool is off" or "OpenDeepCool isn't running"; none until the first read, so
    /// neither flashes.
    pub heading: Option<&'static str>,
    /// The Startup card, in a session that may flip it (Who may flip it).
    pub card: bool,
    pub switch: Switch,
}

pub fn off_screen(boot: &Boot) -> OffScreen {
    let heading = boot.off_heading();
    let mut switch = switch(boot, Place::OffScreen);
    if heading.is_none() {
        switch.commands = None;
    }
    OffScreen {
        heading,
        card: boot.may_flip && heading.is_some(),
        switch,
    }
}

/// Whether the off screen's heading says OpenDeepCool is off on purpose ("Service off", a hollow
/// dot), or that it isn't running ("Service not running", red); none until the first read.
pub fn service_off(boot: &Boot) -> Option<bool> {
    boot.off_heading().map(|heading| heading == IS_OFF)
}

#[cfg(test)]
mod tests;
