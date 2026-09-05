use std::io;

const LAUNCH_AGENT_LABEL: &str = "com.djozman.BlurAutoClicker";

fn launch_agent_path() -> Option<std::path::PathBuf> {
    dirs::home_dir().map(|home| {
        home.join("Library/LaunchAgents")
            .join(format!("{LAUNCH_AGENT_LABEL}.plist"))
    })
}

pub fn get_autostart_enabled() -> bool {
    launch_agent_path().map(|p| p.exists()).unwrap_or(false)
}

pub fn set_autostart_enabled(enabled: bool) -> io::Result<()> {
    let path = launch_agent_path().ok_or_else(|| {
        io::Error::new(
            io::ErrorKind::NotFound,
            "Could not locate ~/Library/LaunchAgents",
        )
    })?;

    if enabled {
        let exe = std::env::current_exe()?;
        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent)?;
        }
        let plist = format!(
            r#"<?xml version="1.0" encoding="UTF-8"?>
<!DOCTYPE plist PUBLIC "-//Apple//DTD PLIST 1.0//EN" "http://www.apple.com/DTDs/PropertyList-1.0.dtd">
<plist version="1.0">
<dict>
    <key>Label</key>
    <string>{label}</string>
    <key>ProgramArguments</key>
    <array>
        <string>{exe}</string>
        <string>--autostart</string>
    </array>
    <key>RunAtLoad</key>
    <true/>
</dict>
</plist>"#,
            label = LAUNCH_AGENT_LABEL,
            exe = exe.display(),
        );
        std::fs::write(&path, plist)?;
    } else {
        let _ = std::fs::remove_file(&path);
    }

    Ok(())
}
