use petgraph::graph::NodeIndex;
use topology::dragonfly::{generate, DragonflyParams, LinkParams};
use topology::graph::{LinkClass, NodeKind};

#[test]
fn node_and_link_counts_perfect() {
    let p = DragonflyParams { a: 4, p: 4, h: 3, g: 13 };
    let t = generate(p, LinkParams::default()).unwrap();
    assert!(p.is_perfect());
    assert_eq!(t.node_count(), 260);

    let (mut term, mut local, mut glob) = (0, 0, 0);
    for e in t.inner().edge_indices() {
        match t.inner().edge_weight(e).unwrap().class {
            LinkClass::Terminal => term += 1,
            LinkClass::Local    => local += 1,
            LinkClass::Global   => glob += 1,
            LinkClass::Generic  => panic!(),
        }
    }
    assert_eq!((term, local, glob), (208, 78, 78));
}

#[test]
fn every_router_has_h_global_links() {
    let p = DragonflyParams { a: 4, p: 4, h: 3, g: 13 };
    let t = generate(p, LinkParams::default()).unwrap();
    for grp in 0..p.g {
        for r in 0..p.a {
            let rid = NodeIndex::new(p.router_id(grp, r));
            let deg = t.inner().edges(rid)
                .filter(|e| e.weight().class == LinkClass::Global)
                .count();
            assert_eq!(deg, p.h, "router ({grp},{r})");
        }
    }
}

#[test]
fn each_group_pair_once() {
    let p = DragonflyParams { a: 4, p: 4, h: 3, g: 13 };
    let t = generate(p, LinkParams::default()).unwrap();
    let g_of = |i: NodeIndex| match t.node(i).unwrap().kind {
        NodeKind::DragonflyRouter { group, .. } => group,
        _ => panic!(),
    };
    let mut pairs = std::collections::HashMap::<(usize, usize), usize>::new();
    for e in t.inner().edge_indices() {
        if t.inner().edge_weight(e).unwrap().class != LinkClass::Global { continue; }
        let (x, y) = t.inner().edge_endpoints(e).unwrap();
        let (g1, g2) = (g_of(x), g_of(y));
        *pairs.entry(if g1 < g2 { (g1, g2) } else { (g2, g1) }).or_default() += 1;
    }
    assert_eq!(pairs.len(), p.g * (p.g - 1) / 2);
    assert!(pairs.values().all(|&c| c == 1));
}

#[test]
fn connected() {
    let p = DragonflyParams { a: 4, p: 4, h: 3, g: 13 };
    assert!(generate(p, LinkParams::default()).unwrap().is_connected());
}

#[test]
fn rejects_bad_params() {
    let d = LinkParams::default();
    assert!(generate(DragonflyParams { a: 1, p: 4, h: 1, g: 2 }, d).is_err());
    assert!(generate(DragonflyParams { a: 4, p: 0, h: 1, g: 2 }, d).is_err());
    assert!(generate(DragonflyParams { a: 4, p: 4, h: 5, g: 13 }, d).is_err());
}