#[derive(Debug, thiserror::Error)]
#[cfg_attr(test, derive(PartialEq))]
pub enum GuardError {
    #[error("Timeout error")]
    Timeout,
    #[error("Killed")]
    Killed,
    #[error("Unable to kill an already elapsed Guard")]
    UnableToKilled,
}
