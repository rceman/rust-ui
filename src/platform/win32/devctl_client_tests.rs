
    use super::*;
    use crate::devtools::server::*;
    #[test]
    fn rejected_request_cannot_execute_after_wake_failure() {
        let mailbox = crate::tasks::Mailbox::new();
        mailbox.install_wake(Arc::new(|| false)).unwrap();
        let (tx, rx) = mpsc::sync_channel(1);
        let (reply, _) = mpsc::sync_channel(1);
        let permit = Arc::new(std::sync::Mutex::new(Admission::Waking));
        let e = Envelope {
            request: Request {
                protocol: PROTOCOL.into(),
                id: 1,
                token: "test".into(),
                command: "click".into(),
                arguments: Value::Null,
            },
            reply,
            deadline: Instant::now() + Duration::from_secs(1),
            admission: permit.clone(),
        };
        assert_eq!(
            enqueue_request(&tx, &mailbox, e).unwrap_err().code,
            "SESSION_CLOSED"
        );
        assert_eq!(
            *rx.recv().unwrap().admission.lock().unwrap(),
            Admission::Rejected
        );
        assert_eq!(*permit.lock().unwrap(), Admission::Rejected);
    }
    #[test]
    fn session_metadata_strict_local_identity() {
        let id = "1".repeat(64);
        let mut s = Session {
            protocol: PROTOCOL.into(),
            session_id: id.clone(),
            token: "2".repeat(64),
            pid: 1,
            endpoint: format!("\\\\.\\pipe\\rust-ui-devctl-{id}"),
            window: Value::Null,
        };
        valid_session(&s).unwrap();
        s.endpoint = format!("\\\\remote\\pipe\\rust-ui-devctl-{id}");
        assert!(valid_session(&s).is_err());
        s.endpoint = format!("\\\\.\\pipe\\rust-ui-devctl-{id}");
        s.token = "wrong".into();
        assert!(valid_session(&s).is_err());
    }
    #[test]
    fn launch_quoting_is_full_argument_not_shell() {
        assert_eq!(quote_arg("a b"), "\"a b\"");
        assert_eq!(quote_arg("a\\\"b"), "\"a\\\\\\\"b\"");
        assert_eq!(quote_arg("W:\\folder\\"), "\"W:\\folder\\\\\"");
    }
    #[test]
    fn native_scale_uses_platform_authority() {
        for (dpi, ratio) in [(96, 1.), (120, 1.25), (144, 1.5), (192, 2.)] {
            let s = crate::platform::win32::space::scale_from_dpi(dpi);
            assert_eq!(s.0, ratio);
            assert_eq!(s.to_physical(32.), (32. * ratio) as i32);
        }
    }
    #[test]
    fn native_frame_size_is_rejected_before_payload_allocation() {
        let id = nonce().unwrap();
        let token = nonce().unwrap();
        let s = Session {
            protocol: PROTOCOL.into(),
            session_id: id.clone(),
            token,
            pid: std::process::id(),
            endpoint: format!("\\\\.\\pipe\\rust-ui-devctl-{id}"),
            window: Value::Null,
        };
        let initial = pipe(&s.endpoint).unwrap();
        let stop = Arc::new(event().unwrap());
        let halted = stop.clone();
        let thread = std::thread::spawn(move || {
            let listener = initial;
            connect(listener.0, halted.0).unwrap();
            let error = read::<Request>(listener.0, halted.0).unwrap_err();
            assert_eq!(error.code, "MESSAGE_TOO_LARGE");
            finish(listener.0, halted.0, &Response::from_result(0, Err(error))).unwrap();
        });
        let name = wide(&s.endpoint);
        let h = Handle(
            unsafe {
                CreateFileW(
                    PCWSTR(name.as_ptr()),
                    GENERIC_READ.0 | GENERIC_WRITE.0,
                    FILE_SHARE_MODE(0),
                    None,
                    OPEN_EXISTING,
                    FILE_FLAG_OVERLAPPED,
                    None,
                )
            }
            .unwrap(),
        );
        transfer(
            h.0,
            stop.0,
            &mut ((MAX_MESSAGE + 1) as u32).to_le_bytes(),
            true,
            Instant::now() + Duration::from_secs(5),
        )
        .unwrap();
        let response: Response = read(h.0, stop.0).unwrap();
        assert!(!response.ok);
        assert_eq!(response.error.unwrap().code, "MESSAGE_TOO_LARGE");
        drop(h);
        thread.join().unwrap();
    }
