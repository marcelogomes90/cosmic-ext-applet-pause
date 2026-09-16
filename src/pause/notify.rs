use std::sync::{Arc, Mutex};

use crate::pause::model::ReminderKind;

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct Request {
    pub kind: ReminderKind,
    pub summary: String,
    pub body: String,
    pub action_label: String,
    pub icon: String,
    pub timeout_ms: i32,
}

pub trait Notifier: Send + 'static {
    fn deliver(&mut self, request: Request);
}

#[derive(Debug, Default)]
pub struct DroppingNotifier;

impl Notifier for DroppingNotifier {
    fn deliver(&mut self, request: Request) {
        tracing::debug!(kind = ?request.kind, "a reminder came due with nowhere to send it");
    }
}

#[derive(Clone, Debug, Default)]
pub struct RecordingNotifier {
    delivered: Arc<Mutex<Vec<Request>>>,
}

impl RecordingNotifier {
    pub fn delivered(&self) -> Vec<Request> {
        self.delivered
            .lock()
            .expect("the recorder is not poisoned")
            .clone()
    }

    pub fn kinds(&self) -> Vec<ReminderKind> {
        self.delivered()
            .into_iter()
            .map(|request| request.kind)
            .collect()
    }

    pub fn take(&self) -> Vec<Request> {
        std::mem::take(&mut *self.delivered.lock().expect("the recorder is not poisoned"))
    }
}

impl Notifier for RecordingNotifier {
    fn deliver(&mut self, request: Request) {
        tracing::info!(kind = ?request.kind, summary = %request.summary, "reminder");
        self.delivered
            .lock()
            .expect("the recorder is not poisoned")
            .push(request);
    }
}

pub const ACTION_KEY: &str = "default";

#[derive(Debug)]
pub struct FreedesktopNotifier {
    requests: tokio::sync::mpsc::Sender<Request>,
}

impl FreedesktopNotifier {
    pub fn spawn(
        runtime: &tokio::runtime::Handle,
        app_name: String,
        events: tokio::sync::mpsc::Sender<crate::pause::Event>,
    ) -> Self {
        let (requests, inbox) = tokio::sync::mpsc::channel(16);
        runtime.spawn(async move { desktop::run(app_name, inbox, events).await });

        Self { requests }
    }
}

impl Notifier for FreedesktopNotifier {
    fn deliver(&mut self, request: Request) {
        if let Err(error) = self.requests.try_send(request) {
            tracing::warn!(error = %error, "dropping a reminder, the notifier is not keeping up");
        }
    }
}

mod desktop {
    use std::collections::HashMap;

    use futures::StreamExt as _;
    use tokio::sync::mpsc;
    use zbus::zvariant::Value;

    use crate::pause::Event;
    use crate::pause::call::{CONNECT_TIMEOUT, NOTIFY_TIMEOUT, with_timeout};
    use crate::pause::model::{NotifierState, ReminderKind};
    use crate::pause::proxies::NotificationsProxy;

    use super::{ACTION_KEY, Request};

    const LIVE_LIMIT: usize = 16;

    pub async fn run(
        app_name: String,
        mut inbox: mpsc::Receiver<Request>,
        events: mpsc::Sender<Event>,
    ) {
        let Some(proxy) = connect(&events).await else {
            while inbox.recv().await.is_some() {
                tracing::debug!("a reminder came due but the desktop has no notification service");
            }
            return;
        };

        let mut invoked = match proxy.receive_action_invoked().await {
            Ok(stream) => stream,
            Err(error) => {
                tracing::warn!(%error, "cannot hear back from the notification service");
                return;
            }
        };

        let mut closed = match proxy.receive_notification_closed().await {
            Ok(stream) => stream,
            Err(error) => {
                tracing::warn!(%error, "cannot hear about dismissed notifications");
                return;
            }
        };

        let mut live: HashMap<u32, ReminderKind> = HashMap::new();

        loop {
            tokio::select! {
                biased;

                Some(signal) = invoked.next() => {
                    if let Ok(args) = signal.args()
                        && let Some(kind) = live.remove(&args.id)
                    {
                        tracing::info!(?kind, action = args.action_key, "the reminder was answered");
                        let _ = events.try_send(Event::Activated(kind));
                    }
                }

                Some(signal) = closed.next() => {
                    if let Ok(args) = signal.args() {
                        live.remove(&args.id);
                    }
                }

                request = inbox.recv() => {
                    let Some(request) = request else { break };

                    match send(&proxy, &app_name, &request).await {
                        Ok(id) => {
                            if live.len() >= LIVE_LIMIT {
                                live.clear();
                            }
                            live.insert(id, request.kind);
                        }
                        Err(error) => {
                            tracing::warn!(%error, kind = ?request.kind, "the reminder did not reach the desktop");
                            let _ = events.try_send(Event::Notifier(NotifierState::Unavailable));
                        }
                    }
                }
            }
        }
    }

    async fn connect(events: &mpsc::Sender<Event>) -> Option<NotificationsProxy<'static>> {
        let connection =
            match with_timeout(CONNECT_TIMEOUT, "session bus", zbus::Connection::session()).await {
                Ok(connection) => connection,
                Err(error) => {
                    tracing::error!(%error, "cannot reach the session bus");
                    let _ = events.try_send(Event::Notifier(NotifierState::Unavailable));
                    return None;
                }
            };

        let proxy = match NotificationsProxy::new(&connection).await {
            Ok(proxy) => proxy,
            Err(error) => {
                tracing::error!(%error, "cannot reach the notification service");
                let _ = events.try_send(Event::Notifier(NotifierState::Unavailable));
                return None;
            }
        };

        match with_timeout(NOTIFY_TIMEOUT, "GetCapabilities", proxy.get_capabilities()).await {
            Ok(capabilities) => {
                tracing::info!(?capabilities, "the notification service is ready");
                let _ = events.try_send(Event::Notifier(NotifierState::Ready));
            }
            Err(error) => {
                tracing::warn!(%error, "the notification service did not introduce itself");
                let _ = events.try_send(Event::Notifier(NotifierState::Unavailable));
            }
        }

        Some(proxy)
    }

    async fn send(
        proxy: &NotificationsProxy<'_>,
        app_name: &str,
        request: &Request,
    ) -> Result<u32, crate::pause::call::CallError> {
        let actions = [ACTION_KEY, request.action_label.as_str()];
        let hints = HashMap::from([
            ("urgency", Value::U8(1)),
            ("desktop-entry", Value::from(crate::APP_ID)),
        ]);

        with_timeout(
            NOTIFY_TIMEOUT,
            "Notify",
            proxy.notify(
                app_name,
                0,
                &request.icon,
                &request.summary,
                &request.body,
                &actions,
                hints,
                request.timeout_ms,
            ),
        )
        .await
    }
}
