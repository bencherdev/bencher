use std::process::Command;

use uuid::Uuid;

impl crate::Fingerprint {
    pub fn new() -> Option<Self> {
        Command::new("ioreg")
            .arg("-d2")
            .arg("-c")
            .arg("IOPlatformExpertDevice")
            .output()
            .ok()
            .and_then(|out| String::from_utf8(out.stdout).ok())
            .and_then(|output| {
                for line in output.lines() {
                    if let Some((_, uuid)) = line.split_once(r#""IOPlatformUUID" = ""#) {
                        if let Some((uuid, _)) = uuid.split_once('"') {
                            return Uuid::parse_str(uuid).ok();
                        }
                    }
                }
                None
            })
            .map(Self)
    }
}
