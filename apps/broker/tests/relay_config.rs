//! Exercise the actual entrypoints without installing rules or starting services.
use std::fs;
use std::os::unix::fs::PermissionsExt;
use std::path::PathBuf;
use std::process::Command;

struct Fixture(PathBuf);

impl Drop for Fixture {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.0);
    }
}

fn run(binary: &str, config: &std::path::Path, command: &str) -> std::process::Output {
    Command::new(binary).arg("--config").arg(config).arg(command).output().unwrap()
}

#[test]
fn relay_never_needs_broker_private_key_but_broker_still_validates_it() {
    let fixture = Fixture(std::env::temp_dir().join(format!(
        "plank-relay-config-{}-{}",
        std::process::id(),
        std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).unwrap().as_nanos()
    )));
    fs::create_dir(&fixture.0).unwrap();
    let key = fixture.0.join("broker-only.pk8");
    fs::write(&key, b"not accessible to the relay").unwrap();
    fs::set_permissions(&key, fs::Permissions::from_mode(0o000)).unwrap();
    let config = fixture.0.join("broker.toml");
    let policy = format!(
        "{}\n[mac_admission]\nsigning_key_file = {:?}\nttl_secs = 60\n[mac_admission.hosts]\n\"mac.example.org\" = \"studio\"\n",
        include_str!("../packaging/broker.toml.example"), key
    );
    fs::write(&config, &policy).unwrap();
    let relay = env!("CARGO_BIN_EXE_plank-broker-relay");
    let broker = env!("CARGO_BIN_EXE_plank-broker");

    // Mode000 models a broker-only key under the relay UID. Absence also
    // proves that no key read or metadata lookup is necessary, even as root.
    for absent in [false, true] {
        if absent {
            fs::remove_file(&key).unwrap();
        }
        let output = run(relay, &config, "--print-ruleset");
        assert!(output.status.success(), "{}", String::from_utf8_lossy(&output.stderr));
        assert!(String::from_utf8_lossy(&output.stdout).contains("table inet"));
        let output = run(broker, &config, "check-config");
        assert!(!output.status.success());
        assert!(String::from_utf8_lossy(&output.stderr).contains("signing key"));
        assert!(plank_broker::config::Config::load(&config).is_err());
    }

    // The relay skips private material only, not validation of the shared policy.
    for invalid in [
        policy.replace("ttl_secs = 60", "ttl_secs = 61"),
        policy.replace("mac.example.org", "MAC.example.org"),
        policy.replace("\"studio\"", "\"root\""),
        policy.replace("port_min = 29001", "port_min = 0"),
    ] {
        assert_ne!(invalid, policy);
        fs::write(&config, invalid).unwrap();
        assert!(!run(relay, &config, "--print-ruleset").status.success());
    }
}
