//! The one integration-test binary of weft-bin. Its presence also makes a workspace test build
//! build the `weft` binary itself, which `weft-testkit`'s `ControlHost` starts.

mod arguments {
    use std::process::Command;

    const WEFT: &str = env!("CARGO_BIN_EXE_weft");

    #[test]
    fn the_binary_refuses_to_start_without_a_state_file() {
        let output = Command::new(WEFT)
            .args(["--port", "0"])
            .output()
            .expect("the binary runs");
        assert_eq!(output.status.code(), Some(2), "{output:?}");
        let stderr = String::from_utf8_lossy(&output.stderr);
        assert!(stderr.contains("--state"), "{stderr}");
    }
}

/// Where the caller header is trusted, the host listens on loopback only.
mod listen {
    use weft_bin::listen_addresses;

    #[test]
    fn a_trusting_build_refuses_an_address_that_is_not_loopback() {
        let refused = listen_addresses("0.0.0.0", 0, true).expect_err("0.0.0.0 is refused");
        assert!(refused.contains("loopback only"), "{refused}");
        assert!(listen_addresses("127.0.0.1", 0, true).is_ok());
    }

    #[test]
    fn a_build_that_trusts_no_header_listens_where_it_is_told() {
        assert!(listen_addresses("0.0.0.0", 0, false).is_ok());
    }
}
