//! The AT-SPI side (Phase 10 plan, Task 8): it finds the app's widgets on the accessibility bus
//! by role and accessible name, acts on them through their AT-SPI actions and values, and reads
//! their names, descriptions, states, text and values. Every read is fresh: no proxy keeps a
//! property cache, so nothing comes from an earlier answer, and every wait reads again every
//! 50 ms for up to 5 s (the spike's lesson: one action, then wait for its effect). Only widgets
//! GTK shows are in the tree, so a widget that isn't found isn't shown.

use std::fmt::Write as _;
use std::thread;
use std::time::{Duration, Instant};

use atspi::proxy::accessible::{AccessibleProxy, ObjectRefExt};
use atspi::proxy::action::ActionProxy;
use atspi::proxy::text::TextProxy;
use atspi::proxy::value::ValueProxy;
use atspi::zbus;
use atspi::zbus::proxy::CacheProperties;
use atspi::{Interface, ObjectRefOwned, State, StateSet};

/// How often a wait reads again, and for how long.
pub const POLL: Duration = Duration::from_millis(50);
pub const WAIT: Duration = Duration::from_secs(5);

/// The app's name on the accessibility bus: its program's name.
const APP: &str = "opendeepcool";

const REGISTRY: &str = "org.a11y.atspi.Registry";
const ROOT: &str = "/org/a11y/atspi/accessible/root";

/// A widget in the app's tree, with its role as GTK names it (`GetRoleName`: "button", "toggle
/// button", "switch"), since atspi's role list lacks some of GTK's, such as the switch.
#[derive(Clone, Debug)]
pub struct Node {
    object: ObjectRefOwned,
    pub role: String,
    pub name: String,
}

pub type Result<T> = std::result::Result<T, String>;

fn bus(e: impl std::fmt::Display) -> String {
    format!("AT-SPI: {e}")
}

/// Waits until `check` gives a value, reading again every 50 ms, for up to 5 s; then fails
/// with `what` and the last answer.
pub fn wait<T>(what: &str, mut check: impl FnMut() -> Result<Option<T>>) -> Result<T> {
    let until = Instant::now() + WAIT;
    loop {
        let last = match check() {
            Ok(Some(found)) => return Ok(found),
            Ok(None) => String::new(),
            Err(e) => e,
        };
        if Instant::now() >= until {
            let last = if last.is_empty() {
                String::new()
            } else {
                format!(" ({last})")
            };
            return Err(format!("{what}: not within {WAIT:?}{last}"));
        }
        thread::sleep(POLL);
    }
}

pub struct Driver {
    conn: zbus::Connection,
    app: ObjectRefOwned,
}

impl Driver {
    /// Connects to the accessibility bus at `address` and waits for the app's root there.
    ///
    /// # Errors
    ///
    /// When the bus can't be reached, or the app isn't on it within 5 s.
    pub fn connect(address: &str) -> Result<Driver> {
        let conn =
            zbus::block_on(async { zbus::connection::Builder::address(address)?.build().await })
                .map_err(bus)?;
        let app = wait("the app on the accessibility bus", || {
            zbus::block_on(async {
                let root = AccessibleProxy::builder(&conn)
                    .destination(REGISTRY)?
                    .path(ROOT)?
                    .cache_properties(CacheProperties::No)
                    .build()
                    .await?;
                for child in root.get_children().await? {
                    let proxy = child.as_accessible_proxy(&conn).await;
                    if let Ok(proxy) = proxy
                        && proxy.name().await.is_ok_and(|name| name == APP)
                    {
                        return Ok(Some(child));
                    }
                }
                Ok(None)
            })
            .map_err(|e: zbus::Error| bus(e))
        })?;
        Ok(Driver { conn, app })
    }

    fn proxy<'a>(&'a self, object: &'a ObjectRefOwned) -> Result<AccessibleProxy<'a>> {
        zbus::block_on(object.as_accessible_proxy(&self.conn)).map_err(bus)
    }

    async fn node(&self, object: ObjectRefOwned) -> zbus::Result<Node> {
        let proxy = object
            .as_accessible_proxy(&self.conn)
            .await
            .map_err(|e| zbus::Error::Failure(e.to_string()))?;
        let role = proxy.get_role_name().await?;
        let name = proxy.name().await?;
        Ok(Node { object, role, name })
    }

    /// Every widget shown, depth first from the app's root, each with its depth.
    pub fn tree(&self) -> Result<Vec<(usize, Node)>> {
        zbus::block_on(async {
            let mut found = Vec::new();
            let mut stack = vec![(0, self.app.clone())];
            while let Some((depth, object)) = stack.pop() {
                // A widget that went away meanwhile is left out.
                let Ok(node) = self.node(object.clone()).await else {
                    continue;
                };
                let proxy = object
                    .as_accessible_proxy(&self.conn)
                    .await
                    .map_err(|e| zbus::Error::Failure(e.to_string()))?;
                let children = proxy.get_children().await.unwrap_or_default();
                for child in children.into_iter().rev() {
                    stack.push((depth + 1, child));
                }
                found.push((depth, node));
            }
            Ok(found)
        })
        .map_err(|e: zbus::Error| bus(e))
    }

    /// Every widget shown with `role` and the accessible name `name`.
    pub fn all(&self, role: &str, name: &str) -> Result<Vec<Node>> {
        Ok(self
            .tree()?
            .into_iter()
            .map(|(_, node)| node)
            .filter(|node| node.role == role && node.name == name)
            .collect())
    }

    /// The one widget shown with `role` and `name`, or none.
    ///
    /// # Errors
    ///
    /// When two are shown: a name must say which widget it is.
    pub fn find(&self, role: &str, name: &str) -> Result<Option<Node>> {
        let mut found = self.all(role, name)?;
        match found.len() {
            0 | 1 => Ok(found.pop()),
            n => Err(format!("{n} widgets, {role} {name:?}")),
        }
    }

    /// Waits until `name` is shown.
    pub fn shown(&self, role: &str, name: &str) -> Result<Node> {
        wait(&format!("{role} {name:?} shown"), || self.find(role, name))
    }

    /// Waits until `name` is no longer shown.
    pub fn gone(&self, role: &str, name: &str) -> Result<()> {
        wait(&format!("{role} {name:?} gone"), || {
            Ok(self.find(role, name)?.is_none().then_some(()))
        })
    }

    /// `node` and every widget shown under it, depth first.
    pub fn subtree(&self, node: &Node) -> Result<Vec<Node>> {
        let mut found = Vec::new();
        let mut stack = vec![node.clone()];
        while let Some(node) = stack.pop() {
            let mut children = self.children(&node)?;
            children.reverse();
            stack.extend(children);
            found.push(node);
        }
        Ok(found)
    }

    /// The one widget shown under `node` with `role` and `name`, or none.
    pub fn find_in(&self, node: &Node, role: &str, name: &str) -> Result<Option<Node>> {
        let mut found: Vec<Node> = self
            .subtree(node)?
            .into_iter()
            .filter(|n| n.role == role && n.name == name)
            .collect();
        match found.len() {
            0 | 1 => Ok(found.pop()),
            n => Err(format!(
                "{n} widgets, {role} {name:?}, under {:?}",
                node.name
            )),
        }
    }

    /// The text of every label shown under `node`, `node` first when it's one, depth first.
    pub fn texts(&self, node: &Node) -> Result<Vec<String>> {
        let mut texts = Vec::new();
        for node in self.subtree(node)? {
            if node.role == "label" {
                texts.push(self.text(&node)?);
            }
        }
        Ok(texts)
    }

    pub fn children(&self, node: &Node) -> Result<Vec<Node>> {
        let proxy = self.proxy(&node.object)?;
        zbus::block_on(async {
            let mut nodes = Vec::new();
            for child in proxy.get_children().await? {
                if let Ok(node) = self.node(child).await {
                    nodes.push(node);
                }
            }
            Ok(nodes)
        })
        .map_err(|e: zbus::Error| bus(e))
    }

    pub fn states(&self, node: &Node) -> Result<StateSet> {
        let proxy = self.proxy(&node.object)?;
        zbus::block_on(proxy.get_state()).map_err(bus)
    }

    pub fn has(&self, node: &Node, state: State) -> Result<bool> {
        Ok(self.states(node)?.contains(state))
    }

    pub fn description(&self, node: &Node) -> Result<String> {
        let proxy = self.proxy(&node.object)?;
        zbus::block_on(proxy.description()).map_err(bus)
    }

    fn interfaces(&self, node: &Node) -> Result<atspi::InterfaceSet> {
        let proxy = self.proxy(&node.object)?;
        zbus::block_on(proxy.get_interfaces()).map_err(bus)
    }

    /// A label's text, through its Text interface.
    pub fn text(&self, node: &Node) -> Result<String> {
        if !self.interfaces(node)?.contains(Interface::Text) {
            return Err(format!("{} {:?} has no text", node.role, node.name));
        }
        zbus::block_on(async {
            let text = TextProxy::builder(&self.conn)
                .destination(self.destination(node)?)?
                .path(node.object.path_as_str())?
                .cache_properties(CacheProperties::No)
                .build()
                .await?;
            text.get_text(0, -1).await
        })
        .map_err(bus)
    }

    fn destination(&self, node: &Node) -> zbus::Result<String> {
        node.object
            .name_as_str()
            .map(str::to_owned)
            .ok_or_else(|| zbus::Error::Failure("a widget without a bus name".to_owned()))
    }

    async fn action_proxy(&self, node: &Node) -> zbus::Result<ActionProxy<'_>> {
        ActionProxy::builder(&self.conn)
            .destination(self.destination(node)?)?
            .path(node.object.path_as_str().to_owned())?
            .cache_properties(CacheProperties::No)
            .build()
            .await
    }

    /// The names of a widget's actions, and their key bindings.
    pub fn actions(&self, node: &Node) -> Result<Vec<(String, String)>> {
        zbus::block_on(async {
            let actions = self.action_proxy(node).await?;
            let list = actions.get_actions().await?;
            Ok(list
                .into_iter()
                .map(|action| (action.name, action.keybinding))
                .collect())
        })
        .map_err(|e: zbus::Error| bus(e))
    }

    /// Runs the widget's action named `action`.
    pub fn act(&self, node: &Node, action: &str) -> Result<()> {
        let actions = self.actions(node)?;
        let index = actions
            .iter()
            .position(|(name, _)| name == action)
            .ok_or_else(|| {
                format!(
                    "{} {:?} has no action {action:?}, only {:?}",
                    node.role,
                    node.name,
                    actions.iter().map(|(n, _)| n).collect::<Vec<_>>()
                )
            })?;
        let done = zbus::block_on(async {
            let proxy = self.action_proxy(node).await?;
            proxy
                .do_action(i32::try_from(index).unwrap_or(i32::MAX))
                .await
        })
        .map_err(bus)?;
        if done {
            Ok(())
        } else {
            Err(format!(
                "{} {:?}: {action:?} did nothing",
                node.role, node.name
            ))
        }
    }

    /// A synthetic key press and release, by its keysym, through the registry's device event
    /// controller.
    pub fn key_sym(&self, keysym: i32) -> Result<()> {
        use atspi::proxy::device_event_controller::{DeviceEventControllerProxy, KeySynthType};
        zbus::block_on(async {
            let dec = DeviceEventControllerProxy::builder(&self.conn)
                .destination(REGISTRY)?
                .path("/org/a11y/atspi/registry/deviceeventcontroller")?
                .cache_properties(CacheProperties::No)
                .build()
                .await?;
            dec.generate_keyboard_event(keysym, "", KeySynthType::Sym)
                .await
        })
        .map_err(bus)
    }

    async fn value_proxy(&self, node: &Node) -> zbus::Result<ValueProxy<'_>> {
        ValueProxy::builder(&self.conn)
            .destination(self.destination(node)?)?
            .path(node.object.path_as_str().to_owned())?
            .cache_properties(CacheProperties::No)
            .build()
            .await
    }

    /// A value widget's value and its value text.
    pub fn value(&self, node: &Node) -> Result<(f64, String)> {
        zbus::block_on(async {
            let value = self.value_proxy(node).await?;
            Ok((value.current_value().await?, value.text().await?))
        })
        .map_err(|e: zbus::Error| bus(e))
    }

    pub fn set_value(&self, node: &Node, to: f64) -> Result<()> {
        zbus::block_on(async {
            let value = self.value_proxy(node).await?;
            value.set_current_value(to).await
        })
        .map_err(bus)
    }

    /// The tree as text, one widget a line, for a failure's report.
    pub fn dump(&self) -> String {
        let mut out = String::new();
        let tree = match self.tree() {
            Ok(tree) => tree,
            Err(e) => return e,
        };
        for (depth, node) in tree {
            let states: Vec<String> = self
                .states(&node)
                .map(|s| s.iter().map(|s| format!("{s:?}")).collect())
                .unwrap_or_default();
            let description = self.description(&node).unwrap_or_default();
            let text = self.text(&node).ok();
            let actions: Vec<String> = self
                .actions(&node)
                .map(|a| a.into_iter().map(|(n, k)| format!("{n}[{k}]")).collect())
                .unwrap_or_default();
            let _ = write!(
                out,
                "{:indent$}{} {:?} states={states:?}",
                "",
                node.role,
                node.name,
                indent = depth * 2
            );
            if !description.is_empty() {
                let _ = write!(out, " description={description:?}");
            }
            if let Some(text) = text {
                let _ = write!(out, " text={text:?}");
            }
            if !actions.is_empty() {
                let _ = write!(out, " actions={actions:?}");
            }
            if let Ok((value, text)) = self.value(&node) {
                let _ = write!(out, " value={value} {text:?}");
            }
            if let Ok(interfaces) = self.interfaces(&node) {
                let shown: Vec<String> = interfaces
                    .iter()
                    .filter(|i| !matches!(i, Interface::Accessible | Interface::Component))
                    .map(|i| format!("{i:?}"))
                    .collect();
                if !shown.is_empty() {
                    let _ = write!(out, " interfaces={shown:?}");
                }
            }
            out.push('\n');
        }
        out
    }
}
