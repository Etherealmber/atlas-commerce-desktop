//! A deliberately small, non-identifying startup summary.

/// Keep only numeric OS version components. Custom distribution strings can
/// contain arbitrary local text, so they must not be copied into the log.
fn safe_version(version: &os_info::Version) -> String {
    let text = version.to_string();
    if !text.is_empty()
        && text.len() <= 64
        && text
            .bytes()
            .all(|byte| byte.is_ascii_digit() || byte == b'.')
        && text.bytes().any(|byte| byte.is_ascii_digit())
    {
        text
    } else {
        "unknown".to_owned()
    }
}

pub(crate) fn log_environment() {
    let info = os_info::get();
    // std::env::consts describes the binary, not necessarily the host CPU.
    // Do not infer laptop/desktop from battery presence or expose model IDs.
    log::info!(
        "Diagnostics: app_version={} device_class=computer (desktop client; form factor unknown) os={} os_version_or_build={} binary_arch={}",
        env!("CARGO_PKG_VERSION"),
        std::env::consts::OS,
        safe_version(info.version()),
        std::env::consts::ARCH,
    );
}

#[cfg(test)]
mod tests {
    use super::safe_version;
    use os_info::Version;

    #[test]
    fn os_version_is_numeric_or_unknown() {
        assert_eq!(safe_version(&Version::Semantic(10, 0, 22631)), "10.0.22631");
        assert_eq!(safe_version(&Version::Custom("24.04".into())), "24.04");
        assert_eq!(safe_version(&Version::Unknown), "unknown");
        assert_eq!(
            safe_version(&Version::Custom("user-pc\nsecret".into())),
            "unknown"
        );
        assert_eq!(safe_version(&Version::Custom("1".repeat(65))), "unknown");
    }
}
