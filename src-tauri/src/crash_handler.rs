// Held in a static so the client is never dropped during abnormal termination
// (a drop during a crash can hang). The static is deliberately not dropped at
// process exit; graceful shutdown goes through `shutdown_crashpad()`.
pub fn initialize_crashpad() -> Result<(), Box<dyn std::error::Error>> {
    log::warn!(
        "[Crashpad] Not available — compile with 'crashpad' feature for out-of-process crash dumps."
    );
    Ok(())
}

pub fn shutdown_crashpad() {}

#[cfg(test)]
mod tests {

    #[test]
    fn crashpad_stub_returns_ok() {
        let result = super::initialize_crashpad();
        assert!(result.is_ok());
    }
}
