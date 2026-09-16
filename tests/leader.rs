use std::io::{BufRead, BufReader};
use std::process::{Child, Command, Stdio};
use std::time::Duration;

use cosmic_ext_applet_pause::pause::Event;
use cosmic_ext_applet_pause::pause::leader;
use tokio::sync::mpsc;

const NAME: &str = "io.github.marcelogomes90.cosmic-ext-applet-pause.test";

struct PrivateBus {
    child: Child,
    address: String,
}

impl PrivateBus {
    fn start() -> std::io::Result<Self> {
        let mut child = Command::new("dbus-daemon")
            .args(["--session", "--nofork", "--print-address"])
            .stdout(Stdio::piped())
            .stderr(Stdio::null())
            .spawn()?;

        let stdout = child.stdout.take().expect("stdout was piped");
        let mut address = String::new();
        BufReader::new(stdout).read_line(&mut address)?;
        let address = address.trim().to_owned();

        if address.is_empty() {
            let _ = child.kill();
            return Err(std::io::Error::other("dbus-daemon printed no address"));
        }

        Ok(Self { child, address })
    }

    async fn connect(&self) -> zbus::Result<zbus::Connection> {
        zbus::connection::Builder::address(self.address.as_str())?
            .build()
            .await
    }
}

impl Drop for PrivateBus {
    fn drop(&mut self) {
        let _ = self.child.kill();
        let _ = self.child.wait();
    }
}

async fn next_role(events: &mut mpsc::Receiver<Event>) -> bool {
    loop {
        let event = tokio::time::timeout(Duration::from_secs(10), events.recv())
            .await
            .expect("an instance never decided whether it leads")
            .expect("the arbitration task hung up");

        if let Event::Leader(leading) = event {
            return leading;
        }
    }
}

#[tokio::test]
async fn exactly_one_of_two_instances_sends_the_reminders() {
    let Ok(bus) = PrivateBus::start() else {
        eprintln!("skipping: dbus-daemon is not available");
        return;
    };

    let (first_tx, mut first_rx) = mpsc::channel(8);
    let first = bus.connect().await.expect("the first instance connects");
    tokio::spawn(leader::arbitrate(first, NAME.to_owned(), first_tx));
    assert!(next_role(&mut first_rx).await, "the first one in leads");

    let (second_tx, mut second_rx) = mpsc::channel(8);
    let second = bus.connect().await.expect("the second instance connects");
    tokio::spawn(leader::arbitrate(second, NAME.to_owned(), second_tx));

    assert!(
        !next_role(&mut second_rx).await,
        "the second one waits its turn instead of speaking over the first"
    );
}

#[tokio::test]
async fn the_one_waiting_takes_over_when_the_other_goes_away() {
    let Ok(bus) = PrivateBus::start() else {
        eprintln!("skipping: dbus-daemon is not available");
        return;
    };

    let (first_tx, mut first_rx) = mpsc::channel(8);
    let first = bus.connect().await.expect("the first instance connects");
    let held = tokio::spawn(leader::arbitrate(first.clone(), NAME.to_owned(), first_tx));
    assert!(next_role(&mut first_rx).await);

    let (second_tx, mut second_rx) = mpsc::channel(8);
    let second = bus.connect().await.expect("the second instance connects");
    tokio::spawn(leader::arbitrate(second, NAME.to_owned(), second_tx));
    assert!(!next_role(&mut second_rx).await);

    held.abort();
    drop(first);

    assert!(
        next_role(&mut second_rx).await,
        "nobody was left to send the reminders"
    );
}
