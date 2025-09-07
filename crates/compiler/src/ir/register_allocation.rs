use super::{BlockId, VarId};
use crate::ir;
use anyhow::Context;
use stationeers_mips::types::Register;
use std::collections::{HashMap, HashSet};

pub struct RegisterAllocation {
    vars: HashMap<VarId, Register>,
}

impl RegisterAllocation {
    pub fn allocate(ir_program: &ir::Program) -> anyhow::Result<Self> {
        // TODO:
        let mut next = 0;
        // let mut vars = HashMap::default();

        let mut var_to_node: HashMap<VarId, i32> = HashMap::default();
        // Assign vars to nodes

        // First, assign registers for PHI variables
        for block in &ir_program.blocks {
            for ins in &block.instructions {
                if let ir::Instruction::Assignment { id, value } = ins {
                    if let ir::VarValue::Phi(phi) = value {
                        var_to_node.insert(*id, next);
                        for var_id in phi {
                            var_to_node.insert(*var_id, next);
                        }
                        next += 1;
                    }
                }
            }
        }
        // The assign all remaining variables.
        for block in &ir_program.blocks {
            for ins in &block.instructions {
                if let ir::Instruction::Assignment { id, value: _ } = ins {
                    if var_to_node.contains_key(id) {
                        continue;
                    }
                    var_to_node.insert(*id, next);
                    next += 1;
                }
            }
        }

        tracing::info!(
            "Initial IR program has {} variables, mapped to {} graph nodes. VarToNode:\n{:?}",
            var_to_node.len(),
            next,
            var_to_node,
        );

        // Construct graph
        let mut graph = Graph::default();
        let mut vars: Vec<VarId> = var_to_node.keys().copied().collect();
        vars.sort();
        for var_id in &vars {
            add_edges(&mut graph, ir_program, *var_id, &var_to_node);
        }
        tracing::debug!("Graph: {:?}", graph);

        let mut colors = HashMap::default();
        anyhow::ensure!(
            color_graph(&mut graph, &mut colors),
            "The program is too complex, failed to perform register allocation"
        );
        tracing::debug!("Colors: {:?}", colors);

        let mut var_to_register = HashMap::default();
        for var_id in vars {
            let node = var_to_node
                .get(&var_id)
                .context(format!("var_to_node[{:?}] missing", var_id))
                .unwrap();
            let color = colors
                .get(&node)
                .context(format!(
                    "color missing for var: {:?} node: {:?}",
                    var_id, node
                ))
                .unwrap();
            var_to_register.insert(var_id, (*color as u8).into());
        }

        Ok(Self {
            vars: var_to_register,
        })
    }

    pub fn get(&self, var_id: VarId) -> Option<Register> {
        self.vars.get(&var_id).copied()
    }
}

#[derive(Default, Debug)]
struct Graph {
    edges: HashMap<i32, HashSet<i32>>,
}

impl Graph {
    fn add_edge(&mut self, node1: i32, node2: i32) {
        if node1 == node2 {
            return;
        }
        tracing::trace!("add_edge({node1}, {node2})");
        self.edges.entry(node1).or_default().insert(node2);
        self.edges.entry(node2).or_default().insert(node1);
        tracing::trace!("graph: {:?}", self);
    }
    fn remove_node(&mut self, node: i32) -> HashSet<i32> {
        let edges = self.edges.remove(&node).unwrap();
        for e in &edges {
            if let Some(x) = self.edges.get_mut(&e) {
                x.remove(&node);
            }
        }
        edges
    }
}

// node->color
fn color_graph(g: &mut Graph, colors: &mut HashMap<i32, i32>) -> bool {
    if g.edges.is_empty() {
        return true;
    }
    let mut nodes: Vec<i32> = g.edges.keys().into_iter().copied().collect();
    nodes.sort();
    // unwrap ok, guaranteed to have a key
    let node = nodes
        .into_iter()
        .filter(|n| g.edges.get(n).unwrap().len() < 16)
        .next();
    let node = match node {
        None => {
            tracing::debug!("Graph too complex to color:\n{:?}", g);
            return false;
        }
        Some(node) => node,
    };
    let edges = g.remove_node(node);
    tracing::trace!("start coloring: {node}, edges: {:?}", edges);
    if !color_graph(g, colors) {
        return false;
    }
    tracing::trace!("end coloring: {node}, edges: {:?}", edges);
    let used_colors: HashSet<i32> = edges
        .into_iter()
        .map(|e| {
            colors
                .get(&e)
                .context(format!("node {} not colored", e))
                .unwrap()
        })
        .copied()
        .collect();
    for color in 0..16 {
        if !used_colors.contains(&color) {
            colors.insert(node, color);
            tracing::trace!("colored: {node}, color {color}");
            return true;
        }
    }
    unreachable!()
}

fn find_var(program: &ir::Program, var_id: VarId) -> (BlockId, usize) {
    for (block_idx, block) in program.blocks.iter().enumerate() {
        for (ins_idx, ins) in block.instructions.iter().enumerate() {
            if let ir::Instruction::Assignment { id, .. } = ins {
                if id == &var_id {
                    return (BlockId(block_idx), ins_idx);
                }
            }
        }
    }
    panic!("Could not find assignment for var {:?}", var_id)
}

fn add_edges_rec(
    graph: &mut Graph,
    program: &ir::Program,
    pos: (BlockId, usize),
    var_id: VarId,
    visited: &mut HashSet<BlockId>,
    var_to_node: &HashMap<VarId, i32>,
) -> bool {
    tracing::trace!("add_edges_rec({:?}), pos={:?}", var_id, pos);
    let block = &program.blocks[pos.0 .0];
    let mut used = false;
    if pos.1 >= block.instructions.len() {
        for n in &block.next {
            if !visited.contains(&n) {
                visited.insert(*n);
                used |= add_edges_rec(graph, program, (*n, 0), var_id, visited, var_to_node);
            }
        }
        return used;
    }
    let used_later = add_edges_rec(
        graph,
        program,
        (pos.0, pos.1 + 1),
        var_id,
        visited,
        var_to_node,
    );
    used |= used_later;

    let ins = &block.instructions[pos.1];
    let used_vars = match ins {
        ir::Instruction::Assignment { id, value } => {
            let mut v = value.used_vars();
            v.insert(*id);
            v
        }
        ir::Instruction::Branch {
            cond,
            true_block: _,
            false_block: _,
        } => cond.used_vars(),
        ir::Instruction::Yield => HashSet::default(),
    };
    used |= used_vars.contains(&var_id);

    if used_later {
        let node = var_to_node.get(&var_id).unwrap();
        for used_var_id in used_vars {
            let used_node = var_to_node.get(&used_var_id).unwrap();
            graph.add_edge(*node, *used_node);
        }
    }

    used
}

fn add_edges(
    graph: &mut Graph,
    program: &ir::Program,
    var_id: VarId,
    var_to_node: &HashMap<VarId, i32>,
) {
    let mut start = find_var(program, var_id);
    // Move to the next instruction after declaration.
    start.1 += 1;
    let mut visited = HashSet::default();
    visited.insert(start.0);
    tracing::trace!("add_edges({:?}), pos = {:?}", var_id, start);
    graph.edges.entry(var_to_node[&var_id]).or_default();
    add_edges_rec(graph, program, start, var_id, &mut visited, var_to_node);
}
