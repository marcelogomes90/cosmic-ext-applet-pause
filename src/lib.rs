pub mod applet;
pub mod config;
pub mod format;
pub mod i18n;
pub mod links;
pub mod pause;
pub mod phrases;

pub const APP_ID: &str = "io.github.marcelogomes90.cosmic-ext-applet-pause";

pub fn init_tracing() {
    use tracing_subscriber::EnvFilter;

    let filter = EnvFilter::try_from_default_env()
        .unwrap_or_else(|_| EnvFilter::new("warn,cosmic_ext_applet_pause=info"));

    let _ = tracing_subscriber::fmt()
        .with_env_filter(filter)
        .with_writer(std::io::stderr)
        .try_init();
}
