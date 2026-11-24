use super::*;

#[test]
fn polkit_is_asked_by_bus_name_for_the_action_without_a_prompt() {
    for allowed in [true, false] {
        let asked = Arc::new(Mutex::new(Vec::new()));
        let authority = FakeAuthority {
            asked: Arc::clone(&asked),
            allowed,
        };
        zbus::block_on(async {
            let (ours, theirs) = UnixStream::pair().expect("a socket pair");
            let server = connection::Builder::unix_stream(theirs)
                .server(Guid::generate())
                .p2p()
                .serve_at("/org/freedesktop/PolicyKit1/Authority", authority)
                .build();
            let client = connection::Builder::unix_stream(ours).p2p().build();
            let (server, client) = both(server, client).await;
            let (_server, client) = (server.expect("the server"), client.expect("the client"));
            let polkit = PolkitAuthorizer::new(&client).await.expect("polkit");
            for action in [Action::Use, Action::ChangeSettings] {
                assert_eq!(polkit.allowed(":1.42", action).await, Ok(allowed));
            }
        });
        let asked = asked.lock().expect("asked");
        assert_eq!(asked.len(), 2);
        for ((signature, question), action) in
            asked.iter().zip([USE_ACTION, CHANGE_SETTINGS_ACTION])
        {
            // polkit 128's declared signature, introspected on 2026-10-29.
            assert_eq!(signature, "(sa{sv})sa{ss}us");
            let ((kind, subject), action_id, details, flags, cancellation) = question;
            assert_eq!(kind, "system-bus-name");
            assert_eq!(subject.len(), 1);
            let name: String = subject["name"]
                .try_clone()
                .expect("a string")
                .try_into()
                .expect("a string");
            assert_eq!(name, ":1.42");
            assert_eq!(action_id, action);
            assert!(details.is_empty());
            assert_eq!(*flags, 0, "no interactive prompt");
            assert_eq!(cancellation, "");
        }
    }
}

#[test]
fn the_bus_says_who_a_caller_is() {
    let dir = dir("credentials");
    let bus = PrivateBus::start(&dir.join("bus")).expect("a private bus");
    let mine = fs::metadata(&dir).expect("the directory").uid();
    zbus::block_on(async {
        let conn = connection::Builder::address(bus.address())
            .build()
            .await
            .expect("a connection");
        let credentials = BusCredentials::new(&conn).await.expect("credentials");
        let name = conn.unique_name().expect("a name").to_string();
        let identity = credentials.of(&name).await.expect("an identity");
        assert_eq!(identity.user, Some(mine));
        assert!(identity.groups.is_some_and(|g| !g.is_empty()));
    });
}

#[test]
fn every_reason_has_its_kind_and_back() {
    let reasons = [
        Unavailable::NoRule { driver: "x".into() },
        Unavailable::NoDevice,
        Unavailable::PermissionDenied {
            path: PathBuf::new(),
        },
        Unavailable::ReadFailed {
            path: PathBuf::new(),
        },
        Unavailable::NotYet,
        Unavailable::IntegratedGpuPower,
        Unavailable::UnknownGpuKind,
        Unavailable::DriverUpdated,
        Unavailable::LibraryFailed {
            library: "x".into(),
            error: "y".into(),
        },
    ];
    let kinds: HashSet<&str> = reasons.iter().map(Unavailable::kind).collect();
    assert_eq!(kinds.len(), reasons.len(), "one kind each");
    for reason in &reasons {
        let back = Unavailable::from_kind(reason.kind()).expect("its kind is taken");
        assert_eq!(back.kind(), reason.kind());
    }
    assert_eq!(Unavailable::from_kind("gone"), None);
}
