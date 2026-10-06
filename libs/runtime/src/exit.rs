use std::process::{ExitCode, Termination};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Exit {
    /// 0: Clean exit, including graceful shutdown on Ctrl+C/SIGTERM
    Success,
    /// 1: Runtime failure: bind, serve, or app start
    Failure,
    /// 78: Invalid configuration (`EX_CONFIG` from sysexits.h)
    Config,
}

impl Exit {
    pub const fn code(self) -> u8 {
        match self {
            Self::Success => 0,
            Self::Failure => 1,
            Self::Config => 78,
        }
    }
}

impl From<Exit> for ExitCode {
    fn from(exit: Exit) -> Self {
        ExitCode::from(exit.code())
    }
}

impl Termination for Exit {
    fn report(self) -> ExitCode {
        self.into()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn codes_are_stable() {
        assert_eq!(Exit::Success.code(), 0);
        assert_eq!(Exit::Failure.code(), 1);
        assert_eq!(Exit::Config.code(), 78);
    }
}
