//! The one integration-test binary of weft-testkit.

mod trivial {
    #[test]
    fn repo_root_holds_the_pinned_toolchain() {
        assert!(weft_testkit::repo_root()
            .join("rust-toolchain.toml")
            .is_file());
    }
}

/// The form of the generated crates (`generated/`): a tree with a `[workspace]` of its own,
/// excluded from the root workspace, used by a weft crate through a path dependency.
mod probe {
    use weft_probe_api::{echo, ProbeId};

    #[test]
    fn a_crate_of_a_tree_with_its_own_workspace_builds_as_a_path_dependency() {
        let manifest = weft_testkit::repo_root().join("tests/fixtures/generated-probe/Cargo.toml");
        let text = std::fs::read_to_string(&manifest).expect("the probe manifest reads");
        assert!(
            text.lines().any(|line| line.trim() == "[workspace]"),
            "{} declares no [workspace]",
            manifest.display()
        );
        assert_eq!(echo(ProbeId(7)), ProbeId(7));
    }
}
