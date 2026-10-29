# D-Bus interface

The contract between OpenDeepCool's display service and its clients: the settings app (Phase 10), the tests, and `busctl` until the app exists. It's OpenDeepCool's own; DeepCreative's window talks to its own main process instead. The crate `odc-ipc` holds the same names, state keys and members, and its tests check this document against them; the service's tests check its introspection against the XML below (Phase 7 plan, Tasks 4 and 7; owner, 2026-10-27, solution review P4-04).

Later phases change this document in the same commit as their code, and `settings.md`'s "What the settings app must offer" with it, so the settings app's design follows. From `v1.0.0` it's frozen: an incompatible change gets a new interface, `…OpenDeepCool2`, next to this one (owner, 2026-10-24).

## Names

| What | Name |
|---|---|
| Bus name, on the system bus | `io.github.dtmarangoni.OpenDeepCool1` |
| Main interface | `io.github.dtmarangoni.OpenDeepCool1`, at `/io/github/dtmarangoni/OpenDeepCool1` |
| Dev interface, only when the service starts with `--dev` | `io.github.dtmarangoni.OpenDeepCool1.Dev`, at `/io/github/dtmarangoni/OpenDeepCool1/Dev` |
| polkit action for every read | `io.github.dtmarangoni.OpenDeepCool1.use` |
| polkit action for every change, `Refresh()` included | `io.github.dtmarangoni.OpenDeepCool1.change-settings` |
| Error names | `io.github.dtmarangoni.OpenDeepCool1.Error.<name>` |

The names start with the project's public address, its GitHub repository, as the D-Bus specification asks of a name's owner; the `1` is the interface's major version (owner, 2026-10-24).

## Who may call

- **Every call is checked with polkit**, reads and changes alike, and both actions are granted to the active local session only: `allow_active` yes, `allow_inactive` no, `allow_any` no. No remote or inactive session reads or changes anything (owner, 2026-10-26).
- **No D-Bus properties.** Anyone may read a property through `org.freedesktop.DBus.Properties`, and `PropertiesChanged` reaches every local process. The state is read with `GetState()` instead, and changes come as `StateChanged`, a signal addressed to each watcher alone.
- **Watchers:** `Watch()` registers the caller, at most 8 per Unix user and 32 in all. A watcher is dropped on `Unwatch()`, or when its connection leaves the bus. Each one is checked again every second: one that isn't the active local session any more gets nothing until it passes again, then the whole state in one `StateChanged`.
- **Arguments** are compared only with what the service found itself: a GPU must be listed in `Gpus`, a fan input in `FanInputs`. A string over its limit (64 bytes; 1024 for a device path) is refused before any lookup, and no file path is ever built from a caller's string (owner, 2026-10-26).
- **The dev interface** exists only when the service starts with `--dev`, at its own object path. Its calls pass three checks: the D-Bus policy lets only group `opendeepcool` send to its path; each method checks the caller's `UnixGroupIDs` from `GetConnectionCredentials`, a missing list meaning no; and polkit's `change-settings` (owner, 2026-10-26).

## Main interface

```xml
<node>
  <interface name="io.github.dtmarangoni.OpenDeepCool1">
    <method name="SetMode">
      <arg name="mode" type="u" direction="in"/>
    </method>
    <method name="RotateScreen"/>
    <method name="SetRotation">
      <arg name="rotation" type="y" direction="in"/>
    </method>
    <method name="SetTemperatureUnit">
      <arg name="unit" type="s" direction="in"/>
    </method>
    <method name="ChooseGpu">
      <arg name="pci" type="s" direction="in"/>
    </method>
    <method name="SetCoolerFanInput">
      <arg name="chip" type="s" direction="in"/>
      <arg name="device" type="s" direction="in"/>
      <arg name="input" type="u" direction="in"/>
    </method>
    <method name="SetPumpInput">
      <arg name="chip" type="s" direction="in"/>
      <arg name="device" type="s" direction="in"/>
      <arg name="input" type="u" direction="in"/>
    </method>
    <method name="Refresh"/>
    <method name="GetState">
      <arg name="state" type="a{sv}" direction="out"/>
    </method>
    <method name="Watch"/>
    <method name="Unwatch"/>
    <method name="Diagnostics">
      <arg name="metrics" type="a(sss)" direction="out"/>
    </method>
    <signal name="StateChanged">
      <arg name="changes" type="a{sv}"/>
    </signal>
  </interface>
</node>
```

| Member | polkit action | What it does | Errors |
|---|---|---|---|
| `SetMode(u)` | `change-settings` | Shows mode *u* from its home, on the next frame | `NotAuthorized`, `InvalidArgument`, `NotYetAvailable`, `ModeUnavailable` |
| `RotateScreen()` | `change-settings` | One press of Rotate Screen: a quarter turn clockwise, 3 wrapping to 0 ([settings.md](settings.md)) | `NotAuthorized` |
| `SetRotation(y)` | `change-settings` | Rotation 0–3 | `NotAuthorized`, `InvalidArgument` |
| `SetTemperatureUnit(s)` | `change-settings` | `celsius` or `fahrenheit`; in a mode without a temperature it's only saved | `NotAuthorized`, `InvalidArgument` |
| `ChooseGpu(s)` | `change-settings` | The GPU at a PCI address listed in `Gpus` | `NotAuthorized`, `InvalidArgument` |
| `SetCoolerFanInput(ssu)`, `SetPumpInput(ssu)` | `change-settings` | The fan input listed in `FanInputs` with that chip, device path and number, or all empty for none | `NotAuthorized`, `InvalidArgument` |
| `Refresh()` | `change-settings` | Fresh readings at once, and the picture from home; nothing is saved | `NotAuthorized` |
| `GetState() → a{sv}` | `use` | The whole state ([State](#state)) | `NotAuthorized` |
| `Watch()`, `Unwatch()` | `use` | Starts or stops `StateChanged` for this caller | `NotAuthorized`, `TooManyWatchers` |
| `Diagnostics() → a(sss)` | `use` | Each metric: its name, its bound source, and its state | `NotAuthorized` |
| `StateChanged(a{sv})` | `use`, checked every second | The keys that changed; the whole state after a watcher passes its check again | — |

Choosing what is already set succeeds and changes nothing: nothing is saved, sent to the display or signalled.

## State

`GetState()` returns every key; `StateChanged` sends the keys that changed. A client ignores keys it doesn't know, so later phases add keys with their meaning here.

| Key | Type | Meaning |
|---|---|---|
| `Mode` | `u` | The mode shown |
| `Rotation` | `y` | 0–3, quarter turns clockwise |
| `TemperatureUnit` | `s` | `celsius` or `fahrenheit` |
| `CoolerConnected` | `b` | Whether the last frame reached the cooler |
| `CoolerState` | `s` | `connected`, `not-found`, `no-access` or `write-failed`, from the last write (owner, 2026-10-27; solution review P5-02, P7-14); on the dev copy also `released`, while a test holds the cooler (owner, 2026-10-29) |
| `AvailableModes` | `au` | The modes the service draws whose every metric can be read on this PC |
| `UnavailableReasons` | `a{us}` | Each mode left out of `AvailableModes`, with a one-line reason |
| `Gpus` | `a(sss)` | Each GPU, best first: PCI address, driver, and kind, `dedicated`, `integrated`, `unknown` or `query-failed`, the last ranked as unknown (owner, 2026-10-27; solution review P6-07) |
| `ChosenGpu` | `s` | The PCI address the user chose, empty when none was chosen |
| `FanInputs` | `a(ssus)` | Each fan input: chip, device path relative to the sysfs root (`devices/…`), input number, and the driver's label, empty when it gives none |
| `CoolerFanInput` | `(ssu)` | The input chosen for the cooler's fans: chip, device path and number, all empty when none is chosen |
| `PumpInput` | `(ssu)` | The same, for the pump |
| `CoolerFanRpm` | `d` | The cooler's fans, in whole RPM capped at 9999 (owner, 2026-10-25); NaN when no input is chosen or it can't be read |
| `PumpRpm` | `d` | The same, for the pump |

## Errors

| Name | When |
|---|---|
| `…Error.NotAuthorized` | polkit refused the caller |
| `…Error.ModeUnavailable` | The mode's metrics can't be read on this PC; the text gives the reason |
| `…Error.NotYetAvailable` | A mode a later phase draws |
| `…Error.InvalidArgument` | An argument outside what the service found or allows, or over its length limit |
| `…Error.TooManyWatchers` | The caller's user already has 8 watchers, or there are 32 in all |
| `…Error.Busy` | The dev copy only: a test session holds the service ("a test is running on this dev copy") |

## Dev interface

Only when the service starts with `--dev`, which only the dev copy's drop-in passes; the packaged unit never does (owner, 2026-10-26; solution review OV-13).

```xml
<node>
  <interface name="io.github.dtmarangoni.OpenDeepCool1.Dev">
    <method name="OverrideReadings">
      <arg name="values" type="a{s(xt)}" direction="in"/>
      <arg name="unavailable" type="a{ss}" direction="in"/>
      <arg name="tick" type="t" direction="out"/>
    </method>
    <method name="ClearOverride"/>
    <method name="BeginTest"/>
    <method name="EndTest"/>
    <method name="ReleaseCooler"/>
  </interface>
</node>
```

| Member | What it does |
|---|---|
| `OverrideReadings(a{s(xt)}, a{ss}) → t` | Exact values (metric → numerator and denominator) and unavailable metrics with their reasons replace the sensor thread's snapshots from the next tick on, whose number it returns. It needs the test session |
| `ClearOverride()` | Back to the real readings |
| `BeginTest()`, `EndTest()` | The test session: while one caller holds it, every change from any other caller gets `Busy`, and the frame recording runs. It ends, with every override and the recording, on `EndTest()` or when the holder's connection leaves the bus (owner, 2026-10-27; solution review P7-16) |
| `ReleaseCooler()` | The session's holder only: the service closes the cooler and writes nothing until the session ends, then starts a new conversation, `03 08` first, so the direct checks share the cooler with the running dev service (owner, 2026-10-29) |
