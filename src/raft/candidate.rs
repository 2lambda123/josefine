use std::collections::HashMap;
use crate::raft::raft::{Apply, ApplyResult};
use crate::raft::raft::IO;
use crate::raft::raft::Command;
use crate::raft::raft::Raft;
use crate::raft::raft::Role;
use crate::raft::leader::Leader;
use crate::raft::follower::Follower;
use std::io::Error;
use log::{info, trace, warn};

pub struct Candidate {
    pub votes: HashMap<u64, bool>,
}

impl <T: IO> Raft<Candidate, T> {
    pub fn seek_election(mut self) -> Result<ApplyResult<T>, Error> {
        info!("{} seeking election", self.id);
        self.state.voted_for = self.id;
        let from = self.id;
        let term = self.state.current_term;
        self.apply(Command::Vote { from, term, voted: true })
    }
}

#[inline]
fn majority(total: usize) -> usize {
    if total == 1 {
        return 0;
    }

    (total / 2) + 1
}

impl <T: IO> Apply<T> for Raft<Candidate, T> {
    fn apply(mut self, command: Command) -> Result<ApplyResult<T>, Error> {
        trace!("Applying command {:?} to {}", command, self.id);

        match command {
            Command::Vote { voted, from, .. } => {
                self.inner.votes.insert(from, voted);
                let (votes, total) = self.inner.votes.clone().into_iter()
                    .fold((0, 0), |(mut votes, mut total), (id, vote) | {
                        if vote {
                            votes += 1;
                        }

                        total += 1;
                        (votes, total)
                    });

                if votes > majority(self.cluster.len()) {
                    let raft: Raft<Leader, T> = Raft::from(self);
                    return Ok(ApplyResult::Leader(raft));
                }

                Ok(ApplyResult::Candidate(self))
            },
            Command::Append { entries, .. } => {
                let mut raft: Raft<Follower, T> = Raft::from(self);
                raft.io.append(entries);
                Ok(ApplyResult::Follower(raft))
            },
            Command::Heartbeat { from, .. } => {
                let mut raft: Raft<Follower, T> = Raft::from(self);
                raft.io.heartbeat(from);
                Ok(ApplyResult::Follower(raft))
            },
            _ => Ok(ApplyResult::Candidate(self))
        }
    }
}

impl <T: IO> From<Raft<Candidate, T>> for Raft<Follower, T> {
    fn from(val: Raft<Candidate, T>) -> Raft<Follower, T> {
        Raft {
            id: val.id,
            state: val.state,
            outbox: val.outbox,
            sender: val.sender,
            cluster: val.cluster,
            io: val.io,
            role: Role::Follower,
            inner: Follower { leader_id: None },
        }
    }
}

impl <T: IO> From<Raft<Candidate, T>> for Raft<Leader, T> {
    fn from(val: Raft<Candidate, T>) -> Raft<Leader, T> {
        Raft {
            id: val.id,
            state: val.state,
            outbox: val.outbox,
            sender: val.sender,
            cluster: val.cluster,
            io: val.io,
            role: Role::Leader,
            inner: Leader {},
        }
    }
}