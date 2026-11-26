use colab_server::ipc::HostPath;
use colab_server::tailscale::{peer, ping_status};
use serde_json::Value;

fn fixture(name: &str) -> Value {
    serde_json::from_str(&std::fs::read_to_string(format!("{}/tests/fixtures/tailscale/{name}.json", env!("CARGO_MANIFEST_DIR"))).unwrap()).unwrap()
}

#[test]
fn saved_hosts_match_peers_by_name_or_address_and_pings_tell_direct_from_relay() {
    let status = fixture("status");
    let ip = |address: &str| peer(&status, address).map(|p| p["TailscaleIPs"][0].as_str().unwrap().to_string());
    for address in ["anton", "Anton", "anton.example.ts.net", "anton.example.ts.net.", "anton:80", "100.68.126.75", "100.68.126.75:80", "fd7a:115c:a1e0::43a:7e4b"] {
        assert_eq!(ip(address).as_deref(), Some("100.68.126.75"), "{address}");
    }
    assert_eq!(peer(&status, "iphone").unwrap()["Online"], false);
    assert_eq!(ip("anton-rex").as_deref(), Some("100.90.25.52"));
    assert_eq!(ip("127.0.0.1:7762"), None);
    assert_eq!(ip("ant"), None);
    assert_eq!(ping_status(&fixture("ping-direct")), Some((HostPath::Direct, Some(7))));
    assert_eq!(ping_status(&fixture("ping-relay")), Some((HostPath::Relay, Some(99))));
    assert_eq!(ping_status(&fixture("ping-no-peer")), None);
}
