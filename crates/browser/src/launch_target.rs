//! Where a browser session is launched.

/// Sandbox was requested and a container runtime can start it.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum LaunchTarget {
    Sandbox,
    Host,
}

/// A missing container runtime must not fail the turn. Host Chromium is already
/// in the image; use it instead of Browserless.
#[must_use]
pub fn launch_target(sandbox_requested: bool, container_available: bool) -> LaunchTarget {
    if sandbox_requested && container_available {
        LaunchTarget::Sandbox
    } else {
        LaunchTarget::Host
    }
}

#[cfg(test)]
mod tests {
    use super::{LaunchTarget, launch_target};

    #[test]
    fn sandbox_without_a_container_runtime_uses_the_host_browser() {
        assert_eq!(launch_target(true, false), LaunchTarget::Host);
    }

    #[test]
    fn sandbox_with_a_container_runtime_stays_sandboxed() {
        assert_eq!(launch_target(true, true), LaunchTarget::Sandbox);
    }

    #[test]
    fn an_unsandboxed_session_uses_the_host_browser() {
        assert_eq!(launch_target(false, true), LaunchTarget::Host);
        assert_eq!(launch_target(false, false), LaunchTarget::Host);
    }
}
