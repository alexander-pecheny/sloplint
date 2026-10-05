use crate::analyze::FileFacts;
use std::collections::HashMap;

pub struct Chain {
    pub file: usize,
    pub function: usize,
    pub path: Vec<String>,
}

pub fn chains(files: &mut [FileFacts]) -> Vec<Chain> {
    let ids: Vec<(usize, usize)> = files.iter().enumerate().flat_map(|(fi, f)| (0..f.functions.len()).map(move |i| (fi, i))).collect();
    let mut by_name: HashMap<&str, Vec<usize>> = HashMap::new();
    for (id, &(fi, i)) in ids.iter().enumerate() {
        by_name.entry(files[fi].functions[i].name.as_str()).or_default().push(id);
    }
    let edges: Vec<Vec<usize>> = ids
        .iter()
        .enumerate()
        .map(|(id, &(fi, i))| {
            let mut out: Vec<usize> = files[fi].functions[i]
                .calls
                .iter()
                .filter_map(|c| by_name.get(c.as_str()).filter(|v| v.len() == 1).map(|v| v[0]))
                .filter(|&t| t != id)
                .collect();
            out.sort_unstable();
            out.dedup();
            out
        })
        .collect();
    let mut called = vec![false; ids.len()];
    for e in edges.iter().flatten() {
        called[*e] = true;
    }
    let mut g = Graph { edges: &edges, state: vec![0; ids.len()], depth: vec![0; ids.len()], next: vec![None; ids.len()] };
    for v in 0..ids.len() {
        g.visit(v);
    }
    for (id, &(fi, i)) in ids.iter().enumerate() {
        files[fi].functions[i].depth = g.depth[id];
    }
    (0..ids.len())
        .filter(|&v| !called[v])
        .map(|v| {
            let mut path = vec![];
            let mut cur = Some(v);
            while let Some(c) = cur {
                let (fi, i) = ids[c];
                path.push(files[fi].functions[i].name.clone());
                cur = g.next[c];
            }
            Chain { file: ids[v].0, function: ids[v].1, path }
        })
        .collect()
}

struct Graph<'a> {
    edges: &'a [Vec<usize>],
    state: Vec<u8>,
    depth: Vec<u32>,
    next: Vec<Option<usize>>,
}

impl Graph<'_> {
    fn visit(&mut self, v: usize) {
        if self.state[v] != 0 {
            return;
        }
        self.state[v] = 1;
        self.depth[v] = 1;
        for &w in &self.edges[v] {
            self.visit(w);
            if self.state[w] == 2 && self.depth[w] + 1 > self.depth[v] {
                self.depth[v] = self.depth[w] + 1;
                self.next[v] = Some(w);
            }
        }
        self.state[v] = 2;
    }
}
