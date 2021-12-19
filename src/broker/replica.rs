use std::path::Path;
use crate::broker::config::BrokerId;
use crate::broker::log::Log;
use crate::broker::state::partition::Partition;

pub struct Replica {
    broker_id: BrokerId,
    partition: Partition,
    pub log: Log,
}

impl Replica {
    pub fn new(path: &Path, broker_id: BrokerId, partition: Partition) -> Self {
        let log = Log::new(&path.join("data").join(format!("{}", partition.id)));
        Self {
            broker_id,
            partition,
            log,
        }
    }
}

