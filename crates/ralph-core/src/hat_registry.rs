//! Hat registry for managing agent personas.

use crate::config::{HatConfig, RalphConfig};
use ralph_proto::{Hat, HatId, Topic};
use std::collections::HashMap;

/// Registry for managing and creating hats from configuration.
#[derive(Debug, Default)]
pub struct HatRegistry {
    hats: HashMap<HatId, Hat>,
}

impl HatRegistry {
    /// Creates a new empty registry.
    pub fn new() -> Self {
        Self::default()
    }

    /// Creates a registry from configuration.
    ///
    /// Empty config → empty registry (no hats).
    /// HatlessRalph is the fallback coordinator when no hats are configured.
    pub fn from_config(config: &RalphConfig) -> Self {
        let mut registry = Self::new();

        for (id, hat_config) in &config.hats {
            let hat = Self::hat_from_config(id, hat_config);
            registry.register(hat);
        }

        registry
    }

    /// Creates a Hat from HatConfig.
    fn hat_from_config(id: &str, config: &HatConfig) -> Hat {
        let mut hat = Hat::new(id, &config.name);
        hat.subscriptions = config.trigger_topics();
        hat.publishes = config.publish_topics();
        hat.instructions = config.instructions.clone();
        hat
    }

    /// Registers a hat with the registry.
    pub fn register(&mut self, hat: Hat) {
        self.hats.insert(hat.id.clone(), hat);
    }

    /// Gets a hat by ID.
    pub fn get(&self, id: &HatId) -> Option<&Hat> {
        self.hats.get(id)
    }

    /// Returns all hats in the registry.
    pub fn all(&self) -> impl Iterator<Item = &Hat> {
        self.hats.values()
    }

    /// Returns all hat IDs.
    pub fn ids(&self) -> impl Iterator<Item = &HatId> {
        self.hats.keys()
    }

    /// Returns the number of registered hats.
    pub fn len(&self) -> usize {
        self.hats.len()
    }

    /// Returns true if no hats are registered.
    pub fn is_empty(&self) -> bool {
        self.hats.is_empty()
    }

    /// Finds all hats subscribed to a topic.
    pub fn subscribers(&self, topic: &Topic) -> Vec<&Hat> {
        self.hats
            .values()
            .filter(|hat| hat.is_subscribed(topic))
            .collect()
    }

    /// Finds the first hat that would be triggered by a topic.
    /// Returns the hat ID if found, used for event logging.
    pub fn find_by_trigger(&self, topic: &str) -> Option<&HatId> {
        let topic = Topic::new(topic);
        self.hats
            .values()
            .find(|hat| hat.is_subscribed(&topic))
            .map(|hat| &hat.id)
    }

    /// Returns true if any hat is subscribed to the given topic.
    pub fn has_subscriber(&self, topic: &str) -> bool {
        let topic = Topic::new(topic);
        self.hats.values().any(|hat| hat.is_subscribed(&topic))
    }

    /// Returns the first hat subscribed to the given topic.
    pub fn get_for_topic(&self, topic: &str) -> Option<&Hat> {
        let topic = Topic::new(topic);
        self.hats.values().find(|hat| hat.is_subscribed(&topic))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_empty_config_creates_empty_registry() {
        let config = RalphConfig::default();
        let registry = HatRegistry::from_config(&config);

        // Empty config → empty registry (HatlessRalph is the fallback)
        assert_eq!(registry.len(), 0);
        assert!(registry.is_empty());
    }

    #[test]
    fn test_custom_hats_from_config() {
        let yaml = r#"
hats:
  implementer:
    name: "Implementer"
    triggers: ["task.*"]
  reviewer:
    name: "Reviewer"
    triggers: ["impl.*"]
"#;
        let config: RalphConfig = serde_yaml::from_str(yaml).unwrap();
        let registry = HatRegistry::from_config(&config);

        assert_eq!(registry.len(), 2);

        let impl_hat = registry.get(&HatId::new("implementer")).unwrap();
        assert!(impl_hat.is_subscribed(&Topic::new("task.start")));
        assert!(!impl_hat.is_subscribed(&Topic::new("impl.done")));

        let review_hat = registry.get(&HatId::new("reviewer")).unwrap();
        assert!(review_hat.is_subscribed(&Topic::new("impl.done")));
    }

    #[test]
    fn test_has_subscriber() {
        let yaml = r#"
hats:
  impl:
    name: "Implementer"
    triggers: ["task.*"]
"#;
        let config: RalphConfig = serde_yaml::from_str(yaml).unwrap();
        let registry = HatRegistry::from_config(&config);

        assert!(registry.has_subscriber("task.start"));
        assert!(!registry.has_subscriber("build.task"));
    }

    #[test]
    fn test_get_for_topic() {
        let yaml = r#"
hats:
  impl:
    name: "Implementer"
    triggers: ["task.*"]
"#;
        let config: RalphConfig = serde_yaml::from_str(yaml).unwrap();
        let registry = HatRegistry::from_config(&config);

        let hat = registry.get_for_topic("task.start");
        assert!(hat.is_some());
        assert_eq!(hat.unwrap().id.as_str(), "impl");

        let no_hat = registry.get_for_topic("build.task");
        assert!(no_hat.is_none());
    }

    #[test]
    fn test_empty_registry_has_no_subscribers() {
        let config = RalphConfig::default();
        let registry = HatRegistry::from_config(&config);

        assert!(!registry.has_subscriber("build.task"));
        assert!(registry.get_for_topic("build.task").is_none());
    }

    #[test]
    fn test_find_subscribers() {
        let yaml = r#"
hats:
  impl:
    name: "Implementer"
    triggers: ["task.*", "review.done"]
  reviewer:
    name: "Reviewer"
    triggers: ["impl.*"]
"#;
        let config: RalphConfig = serde_yaml::from_str(yaml).unwrap();
        let registry = HatRegistry::from_config(&config);

        let task_subs = registry.subscribers(&Topic::new("task.start"));
        assert_eq!(task_subs.len(), 1);
        assert_eq!(task_subs[0].id.as_str(), "impl");

        let impl_subs = registry.subscribers(&Topic::new("impl.done"));
        assert_eq!(impl_subs.len(), 1);
        assert_eq!(impl_subs[0].id.as_str(), "reviewer");
    }
}
