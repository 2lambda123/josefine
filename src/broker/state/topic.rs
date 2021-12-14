use std::collections::HashMap;
use kafka_protocol::messages::create_topics_request::CreatableTopic;
use uuid::Uuid;

#[derive(Serialize, Deserialize, Debug, Clone, Eq, PartialEq, Default)]
pub struct Topic {
    pub id: Uuid,
    pub name: String,
    pub partitions: HashMap<i32, i32>,
    // Config TopicConfig
    // Internal, e.g. group metadata topic
    pub internal: bool,
}
