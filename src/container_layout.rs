//! Authoritative workspace layout. Iced's binary tree is only a rendering adapter.
#[cfg(test)]
use iced::widget::pane_grid::Configuration;
use iced::{
    Size,
    widget::pane_grid::{self, Axis, Node, Pane, Split},
};
use std::{collections::BTreeMap, ops::Deref};
const GAP: f32 = 6.0;
#[derive(Clone, Debug, PartialEq, serde::Serialize, serde::Deserialize)]
pub enum Container {
    Panel(usize),
    Linear {
        #[serde(with = "axis_format")]
        axis: Axis,
        children: Vec<Container>,
        shares: Vec<f32>,
    },
}
mod axis_format {
    use super::*;
    pub fn serialize<S: serde::Serializer>(axis: &Axis, s: S) -> Result<S::Ok, S::Error> {
        s.serialize_str(if *axis == Axis::Horizontal {
            "rows"
        } else {
            "columns"
        })
    }
    pub fn deserialize<'de, D: serde::Deserializer<'de>>(d: D) -> Result<Axis, D::Error> {
        let value = <String as serde::Deserialize>::deserialize(d)?;
        match value.as_str() {
            "rows" => Ok(Axis::Horizontal),
            "columns" => Ok(Axis::Vertical),
            _ => Err(serde::de::Error::custom("invalid container axis")),
        }
    }
}
impl Container {
    pub fn linear(axis: Axis, children: Vec<Self>) -> Self {
        let shares = vec![1.0; children.len()];
        Self::weighted(axis, children, shares)
    }
    fn weighted(axis: Axis, children: Vec<Self>, shares: Vec<f32>) -> Self {
        let mut flat = vec![];
        let mut weights = vec![];
        for (child, weight) in children.into_iter().zip(shares) {
            if let Self::Linear {
                axis: child_axis,
                children,
                shares,
            } = child
            {
                if child_axis == axis {
                    let sum: f32 = shares.iter().sum();
                    for (child, share) in children.into_iter().zip(shares) {
                        flat.push(child);
                        weights.push(weight * share / sum);
                    }
                } else {
                    flat.push(Self::Linear {
                        axis: child_axis,
                        children,
                        shares,
                    });
                    weights.push(weight);
                }
            } else {
                flat.push(child);
                weights.push(weight);
            }
        }
        if flat.len() == 1 {
            return flat.remove(0);
        }
        let sum: f32 = weights.iter().sum();
        for w in &mut weights {
            *w = (*w / sum).max(0.0001);
        }
        Self::Linear {
            axis,
            children: flat,
            shares: weights,
        }
    }
    fn contains(&self, id: usize) -> bool {
        match self {
            Self::Panel(p) => *p == id,
            Self::Linear { children, .. } => children.iter().any(|c| c.contains(id)),
        }
    }
    fn ids(&self, out: &mut Vec<usize>) {
        match self {
            Self::Panel(id) => out.push(*id),
            Self::Linear { children, .. } => {
                for c in children {
                    c.ids(out);
                }
            }
        }
    }
    fn remove(self, id: usize) -> Option<Self> {
        match self {
            Self::Panel(p) => {
                if p == id {
                    None
                } else {
                    Some(Self::Panel(p))
                }
            }
            Self::Linear {
                axis,
                children,
                shares,
            } => {
                let mut kept = vec![];
                let mut weights = vec![];
                for (c, w) in children.into_iter().zip(shares) {
                    if let Some(c) = c.remove(id) {
                        kept.push(c);
                        weights.push(w);
                    }
                }
                if kept.is_empty() {
                    None
                } else {
                    Some(Self::weighted(axis, kept, weights))
                }
            }
        }
    }
    fn insert(
        &mut self,
        target: usize,
        panel: usize,
        axis: Axis,
        before: bool,
        peer: bool,
    ) -> bool {
        match self {
            Self::Panel(id) if *id == target => {
                let pair = if before {
                    vec![Self::Panel(panel), self.clone()]
                } else {
                    vec![self.clone(), Self::Panel(panel)]
                };
                *self = Self::linear(axis, pair);
                true
            }
            Self::Linear {
                axis: own,
                children,
                shares,
            } => {
                if peer
                    && *own == axis
                    && let Some(index) = children
                        .iter()
                        .position(|c| matches!(c,Self::Panel(id) if *id==target))
                {
                    children.insert(index + usize::from(!before), Self::Panel(panel));
                    *shares = vec![1.0 / children.len() as f32; children.len()];
                    return true;
                }
                children
                    .iter_mut()
                    .any(|c| c.insert(target, panel, axis, before, peer))
            }
            _ => false,
        }
    }
    fn normalize(self) -> Self {
        match self {
            Self::Panel(_) => self,
            Self::Linear {
                axis,
                children,
                shares,
            } => Self::weighted(
                axis,
                children.into_iter().map(Self::normalize).collect(),
                shares,
            ),
        }
    }
    #[cfg(test)]
    fn from_configuration(c: Configuration<usize>) -> Self {
        match c {
            Configuration::Pane(id) => Self::Panel(id),
            Configuration::Split { axis, ratio, a, b } => Self::weighted(
                axis,
                vec![Self::from_configuration(*a), Self::from_configuration(*b)],
                vec![ratio, 1.0 - ratio],
            ),
        }
    }
}
#[derive(Clone, Debug)]
pub struct Layout {
    root: Container,
    rendered: pane_grid::State<usize>,
    bridges: BTreeMap<Split, (Vec<usize>, usize)>,
    size: Size,
}
impl Deref for Layout {
    type Target = pane_grid::State<usize>;
    fn deref(&self) -> &Self::Target {
        &self.rendered
    }
}
impl Layout {
    /// Structural changes animate; split-ratio changes follow the pointer.
    pub fn motion_topology(&self) -> String {
        fn visit(node: &Container, out: &mut String) {
            match node {
                Container::Panel(id) => out.push_str(&format!("P{id}")),
                Container::Linear { axis, children, .. } => {
                    out.push_str(&format!("{axis:?}["));
                    for child in children {
                        visit(child, out);
                        out.push(',');
                    }
                    out.push(']');
                }
            }
        }
        let mut result = String::new();
        visit(&self.root, &mut result);
        result
    }

    pub fn new(root: Container) -> Self {
        let mut this = Self {
            root,
            rendered: pane_grid::State::new(0).0,
            bridges: BTreeMap::new(),
            size: Size::new(1200., 900.),
        };
        this.compile();
        this
    }
    #[cfg(test)]
    pub fn with_configuration(c: Configuration<usize>) -> Self {
        Self::new(Container::from_configuration(c))
    }
    pub fn saved_root(&self) -> Container {
        self.root.clone()
    }
    pub fn restore(root: Container, count: usize) -> Option<Self> {
        fn valid(node: &Container, depth: usize, ids: &mut Vec<usize>) -> bool {
            if depth > 8 {
                return false;
            }
            match node {
                Container::Panel(id) => {
                    ids.push(*id);
                    ids.len() <= 4
                }
                Container::Linear {
                    children, shares, ..
                } => {
                    children.len() >= 2
                        && children.len() <= 4
                        && children.len() == shares.len()
                        && shares.iter().all(|w| w.is_finite() && *w > 0. && *w <= 1e6)
                        && children.iter().all(|c| valid(c, depth + 1, ids))
                }
            }
        }
        let mut ids = vec![];
        if !(1..=4).contains(&count) || !valid(&root, 0, &mut ids) {
            return None;
        }
        ids.sort_unstable();
        if ids != (0..count).collect::<Vec<_>>() {
            return None;
        }
        Some(Self::new(root.normalize()))
    }
    pub fn preset(count: usize, stacked: bool, grid: bool, active: usize) -> Self {
        use Container::Panel as P;
        let list = Container::linear;
        let root = if count == 1 {
            P(active)
        } else if grid && count >= 3 {
            let left = if count == 3 {
                P(0)
            } else {
                list(Axis::Horizontal, vec![P(0), P(2)])
            };
            list(
                Axis::Vertical,
                vec![
                    left,
                    list(
                        Axis::Horizontal,
                        vec![P(1), P(if count == 3 { 2 } else { 3 })],
                    ),
                ],
            )
        } else {
            list(
                if stacked {
                    Axis::Horizontal
                } else {
                    Axis::Vertical
                },
                (0..count).map(P).collect(),
            )
        };
        Self::new(root)
    }
    pub fn minimum_width(&self) -> f32 {
        sk_iced::components::split::split_minimum_width(
            &self.root,
            crate::workspace::MIN_PANEL_WIDTH,
            GAP,
        )
    }

    /// Workspace scrollbars reserve six DIP plus four DIP spacing when overflowing.
    pub fn canvas_size(&self, viewport: Size) -> Size {
        let width = viewport.width.max(self.minimum_width().ceil());
        Size::new(
            width,
            (viewport.height - if width > viewport.width { 10.0 } else { 0.0 }).max(1.0),
        )
    }
    pub fn set_size(&mut self, size: Size) {
        if size.width > 0. && size.height > 0. && self.size != size {
            self.size = size;
            self.compile();
        }
    }
    fn compile(&mut self) {
        let mut panel_ids = vec![];
        self.root.ids(&mut panel_ids);
        let count = panel_ids.len();
        panel_ids.sort_unstable();
        panel_ids.dedup();
        debug_assert_eq!(panel_ids.len(), count, "duplicate panel in layout");
        debug_assert!((1..=4).contains(&count));
        let old: BTreeMap<usize, Pane> = self.rendered.iter().map(|(p, id)| (*id, *p)).collect();
        let mut spec = vec![];
        let mut rendered =
            pane_grid::State::with_configuration(sk_iced::components::split::split_configuration(
                &self.root,
                self.size,
                GAP,
                &[],
                &mut spec,
            ));
        // Preserve pane keys across topology changes so Iced retains widget-local state.
        let generated = rendered
            .iter()
            .map(|(p, id)| (*id, *p))
            .collect::<BTreeMap<_, _>>();
        let mut used = std::collections::BTreeSet::new();
        let mut desired = BTreeMap::new();
        for id in generated.keys() {
            if let Some(p) = old.get(id) {
                desired.insert(*id, *p);
                used.insert(*p);
            }
        }
        for id in generated.keys() {
            if !desired.contains_key(id) {
                let key = *generated.values().find(|p| !used.contains(p)).unwrap();
                desired.insert(*id, key);
                used.insert(key);
            }
        }
        let mut current = generated;
        for (id, key) in &desired {
            let at = current[id];
            if at != *key {
                rendered.swap(at, *key);
                for p in current.values_mut() {
                    if *p == at {
                        *p = *key;
                    } else if *p == *key {
                        *p = at;
                    }
                }
            }
        }
        rendered.panes = desired.into_iter().map(|(id, p)| (p, id)).collect();
        fn splits(node: &Node, out: &mut Vec<Split>) {
            if let Node::Split { id, a, b, .. } = node {
                out.push(*id);
                splits(a, out);
                splits(b, out);
            }
        }
        let mut ids = vec![];
        splits(rendered.layout(), &mut ids);
        self.bridges = ids.into_iter().zip(spec).collect();
        self.rendered = rendered;
    }
    pub fn move_panel(&mut self, source: usize, zone: crate::docking::Zone) -> bool {
        if !zone.valid()
            || !self.root.contains(source)
            || self.len() < 2
            || (!zone.workspace()
                && (source == zone.target() || !self.root.contains(zone.target())))
        {
            return false;
        }
        let root = self.root.clone().remove(source).unwrap();
        let Some(root) = Self::place(root, source, zone) else {
            return false;
        };
        self.root = root.normalize();
        self.compile();
        true
    }
    fn place(mut root: Container, source: usize, zone: crate::docking::Zone) -> Option<Container> {
        let axis = if matches!(zone.edge(), 1 | 7) {
            Axis::Horizontal
        } else {
            Axis::Vertical
        };
        let before = matches!(zone.edge(), 1 | 3);
        if zone.workspace() {
            if let Container::Linear {
                axis: own,
                children,
                shares,
            } = &mut root
                && *own == axis
            {
                let index = if before { 0 } else { children.len() };
                for w in shares.iter_mut() {
                    *w *= 0.5;
                }
                children.insert(index, Container::Panel(source));
                shares.insert(index, 0.5);
                if zone.balances_rows() {
                    *shares = vec![1.0 / children.len() as f32; children.len()];
                }
            } else {
                let children = if before {
                    vec![Container::Panel(source), root]
                } else {
                    vec![root, Container::Panel(source)]
                };
                root = Container::linear(axis, children);
            }
        } else if !root.insert(zone.target(), source, axis, before, true) {
            return None;
        }
        Some(root.normalize())
    }
    // Adding a panel is explicit and only used for tab tear-off. Moves cannot call this.
    pub fn add_panel(&mut self, id: usize, zone: crate::docking::Zone) -> bool {
        if self.len() >= 4
            || self.root.contains(id)
            || !zone.valid()
            || (!zone.workspace() && !self.root.contains(zone.target()))
        {
            return false;
        }
        let Some(root) = Self::place(self.root.clone(), id, zone) else {
            return false;
        };
        self.root = root;
        self.compile();
        true
    }
    pub fn close(&mut self, pane: Pane) -> Option<()> {
        let id = *self.rendered.get(pane)?;
        if self.len() < 2 {
            return None;
        }
        self.root = self.root.clone().remove(id)?;
        self.compile();
        Some(())
    }
    pub fn renumber_after(&mut self, closed: usize) {
        fn renumber(node: &mut Container, closed: usize) {
            match node {
                Container::Panel(id) => {
                    if *id > closed {
                        *id -= 1;
                    }
                }
                Container::Linear { children, .. } => {
                    for c in children {
                        renumber(c, closed);
                    }
                }
            }
        }
        renumber(&mut self.root, closed);
        // IDs change, but the surviving widget keys must not.
        for id in self.rendered.panes.values_mut() {
            if *id > closed {
                *id -= 1;
            }
        }
    }
    pub fn resize(&mut self, split: Split, ratio: f32) {
        let Some((path, index)) = self.bridges.get(&split).cloned() else {
            return;
        };
        let regions = self.rendered.layout().split_regions(GAP, 180.0, self.size);
        let Some((axis, bounds, _)) = regions.get(&split) else {
            return;
        };
        let extent = if *axis == Axis::Horizontal {
            bounds.height
        } else {
            bounds.width
        };
        let mut node = &mut self.root;
        for index in path {
            let Container::Linear { children, .. } = node else {
                return;
            };
            node = &mut children[index];
        }
        let Container::Linear { shares, .. } = node else {
            return;
        };
        sk_iced::components::split::resize_split_shares(shares, index, extent, ratio, GAP);
        self.compile();
    }
}
pub trait Snapshot {
    fn snapshot(&self) -> Layout;
}
impl Snapshot for Layout {
    fn snapshot(&self) -> Layout {
        self.clone()
    }
}
#[cfg(test)]
impl Snapshot for pane_grid::State<usize> {
    fn snapshot(&self) -> Layout {
        fn config(node: &Node, state: &pane_grid::State<usize>) -> Configuration<usize> {
            match node {
                Node::Pane(p) => Configuration::Pane(*state.get(*p).unwrap()),
                Node::Split {
                    axis, ratio, a, b, ..
                } => Configuration::Split {
                    axis: *axis,
                    ratio: *ratio,
                    a: Box::new(config(a, state)),
                    b: Box::new(config(b, state)),
                },
            }
        }
        Layout::with_configuration(config(self.layout(), self))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::docking::Zone;
    fn keys(layout: &Layout) -> BTreeMap<usize, Pane> {
        layout.iter().map(|(p, id)| (*id, *p)).collect()
    }
    fn check(node: &Container) {
        if let Container::Linear {
            axis,
            children,
            shares,
        } = node
        {
            assert!(children.len() >= 2);
            assert_eq!(children.len(), shares.len());
            assert!(shares.iter().all(|s| s.is_finite() && *s > 0.));
            for child in children {
                assert!(!matches!(child,Container::Linear{axis:own,..} if own==axis));
                check(child);
            }
        }
    }
    #[test]
    fn repeated_docking_keeps_exact_panel_set_and_widget_keys() {
        for count in 2..=4 {
            let mut layout = Layout::preset(count, true, false, 0);
            let original = keys(&layout);
            let mut seed = 971_u64;
            for _ in 0..1000 {
                seed = seed.wrapping_mul(6364136223846793005).wrapping_add(1);
                let source = ((seed >> 16) as usize) % count;
                let target = ((seed >> 24) as usize) % count;
                let edge = [1, 3, 5, 7][((seed >> 32) as usize) % 4];
                let zone = match (seed >> 40) % 3 {
                    0 => Zone(edge).for_workspace(),
                    1 => Zone(edge).for_panel(target),
                    _ => Zone(if edge == 3 {
                        1
                    } else if edge == 5 {
                        7
                    } else {
                        edge
                    })
                    .for_panel(target),
                };
                layout.move_panel(source, zone);
                let size = Size::new(1600_f32.max(layout.minimum_width().ceil()), 1000.);
                layout.set_size(size);
                assert_eq!(keys(&layout), original);
                assert_eq!(layout.len(), count);
                check(&layout.root);
                let regions = layout.layout().pane_regions(6., 180., size);
                for (pane, rect) in &regions {
                    assert!(rect.width >= 319. && rect.height >= 180., "{rect:?}");
                    for (other, next) in &regions {
                        if pane != other {
                            assert!(rect.intersection(next).is_none());
                        }
                    }
                }
            }
        }
    }
    #[test]
    fn row_insertion_is_a_flat_container_with_equal_shares() {
        let mut layout = Layout::preset(4, true, false, 0);
        assert!(layout.move_panel(0, Zone(1).for_panel(2)));
        let Container::Linear {
            axis,
            children,
            shares,
        } = &layout.root
        else {
            panic!("row container")
        };
        assert_eq!(*axis, Axis::Horizontal);
        assert_eq!(children.len(), 4);
        assert_eq!(
            *children,
            vec![
                Container::Panel(1),
                Container::Panel(0),
                Container::Panel(2),
                Container::Panel(3)
            ]
        );
        assert!(shares.iter().all(|s| (*s - 0.25).abs() < 0.0001));
        for height in [800., 900., 1100.] {
            let size = Size::new(1200., height);
            layout.set_size(size);
            let regions = layout.layout().pane_regions(6., 180., size);
            let expected = (height - 18.) / 4.;
            assert!(regions.values().all(|r| (r.height - expected).abs() <= 1.));
        }
    }
    #[test]
    fn resizing_updates_container_shares_and_survives_recompilation() {
        let mut layout = Layout::preset(3, false, false, 0);
        let size = Size::new(1800., 900.);
        layout.set_size(size);
        let split = *layout.layout().splits().next().unwrap();
        layout.resize(split, 0.4);
        let regions = layout.layout().pane_regions(6., 180., size);
        let first = keys(&layout)[&0];
        assert!((regions[&first].width - 717.).abs() <= 1.);
        let root = layout.root.clone();
        layout.set_size(Size::new(2000., 950.));
        assert_eq!(layout.root, root);
        layout.set_size(size);
        assert_eq!(layout.layout().pane_regions(6., 180., size), regions);
    }
    #[test]
    fn adding_and_closing_preserve_surviving_renderer_keys() {
        let mut layout = Layout::preset(2, true, false, 0);
        let original = keys(&layout);
        assert!(layout.add_panel(2, Zone(5).for_workspace()));
        for (id, pane) in &original {
            assert_eq!(keys(&layout)[id], *pane);
        }
        assert!(layout.add_panel(3, Zone(7).for_panel(2)));
        assert!(!layout.add_panel(4, Zone(3).for_workspace()));
        let before = keys(&layout);
        layout.close(before[&1]);
        layout.renumber_after(1);
        let after = keys(&layout);
        assert_eq!(after[&0], before[&0]);
        assert_eq!(after[&1], before[&2]);
        assert_eq!(after[&2], before[&3]);
        layout.set_size(Size::new(1600., 1000.));
        assert_eq!(keys(&layout), after);
        check(&layout.root);
    }
    #[test]
    fn invalid_moves_are_atomic_and_single_panel_cannot_expand() {
        let mut layout = Layout::preset(1, true, false, 0);
        let original = layout.root.clone();
        for zone in [
            Zone(3).for_workspace(),
            Zone(7).for_panel(0),
            Zone(4),
            Zone(1).for_panel(3),
        ] {
            assert!(!layout.move_panel(0, zone));
            assert_eq!(layout.root, original);
            assert_eq!(layout.len(), 1);
        }
    }
}

impl sk_iced::components::SplitModel for Container {
    type Key = usize;
    fn panel(&self) -> Option<usize> {
        if let Self::Panel(id) = self {
            Some(*id)
        } else {
            None
        }
    }
    fn branch(&self) -> Option<(Axis, &[Self], &[f32])> {
        if let Self::Linear {
            axis,
            children,
            shares,
        } = self
        {
            Some((*axis, children, shares))
        } else {
            None
        }
    }
}
