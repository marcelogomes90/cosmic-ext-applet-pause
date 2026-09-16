use std::future::Future;
use std::time::Duration;

pub const CONNECT_TIMEOUT: Duration = Duration::from_secs(5);
pub const NOTIFY_TIMEOUT: Duration = Duration::from_secs(3);

#[derive(Debug)]
pub enum CallError {
    Timeout {
        label: &'static str,
        after: Duration,
    },
    Dbus {
        label: &'static str,
        source: Box<zbus::Error>,
    },
}

impl CallError {
    pub fn label(&self) -> &'static str {
        match self {
            Self::Timeout { label, .. } | Self::Dbus { label, .. } => label,
        }
    }
}

impl std::fmt::Display for CallError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Timeout { label, after } => {
                write!(f, "{label} timed out after {}ms", after.as_millis())
            }
            Self::Dbus { label, source } => write!(f, "{label} failed: {source}"),
        }
    }
}

impl std::error::Error for CallError {}

pub async fn with_timeout<T, E: Into<zbus::Error>>(
    timeout: Duration,
    label: &'static str,
    fut: impl Future<Output = Result<T, E>>,
) -> Result<T, CallError> {
    match tokio::time::timeout(timeout, fut).await {
        Ok(Ok(value)) => Ok(value),
        Ok(Err(source)) => Err(CallError::Dbus {
            label,
            source: Box::new(source.into()),
        }),
        Err(_) => Err(CallError::Timeout {
            label,
            after: timeout,
        }),
    }
}
