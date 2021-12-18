use std::ops::Index;
use crate::broker::fsm::Transition;
use crate::error::Result;
use crate::broker::broker::{Broker, Handler};
use crate::broker::state::topic::Topic;
use async_trait::async_trait;
use kafka_protocol::messages::create_topics_response::CreatableTopicResult;
use kafka_protocol::messages::{CreateTopicsRequest, CreateTopicsResponse};
use kafka_protocol::messages::create_topics_request::CreatableTopic;
use kafka_protocol::messages::leader_and_isr_response::LeaderAndIsrTopicError;
use rand::seq::SliceRandom;
use rand::thread_rng;
use uuid::Uuid;
use crate::broker::config::BrokerId;
use crate::broker::state::partition::{Partition, PartitionIdx};

impl Broker {
    async fn make_partitions(&self, name: &str, topic: &CreatableTopic) -> Result<Vec<Partition>> {
        let mut brokers = self.get_brokers();

        if topic.replication_factor > brokers.len() as i16 {
            // TODO
        }

        let mut partitions = Vec::new();

        for i in 0..topic.num_partitions {
            brokers.shuffle(&mut thread_rng());
            let leader = brokers.first().unwrap();

            let replicas: Vec<i32> = brokers.iter()
                .take(topic.replication_factor as usize)
                .map(|x| x.0)
                .collect();

            let partition = Partition {
                idx: PartitionIdx(i),
                topic: name.to_string(),
                isr: replicas.clone(),
                assigned_replicas: replicas,
                leader: leader.0,
            };

            partitions.push(partition);
        }

        Ok(partitions)
    }

    async fn create_topic(&self, name: &str, t: CreatableTopic) -> Result<CreatableTopicResult> {
        let topic = Topic {
            id: Uuid::new_v4(),
            name: (*name).to_string(),
            internal: false,
            ..Default::default()
        };

        let mut res = CreatableTopicResult::default();
        res.topic_id = topic.id;
        res.num_partitions = t.num_partitions;
        res.replication_factor = t.replication_factor;

        self
            .client
            .propose(Transition::EnsureTopic(topic).serialize()?)
            .await?;

        let ps = self.make_partitions(name, &t).await?;

        // TODO we should really do topic + partitions within single tx
        for p in ps {
            &self.client.propose(Transition::EnsurePartition(p).serialize()?).await?;
        }

        Ok(res)
    }
}

#[async_trait]
impl Handler<CreateTopicsRequest> for Broker {
    async fn handle(
        &self,
        req: CreateTopicsRequest,
        mut res: CreateTopicsResponse,
    ) -> Result<CreateTopicsResponse> {
        for (name, topic) in req.topics.into_iter() {
            if self.store.topic_exists(&name)? {
                // TODO
            }

            let t = self.create_topic(&name, topic).await?;
            res.topics.insert(name, t);
        }
        Ok(res)
    }
}

#[cfg(test)]
mod tests {
    use std::collections::HashMap;
    use crate::broker::broker::test::new_broker;
    use crate::broker::broker::Broker;
    use crate::broker::state::topic::Topic;
    use crate::error::Result;
    use crate::broker::broker::Handler;
    use kafka_protocol::messages::create_topics_request::CreatableTopic;
    use kafka_protocol::messages::{CreateTopicsRequest, CreateTopicsResponse, TopicName};
    use kafka_protocol::protocol::StrBytes;

    #[tokio::test]
    async fn execute() -> Result<()> {
        let (mut rx, broker) = new_broker();
        let mut req = CreateTopicsRequest::default();
        let topic_name = TopicName(StrBytes::from_str("Test"));
        req.topics
            .insert(topic_name.clone(), CreatableTopic::default());
        let (res, _) = tokio::join!(
            tokio::spawn(async move {
                Result::Ok(
                    broker.handle(req, CreateTopicsResponse::default())
                        .await?,
                )
            }),
            tokio::spawn(async move {
                let (_, cb) = rx.recv().await.unwrap();
                let topic = Topic {
                    id: uuid::Uuid::new_v4(),
                    name: "Test".to_string(),
                    internal: false,
                    partitions: HashMap::new(),
                };
                cb.send(Ok(crate::raft::rpc::Response::new(bincode::serialize(
                    &topic,
                )?)));
                Result::Ok(())
            }),
        );

        let res = res??;
        let name = res.topics.keys().next().unwrap();
        assert_eq!(&topic_name, name);
        Ok(())
    }
}