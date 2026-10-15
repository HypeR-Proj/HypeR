// SPDX-FileCopyrightText: 2026 roolrz
// SPDX-License-Identifier: Apache-2.0

use crate::*;
use std::collections::{BTreeMap, BTreeSet};

#[derive(Clone, Debug)]
pub struct Instance {
    pub path: String,
    pub parent: Option<usize>,
    pub declaration: Component,
    pub children: BTreeMap<String, usize>,
}

#[derive(Clone, Debug, serde::Serialize, serde::Deserialize)]
pub struct RouteStep {
    pub instance: String,
    pub declaration: String,
    pub name: String,
}

#[derive(Clone, Debug, serde::Serialize, serde::Deserialize)]
pub struct Route {
    pub consumer: usize,
    pub local_name: String,
    pub provider: usize,
    pub capability: String,
    pub kind: CapabilityType,
    pub transfer: Transfer,
    pub purpose: Option<u32>,
    pub steps: Vec<RouteStep>,
}

#[derive(Clone, Debug)]
pub struct Topology {
    pub instances: Vec<Instance>,
    pub routes: Vec<Route>,
}

impl Topology {
    pub fn parse(
        root: &str,
        documents: impl IntoIterator<Item = (String, Vec<u8>)>,
    ) -> Result<Self, String> {
        let mut declarations = BTreeMap::new();
        let mut total = 0usize;
        for (filename, bytes) in documents {
            total = total
                .checked_add(bytes.len())
                .ok_or("configuration too large")?;
            if bytes.len() > MAX_DOCUMENT_BYTES
                || total > MAX_CONFIGURATION_BYTES
                || declarations.len() >= MAX_COMPONENTS
            {
                return Err("component configuration exceeds limits".into());
            }
            let value: Component =
                serde_json::from_slice(&bytes).map_err(|e| format!("{filename}: {e}"))?;
            if filename != format!("{}.json", value.name) {
                return Err(format!("{filename}: declaration name must match filename"));
            }
            validate(&value)?;
            if declarations.insert(value.name.clone(), value).is_some() {
                return Err(format!("duplicate declaration: {filename}"));
            }
        }
        let mut result = Self {
            instances: Vec::new(),
            routes: Vec::new(),
        };
        result.instantiate(root, "/", None, &declarations, &mut Vec::new())?;
        for consumer in 0..result.instances.len() {
            for usage in &result.instances[consumer].declaration.uses {
                if result.routes.len() >= MAX_ROUTES {
                    return Err("too many capability routes".into());
                }
                let mut steps = Vec::new();
                let (provider, cap) = result.resolve_parent(
                    consumer,
                    &usage.name,
                    &usage.kind,
                    &mut BTreeSet::new(),
                    &mut steps,
                )?;
                let route = Route {
                    consumer,
                    local_name: usage.name.clone(),
                    provider,
                    capability: cap.name.clone(),
                    kind: usage.kind.clone(),
                    transfer: cap.transfer,
                    purpose: usage.purpose,
                    steps,
                };
                result.routes.push(route);
            }
        }
        for (index, instance) in result.instances.iter().enumerate() {
            for expose in &instance.declaration.expose {
                result.source(
                    index,
                    &expose.from,
                    &expose.source,
                    &expose.kind,
                    &mut BTreeSet::new(),
                    &mut Vec::new(),
                )?;
            }
            for offer in &instance.declaration.offer {
                result.source(
                    index,
                    &offer.from,
                    &offer.source,
                    &offer.kind,
                    &mut BTreeSet::new(),
                    &mut Vec::new(),
                )?;
            }
        }
        let mut moved = BTreeSet::new();
        for route in &result.routes {
            if route.transfer == Transfer::Move
                && !moved.insert((route.provider, route.capability.clone()))
            {
                return Err(format!(
                    "{}: MOVE capability has multiple consumers",
                    route.capability
                ));
            }
            if route.transfer == Transfer::Move
                && result.instances[route.consumer]
                    .declaration
                    .program
                    .as_ref()
                    .is_some_and(|p| p.restart != Restart::Never)
            {
                return Err(format!(
                    "{}: MOVE grants prohibit automatic restart",
                    result.instances[route.consumer].path
                ));
            }
        }
        Ok(result)
    }

    pub fn index(&self, path: &str) -> Option<usize> {
        self.instances.iter().position(|i| i.path == path)
    }
    pub fn route(&self, consumer: usize, local: &str) -> Option<&Route> {
        self.routes
            .iter()
            .find(|r| r.consumer == consumer && r.local_name == local)
    }

    fn instantiate(
        &mut self,
        name: &str,
        path: &str,
        parent: Option<usize>,
        definitions: &BTreeMap<String, Component>,
        stack: &mut Vec<String>,
    ) -> Result<usize, String> {
        if self.instances.len() >= MAX_COMPONENTS
            || path.len() > 255
            || stack.len() >= MAX_DEPTH
            || stack.iter().any(|s| s == name)
        {
            return Err(format!("{path}: recursive or oversized component tree"));
        }
        let declaration = definitions
            .get(name)
            .ok_or_else(|| format!("{path}: unknown component {name}"))?
            .clone();
        let children = declaration.children.clone();
        let index = self.instances.len();
        self.instances.push(Instance {
            path: path.into(),
            parent,
            declaration,
            children: BTreeMap::new(),
        });
        stack.push(name.into());
        for child in children {
            let child_path = format!("{}/{}", path.trim_end_matches('/'), child.name);
            let next = self.instantiate(
                &child.component,
                &child_path,
                Some(index),
                definitions,
                stack,
            )?;
            self.instances[index].children.insert(child.name, next);
        }
        stack.pop();
        Ok(index)
    }

    fn resolve_parent(
        &self,
        child: usize,
        name: &str,
        kind: &CapabilityType,
        visited: &mut BTreeSet<(usize, String, String)>,
        steps: &mut Vec<RouteStep>,
    ) -> Result<(usize, &Capability), String> {
        let instance = &self.instances[child];
        let parent = instance
            .parent
            .ok_or_else(|| format!("{}: root has no parent offering {name}", instance.path))?;
        let realm = &self.instances[parent];
        let local = realm
            .children
            .iter()
            .find(|(_, i)| **i == child)
            .map(|(n, _)| n.as_str())
            .ok_or("invalid child index")?;
        let offer = realm
            .declaration
            .offer
            .iter()
            .find(|o| o.to == local && o.name == name)
            .ok_or_else(|| format!("{}: missing offer {name} to {local}", realm.path))?;
        if !offer.kind.permits(kind) {
            return Err(format!("{}: incompatible offer {name}", realm.path));
        }
        self.step(parent, "offer", &format!("{local}/{name}"), visited, steps)?;
        self.source(
            parent,
            &offer.from,
            &offer.source,
            &offer.kind,
            visited,
            steps,
        )
    }

    fn source(
        &self,
        index: usize,
        from: &str,
        name: &str,
        kind: &CapabilityType,
        visited: &mut BTreeSet<(usize, String, String)>,
        steps: &mut Vec<RouteStep>,
    ) -> Result<(usize, &Capability), String> {
        let instance = &self.instances[index];
        match from {
            "self" => {
                let cap = instance
                    .declaration
                    .capabilities
                    .iter()
                    .find(|c| c.name == name)
                    .ok_or_else(|| {
                        format!("{}: capability {name} is not declared", instance.path)
                    })?;
                if !cap.kind.permits(kind) {
                    return Err(format!("{}: incompatible capability {name}", instance.path));
                }
                self.step(index, "capability", name, visited, steps)?;
                Ok((index, cap))
            }
            "parent" => self.resolve_parent(index, name, kind, visited, steps),
            _ => {
                let child = from
                    .strip_prefix('#')
                    .and_then(|n| instance.children.get(n))
                    .copied()
                    .ok_or_else(|| format!("{}: invalid source {from}", instance.path))?;
                let declaration = &self.instances[child].declaration;
                let exposed = declaration
                    .expose
                    .iter()
                    .find(|e| e.name == name)
                    .ok_or_else(|| {
                        format!("{}: missing expose {name}", self.instances[child].path)
                    })?;
                if !exposed.kind.permits(kind) {
                    return Err(format!(
                        "{}: incompatible expose {name}",
                        self.instances[child].path
                    ));
                }
                self.step(child, "expose", name, visited, steps)?;
                self.source(
                    child,
                    &exposed.from,
                    &exposed.source,
                    &exposed.kind,
                    visited,
                    steps,
                )
            }
        }
    }

    fn step(
        &self,
        index: usize,
        declaration: &str,
        name: &str,
        visited: &mut BTreeSet<(usize, String, String)>,
        steps: &mut Vec<RouteStep>,
    ) -> Result<(), String> {
        if !visited.insert((index, declaration.into(), name.into())) || steps.len() >= MAX_ROUTES {
            return Err(format!(
                "{}: capability routing cycle at {name}",
                self.instances[index].path
            ));
        }
        steps.push(RouteStep {
            instance: self.instances[index].path.clone(),
            declaration: declaration.into(),
            name: name.into(),
        });
        Ok(())
    }
}

fn unique<'a>(names: impl IntoIterator<Item = &'a str>) -> Result<(), String> {
    let mut seen = BTreeSet::new();
    for name in names {
        if !valid_name(name) || !seen.insert(name) {
            return Err(format!("invalid or repeated name {name}"));
        }
    }
    Ok(())
}

fn kind(value: &CapabilityType) -> bool {
    match value {
        CapabilityType::Protocol { protocol } => valid_name(protocol),
        CapabilityType::Object {
            object_type,
            rights,
        } => *object_type != 0 && *rights != 0,
    }
}

fn validate(c: &Component) -> Result<(), String> {
    if !valid_name(&c.name)
        || c.description.len() > 256
        || c.children.len() > MAX_COMPONENTS
        || c.uses.len() > 32
        || c.capabilities.len() > 64
        || c.offer.len() > MAX_ROUTES
        || c.expose.len() > 64
    {
        return Err(format!("{}: invalid declaration limits", c.name));
    }
    unique(c.children.iter().map(|v| v.name.as_str()))?;
    unique(c.uses.iter().map(|v| v.name.as_str()))?;
    unique(c.capabilities.iter().map(|v| v.name.as_str()))?;
    unique(c.expose.iter().map(|v| v.name.as_str()))?;
    let mut purposes = BTreeSet::new();
    for u in &c.uses {
        if !kind(&u.kind)
            || match (&u.kind, u.purpose) {
                (CapabilityType::Protocol { .. }, None) => false,
                (CapabilityType::Object { .. }, Some(p))
                    if p != 0
                        && !matches!(
                            p,
                            4 | 5 | 10 | 15 | 0x8003_0001..=0x8003_0003
                                | 0x8003_0011..=0x8003_0013 | 0x800c_0001 | 0x800c_0002 | 0x800c_0020
                        ) =>
                {
                    !purposes.insert(p)
                }
                _ => true,
            }
        {
            return Err(format!("{}: invalid use {}", c.name, u.name));
        }
        if matches!(u.kind, CapabilityType::Object { .. })
            && c.program.as_ref().is_some_and(|p| p.prepared)
        {
            return Err(format!(
                "{}: prepared object grants belong to its construction transaction",
                c.name
            ));
        }
    }
    let source_valid = |source: &str| {
        source == "self"
            || source == "parent"
            || source
                .strip_prefix('#')
                .is_some_and(|name| c.children.iter().any(|child| child.name == name))
    };
    let mut offers = BTreeSet::new();
    for o in &c.offer {
        if !c.children.iter().any(|child| child.name == o.to)
            || !valid_name(&o.name)
            || !valid_name(&o.source)
            || !source_valid(&o.from)
            || !kind(&o.kind)
            || !offers.insert((&o.to, &o.name))
        {
            return Err(format!("{}: invalid offer {}", c.name, o.name));
        }
    }
    for e in &c.expose {
        if !source_valid(&e.from) || !valid_name(&e.source) || !kind(&e.kind) {
            return Err(format!("{}: invalid expose {}", c.name, e.name));
        }
    }
    for cap in &c.capabilities {
        if (matches!(cap.kind, CapabilityType::Protocol { .. }) && c.program.is_none())
            || (matches!(cap.kind, CapabilityType::Object { .. }) && c.program.is_some())
            || !kind(&cap.kind)
            || (cap.transfer == Transfer::Move
                && matches!(cap.kind, CapabilityType::Protocol { .. }))
        {
            return Err(format!("{}: invalid capability {}", c.name, cap.name));
        }
    }
    if let Some(p) = &c.program
        && (!absolute_path(&p.image)
            || p.arguments.len() > 16
            || p.arguments
                .iter()
                .any(|a| a.len() > 256 || a.as_bytes().contains(&0))
            || p.timeout_ms == 0
            || p.timeout_ms > 300_000
            || p.restart_limit > 10
            || !(100..=60_000).contains(&p.restart_delay_ms)
            || (p.prepared && p.restart != Restart::Never))
    {
        return Err(format!("{}: invalid program", c.name));
    }
    Ok(())
}
