use super::*;

#[test]
fn get_state_returns_every_key_of_the_contract_with_its_type() {
    let h = Harness::start(
        "state",
        Settings::default(),
        status(readings(value(51))),
        false,
    );
    zbus::block_on(async {
        let caller = h.client().await;
        let dict = caller.main.get_state().await.expect("the state");
        let mut got: Vec<(String, String)> = dict
            .iter()
            .map(|(key, value)| (key.clone(), value.value_signature().to_string()))
            .collect();
        got.sort();
        let mut want: Vec<(String, String)> = STATE_KEYS
            .iter()
            .map(|(key, signature)| ((*key).to_owned(), (*signature).to_owned()))
            .collect();
        want.sort();
        assert_eq!(got, want);
        let state = State::from_dict(&dict).expect("a state");
        assert_eq!(state.mode, Some(1));
        assert_eq!(state.cooler_state, Some(CoolerState::Connected));
        assert_eq!(state.chosen_gpu.as_deref(), Some(""));
        let gpus = state.gpus.expect("Gpus");
        assert_eq!(gpus[0].pci, "0000:0a:00.0");
        assert_eq!(gpus[0].kind, odc_ipc::GpuKind::Dedicated);
    });
}

#[test]
fn diagnostics_list_each_metric_with_its_source_and_state() {
    let mut readings = readings(value(51));
    readings = Snapshot::new(|metric| match metric {
        Metric::CpuPower => Reading::Unavailable(Unavailable::PermissionDenied {
            path: PathBuf::from("/fake/energy_uj"),
        }),
        other => readings.get(other).clone(),
    });
    let h = Harness::start("diagnostics", Settings::default(), status(readings), false);
    zbus::block_on(async {
        let caller = h.client().await;
        let list = caller.main.diagnostics().await.expect("the list");
        let names: Vec<&str> = list.iter().map(|(name, _, _)| name.as_str()).collect();
        let want: Vec<&str> = Metric::ALL.iter().map(|m| m.spec_name()).collect();
        assert_eq!(names, want, "all seven, in the spec's order");
        assert_eq!(
            list[0],
            (
                "CPU Temperature".to_owned(),
                "fake CPU Temperature".to_owned(),
                "ok".to_owned()
            )
        );
        assert_eq!(
            list[2],
            (
                "CPU Power".to_owned(),
                String::new(),
                "no permission to read it".to_owned()
            )
        );
    });
}

#[test]
fn the_introspection_is_the_contract_and_lists_no_properties() {
    let h = Harness::start(
        "contract",
        Settings::default(),
        status(readings(value(51))),
        true,
    );
    zbus::block_on(async {
        let caller = h.client().await;
        let main = introspect(&caller.conn, OBJECT_PATH).await;
        let documented = dbus_md();
        assert_eq!(members(&main, INTERFACE), members(&documented, INTERFACE));
        assert!(
            !members(&main, INTERFACE)
                .iter()
                .any(|m| m.starts_with("property")),
            "no properties"
        );
        let dev = introspect(&caller.conn, DEV_PATH).await;
        assert_eq!(
            members(&dev, DEV_INTERFACE),
            members(&documented, DEV_INTERFACE)
        );
    });
}

#[test]
fn without_dev_the_dev_interface_and_its_path_do_not_exist() {
    let h = Harness::start(
        "no-dev",
        Settings::default(),
        status(readings(value(51))),
        false,
    );
    zbus::block_on(async {
        let caller = h.client().await;
        let root = introspect(&caller.conn, OBJECT_PATH).await;
        assert!(!root.contains(DEV_INTERFACE));
        assert!(!root.contains("<node name=\"Dev\""), "{root}");
        match caller.dev.begin_test().await {
            Err(Error::ZBus(zbus::Error::FDO(e))) => {
                assert!(matches!(*e, zbus::fdo::Error::UnknownObject(_)), "{e:?}");
            }
            other => panic!("{other:?}"),
        }
    });
}
