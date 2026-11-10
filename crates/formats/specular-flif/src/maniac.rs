//! MANIAC trees: their decoding and the property-driven context selection that grows the
//! tree while pixels are decoded.

use crate::Error;
use crate::rac::{ChanceTable, Rac, SimpleCoder, SymbolChances, default_table, read_nz};

#[derive(Clone, Copy, Debug)]
pub(crate) struct Node {
    /// -1 for a leaf.
    property: i8,
    /// Uses left before the node splits; negative once split.
    count: i32,
    splitval: i32,
    child: u32,
    leaf: u32,
}

impl Default for Node {
    fn default() -> Self {
        Node {
            property: -1,
            count: 0,
            splitval: 0,
            child: 0,
            leaf: 0,
        }
    }
}

/// A decoded tree (one root leaf when the tree is not coded yet).
#[derive(Clone)]
pub(crate) struct Tree {
    nodes: Vec<Node>,
}

impl Default for Tree {
    fn default() -> Self {
        Tree {
            nodes: vec![Node::default()],
        }
    }
}

enum Step {
    Visit(usize),
    Right {
        p: usize,
        oldmin: i32,
        split: i32,
        oldmax: i32,
        child: usize,
    },
    Restore {
        p: usize,
        oldmax: i32,
    },
}

/// Reads a tree for property ranges `ranges`; at most `max_nodes` nodes.
pub(crate) fn read_tree(
    rac: &mut Rac,
    ranges: &[(i32, i32)],
    max_nodes: usize,
) -> Result<Tree, Error> {
    let table = default_table();
    let mut coders = [SimpleCoder::new(), SimpleCoder::new(), SimpleCoder::new()];
    let mut sub = ranges.to_vec();
    let nprops = ranges.len() as i32;
    let mut nodes = vec![Node::default()];
    let mut stack = vec![Step::Visit(0)];
    while let Some(step) = stack.pop() {
        match step {
            Step::Visit(pos) => {
                let p = coders[0].read2(rac, &table, 0, nprops) - 1;
                if p < 0 {
                    continue;
                }
                let p = p as usize;
                let Some(&(oldmin, oldmax)) = sub.get(p) else {
                    return Err(Error::Malformed("invalid MANIAC tree".into()));
                };
                if oldmin >= oldmax {
                    return Err(Error::Malformed("invalid MANIAC tree".into()));
                }
                let count = coders[1].read2(rac, &table, 1, 512);
                let split = coders[2].read2(rac, &table, oldmin, oldmax - 1);
                let child = nodes.len();
                if child + 2 > max_nodes {
                    return Err(Error::Malformed("MANIAC tree too large".into()));
                }
                if rac.eof() && child > 4096 {
                    return Err(Error::Truncated("MANIAC tree cut short".into()));
                }
                nodes[pos] = Node {
                    property: p as i8,
                    count,
                    splitval: split,
                    child: child as u32,
                    leaf: 0,
                };
                nodes.push(Node::default());
                nodes.push(Node::default());
                sub[p].0 = split + 1;
                stack.push(Step::Right {
                    p,
                    oldmin,
                    split,
                    oldmax,
                    child,
                });
                stack.push(Step::Visit(child));
            }
            Step::Right {
                p,
                oldmin,
                split,
                oldmax,
                child,
            } => {
                sub[p].0 = oldmin;
                sub[p].1 = split;
                stack.push(Step::Restore { p, oldmax });
                stack.push(Step::Visit(child + 1));
            }
            Step::Restore { p, oldmax } => sub[p].1 = oldmax,
        }
    }
    Ok(Tree { nodes })
}

/// The pixel coder of one channel: its tree (growing as it is used) and the chances of
/// its leaves.
pub(crate) struct PropertyCoder {
    nodes: Vec<Node>,
    leaves: Vec<SymbolChances>,
}

impl PropertyCoder {
    pub(crate) fn new(tree: &Tree) -> PropertyCoder {
        let mut nodes = tree.nodes.clone();
        nodes[0].leaf = 0;
        PropertyCoder {
            nodes,
            leaves: vec![SymbolChances::default()],
        }
    }

    #[inline]
    fn find_leaf(&mut self, props: &[i32]) -> usize {
        let mut pos = 0usize;
        loop {
            let n = self.nodes[pos];
            if n.property < 0 {
                return n.leaf as usize;
            }
            let prop = props.get(n.property as usize).copied().unwrap_or(0);
            if n.count < 0 {
                pos = if prop > n.splitval {
                    n.child as usize
                } else {
                    n.child as usize + 1
                };
            } else if n.count > 0 {
                self.nodes[pos].count -= 1;
                return n.leaf as usize;
            } else {
                self.nodes[pos].count = -1;
                let old = n.leaf as usize;
                let new = self.leaves.len();
                let copy = self.leaves[old].clone();
                self.leaves.push(copy);
                self.nodes[n.child as usize].leaf = old as u32;
                self.nodes[n.child as usize + 1].leaf = new as u32;
                return if prop > n.splitval { old } else { new };
            }
        }
    }

    #[inline]
    pub(crate) fn read(
        &mut self,
        rac: &mut Rac,
        table: &ChanceTable,
        props: &[i32],
        min: i32,
        max: i32,
    ) -> i32 {
        if min == max {
            return min;
        }
        let leaf = self.find_leaf(props);
        read_nz(rac, table, &mut self.leaves[leaf], min, max)
    }
}
