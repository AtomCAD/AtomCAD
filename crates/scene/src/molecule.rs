use std::{collections::HashMap, iter::Empty};

use periodic_table::Element;
use petgraph::{
    data::{Build, DataMap},
    graph::Node,
    stable_graph::{self, NodeIndex},
};
use render::{AtomKind, AtomRepr, Atoms, GlobalRenderResources};
use ultraviolet::Vec3;

use crate::{
    feature::{FeatureList, MoleculeCommands, RootFeature},
    ids::{AtomSpecifier, FeatureCopyId},
};

type Graph = stable_graph::StableUnGraph<AtomNode, BondOrder>;
pub type BondOrder = u8;
pub type AtomIndex = stable_graph::NodeIndex;
pub type BondIndex = stable_graph::EdgeIndex;

pub struct AtomNode {
    pub element: Element,
    pub pos: Vec3,
    pub spec: AtomSpecifier,
}

/// The concrete representation of the molecule at some time in the feature history.
pub struct MoleculeRepr {
    // TODO: This atom map is a simple but extremely inefficient implementation. This data
    // is highly structued and repetitive: compression, flattening, and a tree could do
    // a lot to optimize this.
    atom_map: HashMap<AtomSpecifier, AtomIndex>,
    gpu_atoms: Atoms,
    graph: Graph,
    gpu_synced: bool,
}

impl MoleculeRepr {
    pub fn reupload_atoms(&mut self, gpu_resources: &GlobalRenderResources) {
        let atoms: Vec<AtomRepr> = self
            .graph
            .node_weights()
            .map(|node| AtomRepr {
                kind: AtomKind::new(node.element),
                pos: node.pos,
            })
            .collect();

        // TODO: not working, see shinzlet/atomCAD #3
        // self.gpu_atoms.reupload_atoms(&atoms, gpu_resources);

        // This is a workaround, but it has bad perf as it always drops and
        // reallocates
        self.gpu_atoms = Atoms::new(gpu_resources, atoms);
        self.gpu_synced = true;
    }

    pub fn atoms(&self) -> &Atoms {
        &self.gpu_atoms
    }
}

impl MoleculeCommands for MoleculeRepr {
    fn add_atom(&mut self, element: Element, pos: ultraviolet::Vec3, spec: AtomSpecifier) {
        let index = self.graph.add_node(AtomNode {
            element,
            pos,
            spec: spec.clone(),
        });
        self.atom_map.insert(spec, index);
        self.gpu_synced = false;
    }

    fn create_bond(&mut self, a1: &AtomSpecifier, a2: &AtomSpecifier, order: BondOrder) {
        match (self.atom_map.get(&a1), self.atom_map.get(&a2)) {
            (Some(&a1_index), Some(&a2_index)) => {
                self.graph.add_edge(a1_index, a2_index, order);
            }
            _ => {
                panic!("AtomSpecifiers referenced in a feature should always resolve");
            }
        }
    }

    fn find_atom(&self, spec: &AtomSpecifier) -> Option<&AtomNode> {
        match self.atom_map.get(&spec) {
            Some(atom_index) => self.graph.node_weight(*atom_index),
            None => None,
        }
    }
}

pub struct Molecule {
    pub repr: MoleculeRepr,
    rotation: ultraviolet::Rotor3,
    offset: ultraviolet::Vec3,
    features: FeatureList,
    // The index one greater than the most recently applied feature's location in the feature list.
    // This is unrelated to feature IDs: it is effectively just a counter of how many features are
    // applied. (i.e. our current location in the edit history timeline)
    history_step: usize,
}

impl Molecule {
    // TODO: from_feature

    // Creates a `Molecule` containing just one atom. At the moment, it is not possible
    // to construct a `Molecule` with no contents, as wgpu will panic if an empty gpu buffer
    // is created
    pub fn from_first_atom(gpu_resources: &GlobalRenderResources, first_atom: Element) -> Self {
        let mut graph = Graph::default();
        let spec = AtomSpecifier {
            feature_path: vec![FeatureCopyId {
                feature_id: 0,
                copy_index: 0,
            }],
            child_index: 0,
        };

        let first_index = graph.add_node(AtomNode {
            element: first_atom,
            pos: Vec3::default(),
            spec: spec.clone(),
        });

        let gpu_atoms = Atoms::new(
            gpu_resources,
            [AtomRepr {
                kind: AtomKind::new(first_atom),
                pos: Vec3::default(),
            }],
        );

        let mut features = FeatureList::default();
        features.push_back(RootFeature);

        Molecule {
            repr: MoleculeRepr {
                atom_map: HashMap::from([(spec, first_index)]),
                gpu_atoms,
                graph,
                gpu_synced: false,
            },
            rotation: ultraviolet::Rotor3::default(),
            offset: ultraviolet::Vec3::default(),
            features,
            history_step: 1,
        }
    }

    pub fn features(&self) -> &FeatureList {
        &self.features
    }

    pub fn with_features(&mut self, mut func: impl FnMut(&mut FeatureList) -> ()) {
        func(&mut self.features)
        // TODO: Either recompute the model every time this is called, or implement
        // a mechanism for FeatureList to track what has been altered and flag itself
        // as edited.
        // Could also make it illegal to modify the past using this method: i.e. make
        // the past features immutable and the future features mutable using some sort
        // of split list straddling the history step
    }

    // Recomputes the model to advance itself to a given history step.
    pub fn set_history_step(&mut self, history_step: usize) {
        // TODO: Handle stepping backwards. Right now this only allows stepping forwards
        // in the feature history
        assert!(
            history_step <= self.features.len(),
            "history step exceeds feature list size"
        );
        assert!(
            history_step > self.history_step,
            "stepping backwards in history is not yet implemented"
        );

        for feature_id in &self.features.order()[self.history_step..history_step] {
            let feature = self
                .features
                .get(feature_id)
                .expect("Feature IDs referenced by the FeatureList order should exist!");
            feature.apply(feature_id, &mut self.repr);
        }

        self.history_step = history_step;
    }

    // equivalent to `set_history_step(features.len()): applies every feature that is in the
    // feature timeline.
    pub fn apply_all_features(&mut self) {
        self.set_history_step(self.features.len())
    }
}
