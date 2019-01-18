use std::io::Error;

use slog::Logger;

use crate::election::{Election, ElectionStatus};
use crate::follower::Follower;
use crate::leader::Leader;
use crate::raft::{Apply, RaftHandle};
use crate::raft::Command;
use crate::raft::Io;
use crate::raft::Raft;
use crate::raft::Role;
use crate::rpc::Rpc;
use crate::progress::ReplicationProgress;
use std::time::Instant;
use std::time::Duration;

pub struct Candidate {
    pub election: Election,
    pub log: Logger,
}

impl<I: Io, R: Rpc> Raft<Candidate, I, R> {
    pub(crate) fn seek_election(mut self) -> Result<RaftHandle<I, R>, Error> {
        info!(self.role.log, "Seeking election");
        self.state.voted_for = Some(self.id);
        self.state.current_term += 1;
        let from = self.id;
        let term = self.state.current_term;
        self.apply(Command::VoteResponse { candidate_id: from, term, granted: true })
    }
}

impl Role for Candidate {
    fn term(&mut self, _term: u64) {
        self.election.reset();
    }
}

impl<I: Io, R: Rpc> Apply<I, R> for Raft<Candidate, I, R> {
    fn apply(mut self, command: Command) -> Result<RaftHandle<I, R>, Error> {
//        trace!("Applying command {:?} to {}", "", command, self.id);

        match command {
            Command::Tick => {
                Ok(RaftHandle::Candidate(self))
            }
            Command::VoteRequest { candidate_id, term: _, .. } => {
                self.rpc.respond_vote(&self.state, candidate_id, false);
                Ok(RaftHandle::Candidate(self))
            }
            Command::VoteResponse { granted, candidate_id, .. } => {
                self.role.election.vote(candidate_id, granted);
                match self.role.election.election_status() {
                    ElectionStatus::Elected => Ok(RaftHandle::Leader(Raft::from(self))),
                    ElectionStatus::Voting => Ok(RaftHandle::Candidate(self)),
                    ElectionStatus::Defeated => Ok(RaftHandle::Follower(Raft::from(self))),
                }
            }
            Command::AppendEntries { mut entries, term, .. } => {
                if term >= self.state.current_term {
                    info!(self.role.log, "Received higher term, transitioning to follower");
                    let mut raft: Raft<Follower, I, R> = Raft::from(self);
                    raft.io.append(&mut entries);
                    return Ok(RaftHandle::Follower(raft));
                }

                Ok(RaftHandle::Candidate(self))
            }
            Command::Heartbeat { term, leader_id, .. } => {
                if term >= self.state.current_term {
                    info!(self.role.log, "Received higher term, transitioning to follower");
                    let mut raft: Raft<Follower, I, R> = Raft::from(self);
                    raft.io.heartbeat(leader_id);
                    return Ok(RaftHandle::Follower(raft));
                }

                Ok(RaftHandle::Candidate(self))
            }
            _ => Ok(RaftHandle::Candidate(self))
        }
    }
}

impl<I: Io, R: Rpc> From<Raft<Candidate, I, R>> for Raft<Follower, I, R> {
    fn from(val: Raft<Candidate, I, R>) -> Raft<Follower, I, R> {
        Raft {
            id: val.id,
            state: val.state,
            nodes: val.nodes,
            io: val.io,
            rpc: val.rpc,
            role: Follower { leader_id: None, log: val.log.new(o!("role" => "follower")) },
            log: val.log,
            config: val.config,
        }
    }
}

impl<I: Io, R: Rpc> From<Raft<Candidate, I, R>> for Raft<Leader, I, R> {
    fn from(val: Raft<Candidate, I, R>) -> Raft<Leader, I, R> {
        info!(val.role.log, "Becoming the leader");
        Raft {
            id: val.id,
            state: val.state,
            nodes: val.nodes.clone(),
            io: val.io,
            rpc: val.rpc,
            role: Leader {
                log: val.log.new(o!("role" => "leader", "nodes" => format!("{:?}", val.nodes.read().unwrap()))),
                progress: ReplicationProgress::new(val.nodes.clone()),
                heartbeat_time: Instant::now(),
                heartbeat_timeout: val.config.heartbeat_timeout,
            },
            log: val.log,
            config: val.config,
        }
    }
}
