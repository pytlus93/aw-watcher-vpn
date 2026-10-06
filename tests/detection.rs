use aw_watcher_vpn::*;
fn snapshot() -> Snapshot {
    Snapshot {
        schema_version: 1,
        observed_at: "2026-10-05T12:00:00Z".into(),
        adapters: vec![
            adapter("wifi", 1, "192.0.2.5"),
            adapter("tunnel", 2, "198.51.100.5"),
        ],
        routes: vec![route(1, "0.0.0.0/0", 5), route(2, "203.0.113.0/24", 10)],
    }
}
fn adapter(id: &str, index: u32, ip: &str) -> Adapter {
    Adapter {
        network_type: "Ethernet".into(),
        id: id.into(),
        index,
        name: id.into(),
        description: "Synthetic adapter".into(),
        up: true,
        addresses: vec![ip.into()],
        dns_suffix: "synthetic.example".into(),
        dns_servers: vec!["192.0.2.53".into()],
        metric_v4: 10,
        metric_v6: 10,
    }
}
fn route(index: u32, prefix: &str, metric: u32) -> Route {
    Route {
        interface_index: index,
        prefix: prefix.into(),
        next_hop: "0.0.0.0".into(),
        metric,
    }
}
fn config() -> Config {
    Config {
        schema_version: 1,
        poll_seconds: 5,
        autolearn: true,
        notify: true,
        ignored_connections: vec![],
        profiles: vec![Profile {
            id: "customer-a".into(),
            customer_hint: "Synthetic A".into(),
            rules: Rules {
                adapter_ids: vec!["tunnel".into()],
                route_prefixes: vec!["203.0.113.0/24".into()],
                ..Default::default()
            },
            destinations: vec!["203.0.113.7".into()],
        }],
    }
}
#[test]
fn split_tunnel_without_default_gateway() {
    let o = evaluate(&config(), Some(&snapshot()));
    assert_eq!(o[0].state, State::Present);
    assert_eq!(o[0].destinations_routed, 1);
}
#[test]
fn records_known_and_unknown_connections_without_physical_adapters() {
    let mut s = snapshot();
    let mut unknown = adapter("unknown", 3, "198.51.100.9");
    unknown.network_type = "Ppp".into();
    s.adapters.push(unknown);
    s.routes.push(route(3, "192.0.2.0/24", 10));
    let connections = active_connections(&config(), &s);
    assert_eq!(connections.len(), 2);
    assert_eq!(connections[0].customers, vec!["Synthetic A"]);
    assert_eq!(connections[1].name, "unknown");
    assert!(connections[1].customers.is_empty());
    s.adapters[1].up = false;
    s.adapters[2].up = false;
    assert!(active_connections(&config(), &s).is_empty());
}
#[test]
fn unlearned_virtual_ethernet_tunnel_requires_usable_address_and_route() {
    let mut s = snapshot();
    let mut c = config();
    c.profiles.clear();
    s.adapters[1].description = "WireGuard Tunnel".into();
    assert_eq!(active_connections(&c, &s).len(), 1);
    s.routes.retain(|r| r.interface_index != 2);
    assert!(active_connections(&c, &s).is_empty());
    s.routes.push(route(2, "203.0.113.0/24", 10));
    s.adapters[1].addresses = vec!["169.254.1.2".into()];
    assert!(active_connections(&c, &s).is_empty());
}
#[test]
fn connection_name_survives_recreated_adapter_id() {
    let mut s = snapshot();
    let mut c = config();
    c.profiles[0].rules.adapter_ids.clear();
    c.profiles[0].rules.adapter_names = vec!["tunnel".into()];
    c.validate().unwrap();
    s.adapters[1].id = "recreated-guid".into();
    assert_eq!(active_connections(&c, &s)[0].customers, vec!["Synthetic A"]);
    s.adapters[1].name = "different VPN".into();
    s.adapters[1].network_type = "Ppp".into();
    assert!(active_connections(&c, &s)[0].customers.is_empty());
}
#[test]
fn ignored_company_tunnel_does_not_hide_customer_connections() {
    let mut s = snapshot();
    let mut company = adapter("company", 3, "198.51.100.8");
    company.name = "Polcom".into();
    company.network_type = "Ppp".into();
    s.adapters.push(company);
    s.routes.push(route(3, "10.0.0.0/8", 10));
    let mut c = config();
    c.ignored_connections = vec![" polcom ".into()];
    c.validate().unwrap();
    let connections = active_connections(&c, &s);
    assert_eq!(connections.len(), 1);
    assert_eq!(connections[0].customers, vec!["Synthetic A"]);
    s.adapters[1].up = false;
    assert!(active_connections(&c, &s).is_empty());
    c.ignored_connections = vec![" ".into()];
    assert!(c.validate().is_err());
}
#[test]
fn learning_avoids_host_routes_and_large_route_fingerprints() {
    let mut before = snapshot();
    before.adapters[1].up = false;
    before.routes.retain(|r| r.interface_index != 2);
    let mut after = snapshot();
    for n in 1..100 {
        after.routes.push(route(2, &format!("10.0.0.{n}/32"), 10));
    }
    for n in 1..10 {
        after.routes.push(route(2, &format!("10.{n}.0.0/16"), 10));
    }
    let learned = learn(&before, &after);
    assert_eq!(learned.profiles[0].rules.route_prefixes.len(), 3);
    assert!(learned.profiles[0]
        .rules
        .route_prefixes
        .iter()
        .all(|p| !p.ends_with("/32")));
}
#[test]
fn dormant_installed_adapter_is_not_present() {
    let mut s = snapshot();
    s.adapters[1].up = false;
    assert_eq!(evaluate(&config(), Some(&s))[0].state, State::Absent);
}
#[test]
fn link_local_alone_is_not_usable() {
    let mut s = snapshot();
    s.adapters[1].addresses = vec!["169.254.1.2".into(), "fe80::1".into()];
    assert_eq!(evaluate(&config(), Some(&s))[0].state, State::Absent);
}
#[test]
fn collection_failure_is_unknown() {
    assert_eq!(evaluate(&config(), None)[0].state, State::Unknown);
}
#[test]
fn simultaneous_contexts_are_preserved() {
    let mut c = config();
    let mut p = c.profiles[0].clone();
    p.id = "customer-b".into();
    c.profiles.push(p);
    assert_eq!(evaluate(&c, Some(&snapshot())).len(), 2);
}
#[test]
fn route_on_wrong_adapter_does_not_match() {
    let mut s = snapshot();
    s.routes[1].interface_index = 1;
    assert_eq!(evaluate(&config(), Some(&s))[0].state, State::Absent);
}
#[test]
fn longest_prefix_and_total_metric_win() {
    let mut s = snapshot();
    s.routes.push(route(1, "203.0.113.7/32", 100));
    assert_eq!(evaluate(&config(), Some(&s))[0].destinations_routed, 0);
    s.routes.pop();
    s.routes.push(route(1, "203.0.113.0/24", 9));
    assert_eq!(evaluate(&config(), Some(&s))[0].destinations_routed, 0);
}
#[test]
fn equal_cost_is_ambiguous() {
    let mut s = snapshot();
    s.routes.push(route(1, "203.0.113.0/24", 10));
    assert_eq!(evaluate(&config(), Some(&s))[0].destinations_routed, 0);
}
#[test]
fn ipv6_supported() {
    let mut s = snapshot();
    s.adapters[1].addresses = vec!["2001:db8:1::2".into()];
    s.routes.push(route(2, "2001:db8:2::/64", 1));
    assert_eq!(
        best_interfaces(&s, "2001:db8:2::9".parse().unwrap()),
        vec![2]
    );
}
#[test]
fn dns_rules_are_on_same_interface() {
    let mut c = config();
    c.profiles[0].rules.dns_suffixes = vec!["SYNTHETIC.EXAMPLE.".into()];
    c.profiles[0].rules.dns_servers = vec!["192.0.2.53".into()];
    assert_eq!(evaluate(&c, Some(&snapshot()))[0].state, State::Present);
    c.profiles[0].rules.dns_servers = vec!["192.0.2.54".into()];
    assert_eq!(evaluate(&c, Some(&snapshot()))[0].state, State::Absent);
}
#[test]
fn learning_produces_reviewable_draft() {
    let mut before = snapshot();
    before.adapters[1].up = false;
    before.routes.pop();
    let draft = learn(&before, &snapshot());
    assert_eq!(draft.profiles.len(), 1);
    assert_eq!(draft.profiles[0].rules.adapter_ids, vec!["tunnel"]);
    draft.validate().unwrap();
}
#[test]
fn rejects_unsafe_or_empty_rules() {
    let mut c = config();
    c.profiles[0].id = "../bad".into();
    assert!(c.validate().is_err());
    let mut c = config();
    c.profiles[0].rules = Rules::default();
    assert!(c.validate().is_err());
    let mut c = config();
    c.profiles[0].destinations = vec!["private.example".into()];
    assert!(c.validate().is_err());
}
