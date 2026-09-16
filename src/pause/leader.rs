use futures::StreamExt as _;
use tokio::sync::mpsc;
use zbus::fdo::{DBusProxy, RequestNameFlags, RequestNameReply};

use crate::pause::Event;
use crate::pause::call::{CONNECT_TIMEOUT, with_timeout};

pub fn spawn(runtime: &tokio::runtime::Handle, name: String, events: mpsc::Sender<Event>) {
    runtime.spawn(async move { run(name, events).await });
}

async fn run(name: String, events: mpsc::Sender<Event>) {
    let connection =
        match with_timeout(CONNECT_TIMEOUT, "session bus", zbus::Connection::session()).await {
            Ok(connection) => connection,
            Err(error) => {
                tracing::warn!(%error, "cannot arbitrate with other instances, acting alone");
                let _ = events.try_send(Event::Leader(true));
                return;
            }
        };

    arbitrate(connection, name, events).await;
}

pub async fn arbitrate(connection: zbus::Connection, name: String, events: mpsc::Sender<Event>) {
    let proxy = match DBusProxy::new(&connection).await {
        Ok(proxy) => proxy,
        Err(error) => {
            tracing::warn!(%error, "cannot reach the bus daemon, acting alone");
            let _ = events.try_send(Event::Leader(true));
            return;
        }
    };

    let mut acquired = match proxy
        .receive_name_acquired_with_args(&[(0, name.as_str())])
        .await
    {
        Ok(stream) => stream,
        Err(error) => {
            tracing::warn!(%error, "cannot watch for the name, acting alone");
            let _ = events.try_send(Event::Leader(true));
            return;
        }
    };

    let reply = connection
        .request_name_with_flags(name.as_str(), RequestNameFlags::AllowReplacement.into())
        .await;

    let leading = match reply {
        Ok(RequestNameReply::PrimaryOwner | RequestNameReply::AlreadyOwner) => true,
        Ok(RequestNameReply::InQueue) => {
            tracing::info!("another instance is sending the reminders, waiting in the queue");
            false
        }
        Ok(RequestNameReply::Exists) => {
            tracing::info!("another instance holds the name and is not sharing");
            false
        }
        Err(error) => {
            tracing::warn!(%error, "could not ask for the name, acting alone");
            true
        }
    };

    let _ = events.try_send(Event::Leader(leading));

    if leading {
        tracing::info!("this instance is sending the reminders");
    }

    while acquired.next().await.is_some() {
        tracing::info!("this instance has taken over sending the reminders");
        if events.try_send(Event::Leader(true)).is_err() {
            break;
        }
    }

    drop(connection);
}
