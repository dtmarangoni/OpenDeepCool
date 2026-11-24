use super::*;

// dbus-daemon(1): the interface field of a message is optional, so every send rule names the
// destination with an interface, a path or a message type; only the service's account owns the
// name; only group opendeepcool reaches the dev path.
#[test]
fn the_dbus_policy_is_scoped_to_the_service() {
    let path = packaging(DBUS);
    let text = fs::read_to_string(&path).expect("the policy");
    let (ok, out) = run("xmllint", &["--noout", "--nonet", &path.to_string_lossy()]);
    assert!(ok, "{out}");
    let all = rules(&text);
    assert_eq!(all.len(), 5);
    for rule in &all {
        let get = |key: &str| rule.iter().find(|(k, _)| k == key).map(|(_, v)| v.as_str());
        if let Some(name) = get("own") {
            assert_eq!(name, BUS_NAME);
            continue;
        }
        assert_eq!(get("send_destination"), Some(BUS_NAME), "{rule:?}");
        assert!(
            get("send_interface").is_some()
                || get("send_path").is_some()
                || get("send_type").is_some(),
            "{rule:?}"
        );
    }
    let owner = block(&text, "<policy user=\"opendeepcool\">", "policy");
    assert!(owner.contains(&format!("<allow own=\"{BUS_NAME}\"/>")));
    let everyone = block(&text, "<policy context=\"default\">", "policy");
    assert!(everyone.contains(&format!("send_interface=\"{INTERFACE}\"")));
    assert!(everyone.contains(&format!("send_path=\"{OBJECT_PATH}\"")));
    assert!(
        !everyone.contains(DEV_PATH),
        "the dev path isn't everyone's"
    );
    let group = block(&text, "<policy group=\"opendeepcool\">", "policy");
    assert!(group.contains(&format!("send_path=\"{DEV_PATH}\"")));
}
