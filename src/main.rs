use cosmic_ext_applet_pause::config::{ConfigScheduleStore, SettingsStore};
use cosmic_ext_applet_pause::pause::notify::FreedesktopNotifier;
use cosmic_ext_applet_pause::pause::{Builder, channel, leader};
use cosmic_ext_applet_pause::{APP_ID, applet, fl, i18n, init_tracing, phrases};

fn main() -> cosmic::iced::Result {
    init_tracing();
    i18n::init();
    tracing::info!(version = env!("CARGO_PKG_VERSION"), "starting pause");

    let runtime = tokio::runtime::Builder::new_multi_thread()
        .worker_threads(2)
        .thread_name("pause-scheduler")
        .enable_all()
        .build()
        .expect("failed to start the scheduler runtime");

    let settings = SettingsStore::open(APP_ID).load();
    let (events, inbox) = channel();
    let notifier = FreedesktopNotifier::spawn(runtime.handle(), fl!("app-title"), events.sender());

    leader::spawn(runtime.handle(), APP_ID.to_owned(), events.sender());

    let (handle, _events, _join) =
        Builder::new(ConfigScheduleStore::open(APP_ID), phrases::Fluent, settings)
            .notifier(notifier)
            .inbox(inbox)
            .spawn(runtime.handle());

    let _runtime = Box::leak(Box::new(runtime));

    applet::run(handle)
}
