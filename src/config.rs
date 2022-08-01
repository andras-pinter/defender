#[derive(Clone)]
pub enum Timeout {
    Instant,
    Duration(std::time::Duration),
    Infinite,
}

#[derive(Clone)]
pub struct GuardConfig {
    pub timeout: Timeout,
}

impl Default for GuardConfig {
    fn default() -> Self {
        GuardConfig {
            timeout: Timeout::Infinite,
        }
    }
}
