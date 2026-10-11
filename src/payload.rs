//! Event privacy projection. Internal detection/management evidence is separate.
use super::*;
use aw_watcher_vpn::{PayloadMode, PayloadSettings};
#[derive(serde::Serialize)]
pub struct EventData {
    pub title: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    schema_version: Option<u32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    connections: Option<Vec<EventConnection>>,
}
#[derive(serde::Serialize)]
struct EventConnection {
    #[serde(skip_serializing_if = "Option::is_none")]
    id: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    name: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    customers: Option<Vec<String>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    suggested_name: Option<String>,
}
impl EventData {
    pub fn new(context: &Context, settings: &PayloadSettings) -> Self {
        let filtered = context
            .connections
            .iter()
            .map(|c| Connection {
                id: String::new(),
                name: if settings.connection_names {
                    c.name.clone()
                } else {
                    String::new()
                },
                customers: if settings.customers {
                    c.customers.clone()
                } else {
                    vec![]
                },
                suggested_name: None,
            })
            .collect();
        let title = Context::new(filtered).title;
        let extended = settings.mode == PayloadMode::Extended;
        let fields = settings.connection_ids
            || settings.connection_names
            || settings.customers
            || settings.suggested_names;
        Self {
            title,
            schema_version: (extended && settings.schema_version).then_some(3),
            connections: (extended && fields).then(|| {
                context
                    .connections
                    .iter()
                    .map(|c| EventConnection {
                        id: settings.connection_ids.then(|| c.id.clone()),
                        name: settings.connection_names.then(|| c.name.clone()),
                        customers: settings.customers.then(|| c.customers.clone()),
                        suggested_name: if settings.suggested_names {
                            c.suggested_name.clone()
                        } else {
                            None
                        },
                    })
                    .collect()
            }),
        }
    }
}
#[derive(serde::Serialize)]
pub struct Event<'a> {
    pub timestamp: String,
    pub duration: u32,
    pub data: &'a EventData,
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn strict_default_migration_and_privacy_projection() {
        let settings: PayloadSettings = serde_json::from_str("{}").unwrap();
        assert_eq!(settings, PayloadSettings::default());
        let context = Context::new(vec![Connection {
            id: "private-id".into(),
            name: "private-name".into(),
            customers: vec!["Customer B".into(), "Customer A".into()],
            suggested_name: Some("private-dns".into()),
        }]);
        assert_eq!(
            serde_json::to_string(&EventData::new(&context, &settings)).unwrap(),
            r#"{"title":"Customer A|Customer B"}"#
        );
        let settings = PayloadSettings {
            customers: false,
            mode: PayloadMode::Extended,
            ..settings
        };
        assert_eq!(
            serde_json::to_string(&EventData::new(&context, &settings)).unwrap(),
            r#"{"title":"Activity (context unavailable)"}"#
        );
        for (key, settings) in [
            (
                "id",
                PayloadSettings {
                    connection_ids: true,
                    ..settings.clone()
                },
            ),
            (
                "name",
                PayloadSettings {
                    connection_names: true,
                    ..settings.clone()
                },
            ),
            (
                "suggested_name",
                PayloadSettings {
                    suggested_names: true,
                    ..settings.clone()
                },
            ),
            (
                "customers",
                PayloadSettings {
                    customers: true,
                    ..settings.clone()
                },
            ),
        ] {
            let data = serde_json::to_value(EventData::new(&context, &settings)).unwrap();
            assert_eq!(data["connections"][0].as_object().unwrap().len(), 1);
            assert!(data["connections"][0].get(key).is_some());
            if key != "name" {
                assert!(!data["title"].as_str().unwrap().contains("private-name"));
            }
            assert!(!data["title"].as_str().unwrap().contains("private-dns"));
        }
        let all = PayloadSettings {
            mode: PayloadMode::Extended,
            schema_version: true,
            connection_ids: true,
            connection_names: true,
            customers: true,
            suggested_names: true,
        };
        let json = serde_json::to_string(&EventData::new(&context, &all)).unwrap();
        assert!(json
            .starts_with(r#"{"title":"Customer A|Customer B","schema_version":3,"connections":["#));
    }
}
