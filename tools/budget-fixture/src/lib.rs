//! Fixture of `tools/test-gate-budget.sh`: one test that passes at once and one that sleeps past
//! the lowered limits the self-test sets. Not part of the root workspace.

#[cfg(test)]
mod tests {
    #[test]
    fn passes_at_once() {}

    #[test]
    fn sleeps_ten_seconds() {
        std::thread::sleep(std::time::Duration::from_secs(10));
    }
}
