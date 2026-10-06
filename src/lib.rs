//! Host-independent profile matching. This library performs no IO.
use ipnet::IpNet;
use serde::{Deserialize, Serialize};
use std::{collections::HashSet, net::IpAddr};

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Snapshot {
    pub schema_version: u32,
    pub observed_at: String,
    pub adapters: Vec<Adapter>,
    pub routes: Vec<Route>,
}
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Adapter {
    #[serde(default)]
    pub network_type: String,
    pub id: String,
    pub index: u32,
    pub name: String,
    pub description: String,
    pub up: bool,
    pub addresses: Vec<String>,
    pub dns_suffix: String,
    pub dns_servers: Vec<String>,
    pub metric_v4: u32,
    pub metric_v6: u32,
}
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Route {
    pub interface_index: u32,
    pub prefix: String,
    pub next_hop: String,
    pub metric: u32,
}
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Config {
    pub schema_version: u32,
    #[serde(default = "default_interval")]
    pub poll_seconds: u64,
    #[serde(default = "enabled")]
    pub autolearn: bool,
    #[serde(default = "enabled")]
    pub notify: bool,
    #[serde(default)]
    pub ignored_connections: Vec<String>,
    #[serde(default)]
    pub profiles: Vec<Profile>,
}
fn default_interval() -> u64 {
    5
}
fn enabled() -> bool {
    true
}
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Profile {
    pub id: String,
    pub customer_hint: String,
    pub rules: Rules,
    #[serde(default)]
    pub destinations: Vec<String>,
}
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
#[serde(default, deny_unknown_fields)]
pub struct Rules {
    pub adapter_ids: Vec<String>,
    pub adapter_names: Vec<String>,
    pub description_contains: Vec<String>,
    pub dns_suffixes: Vec<String>,
    pub dns_servers: Vec<String>,
    pub route_prefixes: Vec<String>,
}
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "snake_case")]
pub enum State {
    Present,
    Absent,
    Unknown,
}
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct Observation {
    pub schema_version: u32,
    pub profile_id: String,
    pub customer_hint: String,
    pub state: State,
    // These contain local IDs only, never adapter names, DNS servers or endpoints.
    pub matching_adapters: Vec<String>,
    pub destinations_routed: usize,
    pub destinations_checked: usize,
    pub source: String,
}

impl Config {
    pub fn validate(&self) -> Result<(), String> {
        if self.schema_version != 1 {
            return Err("unsupported config schema".into());
        }
        if !(2..=60).contains(&self.poll_seconds) {
            return Err("poll_seconds must be 2..60".into());
        }
        if self
            .ignored_connections
            .iter()
            .any(|name| name.trim().is_empty())
        {
            return Err("ignored connection names cannot be empty".into());
        }
        let mut ids = HashSet::new();
        for p in &self.profiles {
            if p.id.is_empty()
                || p.id.len() > 64
                || !p
                    .id
                    .bytes()
                    .all(|b| b.is_ascii_alphanumeric() || b == b'-' || b == b'_')
            {
                return Err("profile IDs must be 1..64 ASCII letters, digits, '-' or '_'".into());
            }
            if !ids.insert(&p.id) {
                return Err(format!("duplicate profile ID: {}", p.id));
            }
            let r = &p.rules;
            if r.adapter_ids.is_empty()
                && r.adapter_names.is_empty()
                && r.description_contains.is_empty()
                && r.dns_suffixes.is_empty()
                && r.dns_servers.is_empty()
                && r.route_prefixes.is_empty()
            {
                return Err(format!("{} has no matching rules", p.id));
            }
            for s in r
                .adapter_ids
                .iter()
                .chain(&r.adapter_names)
                .chain(&r.description_contains)
                .chain(&r.dns_suffixes)
            {
                if s.trim().is_empty() {
                    return Err("empty rule strings are forbidden".into());
                }
            }
            for s in &r.route_prefixes {
                s.parse::<IpNet>()
                    .map_err(|_| format!("invalid route prefix: {s}"))?;
            }
            for s in r.dns_servers.iter().chain(&p.destinations) {
                s.parse::<IpAddr>()
                    .map_err(|_| format!("expected numeric IP address: {s}"))?;
            }
        }
        Ok(())
    }
}

fn usable(a: &Adapter) -> bool {
    a.up && a.addresses.iter().any(|s| {
        s.parse::<IpAddr>().is_ok_and(|ip| match ip {
            IpAddr::V4(v) => {
                !v.is_loopback() && !v.is_unspecified() && !v.is_link_local() && !v.is_multicast()
            }
            IpAddr::V6(v) => {
                !v.is_loopback()
                    && !v.is_unspecified()
                    && !v.is_unicast_link_local()
                    && !v.is_multicast()
            }
        })
    })
}
fn suffix(s: &str) -> String {
    s.trim_end_matches('.').to_ascii_lowercase()
}
fn matches(p: &Profile, a: &Adapter, s: &Snapshot) -> bool {
    let r = &p.rules;
    usable(a)
        && (r.adapter_names.is_empty()
            || r.adapter_names
                .iter()
                .any(|name| name.eq_ignore_ascii_case(&a.name)))
        && (r.adapter_ids.is_empty()
            || r.adapter_ids
                .iter()
                .any(|id| id.eq_ignore_ascii_case(&a.id)))
        && (r.description_contains.is_empty()
            || r.description_contains.iter().any(|x| {
                a.description
                    .to_ascii_lowercase()
                    .contains(&x.to_ascii_lowercase())
            }))
        && (r.dns_suffixes.is_empty()
            || r.dns_suffixes
                .iter()
                .any(|x| suffix(x) == suffix(&a.dns_suffix)))
        && r.dns_servers.iter().all(|x| {
            a.dns_servers
                .iter()
                .any(|y| x.parse::<IpAddr>().ok() == y.parse::<IpAddr>().ok())
        })
        && r.route_prefixes.iter().all(|x| {
            s.routes.iter().any(|route| {
                route.interface_index == a.index
                    && x.parse::<IpNet>().ok() == route.prefix.parse::<IpNet>().ok()
            })
        })
}

/// All interfaces tied for the best local route. No traffic is sent. An equal-cost
/// tie across interfaces is ambiguous and must not count as confirmed routing.
pub fn best_interfaces(s: &Snapshot, destination: IpAddr) -> Vec<u32> {
    let mut best: Option<(u8, u64)> = None;
    let mut indices = Vec::new();
    for r in &s.routes {
        let Some(a) = s
            .adapters
            .iter()
            .find(|a| a.index == r.interface_index && usable(a))
        else {
            continue;
        };
        let Ok(net) = r.prefix.parse::<IpNet>() else {
            continue;
        };
        if !net.contains(&destination) {
            continue;
        }
        let metric = u64::from(r.metric)
            + u64::from(if destination.is_ipv4() {
                a.metric_v4
            } else {
                a.metric_v6
            });
        let rank = (net.prefix_len(), metric);
        if best.is_none() || best.is_some_and(|b| rank.0 > b.0 || (rank.0 == b.0 && rank.1 < b.1)) {
            best = Some(rank);
            indices.clear();
            indices.push(a.index);
        } else if best == Some(rank) && !indices.contains(&a.index) {
            indices.push(a.index);
        }
    }
    indices.sort_unstable();
    indices
}

pub fn evaluate(config: &Config, snapshot: Option<&Snapshot>) -> Vec<Observation> {
    config
        .profiles
        .iter()
        .map(|p| {
            let mut o = Observation {
                schema_version: 1,
                profile_id: p.id.clone(),
                customer_hint: p.customer_hint.clone(),
                state: State::Unknown,
                matching_adapters: Vec::new(),
                destinations_routed: 0,
                destinations_checked: p.destinations.len(),
                source: "windows_network_snapshot".into(),
            };
            let Some(s) = snapshot.filter(|s| s.schema_version == 1) else {
                return o;
            };
            let mut indices = Vec::new();
            for a in &s.adapters {
                if matches(p, a, s) {
                    o.matching_adapters.push(a.id.clone());
                    indices.push(a.index);
                }
            }
            o.matching_adapters.sort();
            o.matching_adapters.dedup();
            for d in &p.destinations {
                if let Ok(ip) = d.parse() {
                    let best = best_interfaces(s, ip);
                    if best.len() == 1 && indices.contains(&best[0]) {
                        o.destinations_routed += 1;
                    }
                }
            }
            o.state = if !indices.is_empty() {
                State::Present
            } else {
                State::Absent
            };
            o
        })
        .collect()
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Connection {
    pub id: String,
    pub name: String,
    pub customers: Vec<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub suggested_name: Option<String>,
}

/// Active learned contexts plus unlearned tunnel candidates. Network metadata
/// cannot prove authentication; virtual Ethernet tunnels need a recognizable
/// driver description until they have been learned.
pub fn active_connections(config: &Config, snapshot: &Snapshot) -> Vec<Connection> {
    let mut connections = Vec::new();
    for a in snapshot.adapters.iter().filter(|a| usable(a)) {
        if connection_ignored(config, &a.name) {
            continue;
        }
        let mut customers: Vec<String> = config
            .profiles
            .iter()
            .filter(|p| matches(p, a, snapshot))
            .map(|p| p.customer_hint.clone())
            .collect();
        customers.sort();
        customers.dedup();
        let description = a.description.to_ascii_lowercase();
        let tunnel = a.network_type.eq_ignore_ascii_case("ppp")
            || a.network_type.eq_ignore_ascii_case("tunnel")
            || [
                "vpn",
                "wireguard",
                "wintun",
                "tap-windows",
                "tap-win32",
                "tunnel",
                "anyconnect",
                "globalprotect",
                "fortinet",
                "juniper",
                "pulse secure",
                "ivanti",
                "defguard",
            ]
            .iter()
            .any(|token| description.contains(token));
        if customers.is_empty() && !tunnel {
            continue;
        }
        if !snapshot.routes.iter().any(|r| r.interface_index == a.index) {
            continue;
        }
        connections.push(Connection {
            id: a.id.clone(),
            name: a.name.clone(),
            suggested_name: if customers.is_empty() && !a.dns_suffix.trim().is_empty() {
                Some(a.dns_suffix.clone())
            } else {
                None
            },
            customers,
        });
    }
    connections.sort_by(|a, b| a.id.cmp(&b.id));
    connections
}
pub fn connection_ignored(config: &Config, name: &str) -> bool {
    config
        .ignored_connections
        .iter()
        .any(|ignored| ignored.trim().eq_ignore_ascii_case(name.trim()))
}

/// Produces conservative drafts for changed usable adapters, never auto-activates.
pub fn learn(before: &Snapshot, after: &Snapshot) -> Config {
    let mut profiles = Vec::new();
    for a in after.adapters.iter().filter(|a| usable(a)) {
        let old = before.adapters.iter().find(|x| x.id == a.id);
        let new_routes: Vec<String> = after
            .routes
            .iter()
            .filter(|r| {
                r.interface_index == a.index
                    && !before.routes.iter().any(|b| {
                        b.interface_index == old.map(|x| x.index).unwrap_or(u32::MAX)
                            && b.prefix == r.prefix
                    })
            })
            .map(|r| r.prefix.clone())
            .collect();
        let changed = old.is_none_or(|b| {
            !usable(b)
                || b.addresses != a.addresses
                || suffix(&b.dns_suffix) != suffix(&a.dns_suffix)
                || b.dns_servers != a.dns_servers
        }) || !new_routes.is_empty();
        if !changed {
            continue;
        }
        let mut rules = Rules {
            adapter_ids: vec![a.id.clone()],
            ..Default::default()
        };
        // Only require DNS values that changed; always-on DNS is weak evidence.
        if !a.dns_suffix.is_empty()
            && old.is_none_or(|b| suffix(&b.dns_suffix) != suffix(&a.dns_suffix))
        {
            rules.dns_suffixes.push(a.dns_suffix.clone());
        }
        rules.dns_servers = a
            .dns_servers
            .iter()
            .filter(|d| old.is_none_or(|b| !b.dns_servers.contains(d)))
            .cloned()
            .collect();
        // Avoid freezing every assigned-address/peer route into a fingerprint.
        // Broad non-host prefixes are better candidates; users still review them.
        let mut stable_routes: Vec<_> = new_routes
            .into_iter()
            .filter_map(|s| {
                let net = s.parse::<IpNet>().ok()?;
                let limit = if net.addr().is_ipv4() { 32 } else { 128 };
                (net.prefix_len() > 0 && net.prefix_len() < limit).then_some((net.prefix_len(), s))
            })
            .collect();
        stable_routes.sort();
        stable_routes.dedup();
        rules.route_prefixes = stable_routes.into_iter().take(3).map(|(_, s)| s).collect();
        profiles.push(Profile {
            id: format!("draft-{}", profiles.len() + 1),
            customer_hint: "REPLACE WITH CUSTOMER LABEL".into(),
            rules,
            destinations: Vec::new(),
        });
    }
    Config {
        schema_version: 1,
        poll_seconds: 5,
        autolearn: true,
        notify: true,
        ignored_connections: vec![],
        profiles,
    }
}
