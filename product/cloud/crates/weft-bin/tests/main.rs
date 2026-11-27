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
