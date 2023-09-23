//! Composite nodes that run children parallelly.

use std::sync::{Arc, Mutex};
use genawaiter::sync::{Gen, Co};

use bevy::ecs::entity::Entity;

use crate::{Node, NodeResult, NodeRunner, NodeGen, NodeGenState, ResumeSignal, nullable_access::NullableWorldAccess};

pub mod variants;


/// Node that runs children in parallel.
pub struct Parallel {
    children: Vec<Arc<dyn Node>>,
    checker: Box<dyn Fn(Vec<&NodeGenState>) -> NodeGenState + 'static + Send + Sync>,
}
impl Parallel {
    pub fn new(
        children: Vec<Arc<dyn Node>>,
        checker: impl Fn(Vec<&NodeGenState>) -> NodeGenState + 'static + Send + Sync,
    ) -> Arc<Self> {
        Arc::new(Self {
            children,
            checker: Box::new(checker)
        })
    }
}
impl Node for Parallel {
    fn run(self: Arc<Self>, world: Arc<Mutex<NullableWorldAccess>>, entity: Entity) -> Box<dyn NodeGen> {
        let producer = |co: Co<(), ResumeSignal>| async move {
            let mut children: Vec<NodeRunner> = self.children.iter().map(|child| {
                NodeRunner::new(child.clone(), world.clone(), entity)
            }).collect();
            let mut node_res: Option<NodeResult> = None;
            while node_res.is_none() {
                match (self.checker)(children.iter().map(|runner| runner.state()).collect()) {
                    NodeGenState::Complete(res) => {
                        node_res = Some(res);
                    },
                    NodeGenState::Yielded(()) => {
                        let signal = co.yield_(()).await;
                        match signal {
                            ResumeSignal::Abort => {
                                node_res = Some(NodeResult::Aborted);
                            },
                            ResumeSignal::Resume => {
                                for child in children.iter_mut() {
                                    child.resume_if_incomplete();
                                }
                                let debug_print: Vec<String> = children.iter()
                                .map(|child|
                                    match child.state() {
                                        NodeGenState::Yielded(()) => format!("yield"),
                                        NodeGenState::Complete(res) => format!("{:?}", res),
                                    }
                                ).collect();
                                println!("{:?}", debug_print);
                            }
                        }
                    }
                }
            };
            // abort rest
            for mut child in children {
                child.abort_if_incomplete();
            }
            node_res.unwrap()
        };
        Box::new(Gen::new(producer))
    }
}

