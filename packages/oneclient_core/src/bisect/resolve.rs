use std::collections::{BTreeMap, BTreeSet, HashMap};

use super::meta::{ClassIndex, ModMeta, Ranges};
use super::version::{Predicate, Version};

pub type Set = BTreeSet<String>;

const IMPLICIT: [&str; 10] = [
    "java",
    "minecraft",
    "fabricloader",
    "fabric-loader",
    "quilt_loader",
    "neoforge",
    "forge",
    "fml",
    "javafml",
    "mixinextras",
];

#[must_use]
pub fn is_implicit(id: &str) -> bool {
    IMPLICIT.contains(&id)
}

#[derive(Debug, Clone)]
struct Provider {
    hash: String,
    version: Version,
    direct: bool,
}

#[derive(Debug, Default)]
pub struct Catalog {
    pub mods: BTreeMap<String, ModMeta>,
    providers: HashMap<String, Vec<Provider>>,
    extra: BTreeMap<String, Set>,
}

#[derive(Debug, Clone, Default)]
pub struct Resolution {
    pub set: Set,
    pub problem: Option<String>,
}

impl Catalog {
    #[must_use]
    pub fn new(mods: BTreeMap<String, ModMeta>, extra: BTreeMap<String, Set>) -> Self {
        let mut providers: HashMap<String, Vec<Provider>> = HashMap::new();
        for (hash, meta) in &mods {
            let mut add = |id: &str, version: &Version, direct: bool| {
                if !id.is_empty() {
                    providers.entry(id.to_string()).or_default().push(Provider {
                        hash: hash.clone(),
                        version: version.clone(),
                        direct,
                    });
                }
            };
            add(&meta.id, &meta.version, true);
            for id in &meta.provides {
                add(id, &meta.version, true);
            }
            for nested in &meta.nested {
                add(&nested.id, &nested.version, false);
                for id in &nested.provides {
                    add(id, &nested.version, false);
                }
            }
        }
        for list in providers.values_mut() {
            list.sort_by(|a, b| {
                b.direct
                    .cmp(&a.direct)
                    .then_with(|| b.version.compare(&a.version))
                    .then_with(|| a.hash.cmp(&b.hash))
            });
        }
        Self {
            mods,
            providers,
            extra,
        }
    }

    #[must_use]
    pub fn ids_of(&self, hash: &str) -> Vec<String> {
        let Some(meta) = self.mods.get(hash) else {
            return Vec::new();
        };
        let mut ids = vec![meta.id.clone()];
        ids.extend(meta.provides.iter().cloned());
        for nested in &meta.nested {
            ids.push(nested.id.clone());
            ids.extend(nested.provides.iter().cloned());
        }
        ids
    }

    #[must_use]
    pub fn provider_of(&self, id: &str, among: &Set) -> Option<String> {
        self.providers
            .get(id)?
            .iter()
            .find(|p| among.contains(&p.hash))
            .map(|p| p.hash.clone())
    }

    #[must_use]
    pub fn resolve(&self, targets: &Set, forced: &Set, activatable: &Set) -> Resolution {
        let mut strict = Session::new(self, activatable, false);
        let initial: Set = targets.union(forced).cloned().collect();
        for hash in &initial {
            if strict.failed.is_some() {
                break;
            }
            strict.ensure(hash);
        }
        if strict.failed.is_none() {
            strict.check_breaks();
        }
        if strict.failed.is_none() {
            return Resolution {
                set: strict.active,
                problem: None,
            };
        }

        let problem = strict.failed.take();
        let mut lenient = Session::new(self, activatable, true);
        for hash in &initial {
            lenient.ensure(hash);
        }
        Resolution {
            set: lenient.active,
            problem,
        }
    }

    fn satisfiable(&self, dep: &str, predicates: &[Predicate], available: &Set) -> bool {
        self.providers.get(dep).is_some_and(|list| {
            list.iter().any(|p| {
                available.contains(&p.hash) && predicates.iter().any(|pr| pr.test(&p.version))
            })
        })
    }

    #[must_use]
    pub fn directly_unresolvable(
        &self,
        available: &Set,
        with_extra: bool,
    ) -> BTreeMap<String, Vec<String>> {
        let mut out = BTreeMap::new();
        for hash in available {
            let Some(meta) = self.mods.get(hash) else {
                continue;
            };
            let mut failed: Vec<String> = meta
                .depends
                .iter()
                .chain(meta.nested.iter().flat_map(|nested| &nested.depends))
                .filter(|(dep, _)| !is_implicit(dep))
                .filter(|(dep, predicates)| !self.satisfiable(dep, predicates, available))
                .map(|(dep, _)| dep.clone())
                .collect();
            if with_extra {
                for target in self.extra.get(hash).into_iter().flatten() {
                    if !available.contains(target) {
                        failed.push(
                            self.mods
                                .get(target)
                                .map_or_else(|| target.clone(), |m| m.id.clone()),
                        );
                    }
                }
            }
            if !failed.is_empty() {
                out.insert(hash.clone(), failed);
            }
        }
        out
    }

    #[must_use]
    pub fn transitively_unresolvable(&self, available: &Set, with_extra: bool) -> Set {
        let mut available = available.clone();
        let mut total = Set::new();
        loop {
            let found = self.directly_unresolvable(&available, with_extra);
            if found.is_empty() {
                return total;
            }
            for hash in found.into_keys() {
                available.remove(&hash);
                total.insert(hash);
            }
        }
    }

    #[must_use]
    pub fn unresolvable_details(&self, available: &Set) -> Unresolvable {
        let direct = self.directly_unresolvable(available, false);
        let mut available = available.clone();
        let mut roots: BTreeMap<String, Set> = BTreeMap::new();
        for hash in direct.keys() {
            available.remove(hash);
            roots.insert(hash.clone(), Set::from([hash.clone()]));
        }
        let mut transitive: BTreeMap<String, Set> = BTreeMap::new();
        loop {
            let found = self.directly_unresolvable(&available, false);
            if found.is_empty() {
                break;
            }
            for (hash, deps) in found {
                let mut causes = Set::new();
                for dep in deps {
                    for provider in self.providers.get(&dep).into_iter().flatten() {
                        if let Some(root) = roots.get(&provider.hash) {
                            causes.extend(root.iter().cloned());
                        }
                    }
                }
                if causes.is_empty() {
                    causes.insert(hash.clone());
                }
                roots.insert(hash.clone(), causes.clone());
                transitive.insert(hash.clone(), causes);
                available.remove(&hash);
            }
        }
        Unresolvable { direct, transitive }
    }

    #[must_use]
    pub fn has_extra(&self) -> bool {
        self.extra.values().any(|targets| !targets.is_empty())
    }
}

#[derive(Debug, Clone, Default)]
pub struct Unresolvable {
    pub direct: BTreeMap<String, Vec<String>>,
    pub transitive: BTreeMap<String, Set>,
}

impl Unresolvable {
    #[must_use]
    pub fn all(&self) -> Set {
        self.direct
            .keys()
            .chain(self.transitive.keys())
            .cloned()
            .collect()
    }
}

struct Session<'a> {
    catalog: &'a Catalog,
    activatable: &'a Set,
    lenient: bool,
    active: Set,
    stack: Set,
    chosen: HashMap<String, Provider>,
    dead: Set,
    failed: Option<String>,
}

impl<'a> Session<'a> {
    fn new(catalog: &'a Catalog, activatable: &'a Set, lenient: bool) -> Self {
        Self {
            catalog,
            activatable,
            lenient,
            active: Set::new(),
            stack: Set::new(),
            chosen: HashMap::new(),
            dead: Set::new(),
            failed: None,
        }
    }

    fn name(&self, hash: &str) -> String {
        self.catalog
            .mods
            .get(hash)
            .map_or_else(|| hash.to_string(), |m| m.id.clone())
    }

    fn ensure(&mut self, hash: &str) -> bool {
        if self.failed.is_some() {
            return false;
        }
        if self.active.contains(hash) || self.stack.contains(hash) {
            return true;
        }
        if !self.activatable.contains(hash) {
            return false;
        }
        let Some(meta) = self.catalog.mods.get(hash) else {
            return false;
        };

        self.stack.insert(hash.to_string());
        let before = self.active.clone();
        self.active.insert(hash.to_string());

        let mut ok = self.dependencies(hash, &meta.depends);
        if ok {
            let extra: Vec<String> = self
                .catalog
                .extra
                .get(hash)
                .into_iter()
                .flatten()
                .cloned()
                .collect();
            for target in extra {
                self.ensure(&target);
            }
        }
        if ok {
            for nested in &meta.nested {
                if !self.dependencies(&nested.id, &nested.depends) {
                    ok = false;
                    break;
                }
            }
        }
        self.stack.remove(hash);

        if !ok {
            self.active = before;
        }
        ok
    }

    fn dependencies(&mut self, requester: &str, deps: &Ranges) -> bool {
        for (dep, predicates) in deps {
            if is_implicit(dep) {
                continue;
            }
            if !self.dependency(dep, predicates, requester) {
                return false;
            }
        }
        true
    }

    fn dependency(&mut self, dep: &str, predicates: &[Predicate], requester: &str) -> bool {
        if self.dead.contains(dep) {
            return self.lenient;
        }
        if let Some(chosen) = self.chosen.get(dep).cloned() {
            if !self.lenient && !predicates.iter().any(|p| p.test(&chosen.version)) {
                self.failed = Some(format!(
                    "{} needs {dep} {}, but {} {} was already picked",
                    self.name(requester),
                    join(predicates),
                    self.name(&chosen.hash),
                    chosen.version
                ));
                return false;
            }
            return self.ensure(&chosen.hash);
        }

        let candidates: Vec<Provider> = self
            .catalog
            .providers
            .get(dep)
            .into_iter()
            .flatten()
            .filter(|p| self.activatable.contains(&p.hash))
            .filter(|p| self.lenient || predicates.iter().any(|pr| pr.test(&p.version)))
            .cloned()
            .collect();
        let mut ordered = candidates;
        let on = |p: &Provider| self.active.contains(&p.hash) || self.stack.contains(&p.hash);
        ordered.sort_by(|a, b| {
            on(b)
                .cmp(&on(a))
                .then_with(|| b.version.compare(&a.version))
                .then_with(|| b.direct.cmp(&a.direct))
                .then_with(|| a.hash.cmp(&b.hash))
        });

        let before = self.active.clone();
        for provider in ordered {
            if self.ensure(&provider.hash) {
                self.chosen.insert(dep.to_string(), provider);
                return true;
            }
            if self.failed.is_some() {
                return false;
            }
            self.active = before.clone();
        }
        self.dead.insert(dep.to_string());
        self.lenient
    }

    fn check_breaks(&mut self) {
        let mut versions: HashMap<String, (String, Version)> = HashMap::new();
        for hash in &self.active {
            let Some(meta) = self.catalog.mods.get(hash) else {
                continue;
            };
            let mut add = |id: &String, version: &Version| {
                versions
                    .entry(id.clone())
                    .or_insert_with(|| (hash.clone(), version.clone()));
            };
            add(&meta.id, &meta.version);
            for id in &meta.provides {
                add(id, &meta.version);
            }
            for nested in &meta.nested {
                add(&nested.id, &nested.version);
            }
        }
        for hash in &self.active {
            let Some(meta) = self.catalog.mods.get(hash) else {
                continue;
            };
            for (dep, predicates) in &meta.breaks {
                if let Some((other, version)) = versions.get(dep)
                    && other != hash
                    && predicates
                        .iter()
                        .any(|p| p.is_any() || (*version != Version::Unknown && p.test(version)))
                {
                    self.failed =
                        Some(format!("{} breaks {} {version}", meta.id, self.name(other)));
                    return;
                }
            }
        }
    }
}

fn join(predicates: &[Predicate]) -> String {
    predicates
        .iter()
        .map(ToString::to_string)
        .collect::<Vec<_>>()
        .join(" || ")
}

const VENDORED: [&str; 44] = [
    "com/azure/json/",
    "com/electronwill/nightconfig/",
    "com/google/",
    "com/ibm/icu/",
    "com/jcraft/",
    "com/llamalad7/mixinextras/platform/forge/",
    "com/microsoft/aad/",
    "com/mojang/",
    "com/sun/jna/",
    "cpw/mods/",
    "gnu/trove/",
    "io/github/zekerzhayard/",
    "io/netty/",
    "it/unimi/dsi/",
    "javax/",
    "joptsimple/",
    "net/fabricmc/api/",
    "net/fabricmc/loader/",
    "net/java/games/",
    "net/jpountz/",
    "net/jodah/typetools/",
    "net/minecraft/",
    "net/minecraftforge/",
    "net/minecrell/terminalconsole/",
    "net/neoforged/",
    "org/apache/",
    "org/codehaus/plexus/util/",
    "org/jspecify/annotations/",
    "org/jline/",
    "org/joml/",
    "org/lwjgl/",
    "org/objectweb/asm/",
    "org/prismlauncher/",
    "org/quiltmc/config/",
    "org/quiltmc/json5/",
    "org/quiltmc/loader/",
    "org/slf4j/",
    "org/spongepowered/",
    "oshi/",
    "paulscode/sound/",
    "tv/twitch/",
    "org/jetbrains/annotations/",
    "org/intellij/lang/annotations/",
    "kotlin/",
];

fn vendored(class: &str) -> bool {
    VENDORED.iter().any(|prefix| class.starts_with(prefix))
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Inferred {
    pub source: String,
    pub target: String,
    pub classes: Vec<String>,
}

#[must_use]
pub fn infer(catalog: &Catalog, indexes: &BTreeMap<String, ClassIndex>) -> Vec<Inferred> {
    let mut owners: HashMap<&str, Vec<&str>> = HashMap::new();
    for (hash, index) in indexes {
        for class in &index.declared {
            if !vendored(class) {
                owners
                    .entry(class.as_str())
                    .or_default()
                    .push(hash.as_str());
            }
        }
    }

    let mut out = Vec::new();
    for (source, index) in indexes {
        let mut by_target: BTreeMap<&str, Vec<String>> = BTreeMap::new();
        for class in &index.referenced {
            if vendored(class) {
                continue;
            }
            if let Some([target]) = owners.get(class.as_str()).map(Vec::as_slice)
                && *target != source.as_str()
            {
                by_target.entry(target).or_default().push(class.clone());
            }
        }
        let Some(source_meta) = catalog.mods.get(source) else {
            continue;
        };
        for (target, classes) in by_target {
            let already = catalog
                .ids_of(target)
                .iter()
                .any(|id| source_meta.mentions(id))
                || catalog
                    .extra
                    .get(source)
                    .is_some_and(|t| t.contains(target));
            if already {
                continue;
            }
            out.push(Inferred {
                source: source.clone(),
                target: target.to_string(),
                classes,
            });
        }
    }
    out
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Action {
    Replace,
    Add,
    Remove,
}

#[derive(Debug, Clone)]
pub struct Rule {
    target: String,
    field: String,
    key: String,
    action: Action,
    predicate: Predicate,
}

pub fn parse_overrides(text: &str) -> Result<Vec<Rule>, String> {
    let json: serde_json::Value = serde_json::from_str(text).map_err(|e| e.to_string())?;
    if json.get("version").and_then(serde_json::Value::as_i64) != Some(1) {
        return Err("only version 1 override files are supported".to_string());
    }
    let mut rules = Vec::new();
    let Some(overrides) = json.get("overrides").and_then(|v| v.as_object()) else {
        return Ok(rules);
    };
    for (target, fields) in overrides {
        let Some(fields) = fields.as_object() else {
            continue;
        };
        for (raw_key, value) in fields {
            let (action, field) = if let Some(rest) = raw_key.strip_prefix('+') {
                (Action::Add, rest)
            } else if let Some(rest) = raw_key.strip_prefix('-') {
                (Action::Remove, rest)
            } else {
                (Action::Replace, raw_key.as_str())
            };
            let mut push = |key: &str, predicate: Predicate| {
                rules.push(Rule {
                    target: target.to_ascii_lowercase(),
                    field: field.to_string(),
                    key: key.to_ascii_lowercase(),
                    action,
                    predicate,
                });
            };
            match field {
                "depends" | "recommends" | "suggests" | "conflicts" | "breaks" => {
                    let Some(map) = value.as_object() else {
                        return Err(format!("{field} of {target} must be an object"));
                    };
                    for (key, spec) in map {
                        let spec = spec.as_str().unwrap_or("*");
                        let predicate = Predicate::parse(spec)
                            .ok_or_else(|| format!("bad version range {spec} for {target}"))?;
                        push(key, predicate);
                    }
                }
                "provides" => {
                    for item in value
                        .as_array()
                        .into_iter()
                        .flatten()
                        .filter_map(|v| v.as_str())
                    {
                        push(item, Predicate::any());
                    }
                }
                _ => {}
            }
        }
    }
    Ok(rules)
}

#[must_use]
pub fn merge_overrides(layers: Vec<Vec<Rule>>) -> Vec<Rule> {
    let mut out = Vec::new();
    let mut replaced: BTreeMap<String, usize> = BTreeMap::new();
    let mut seen = BTreeSet::new();
    for (layer, rules) in layers.into_iter().enumerate() {
        for rule in rules {
            let field = format!("{}:{}", rule.target, rule.field);
            if replaced.get(&field).is_some_and(|at| *at != layer) {
                continue;
            }
            if !seen.insert(format!("{field}:{}", rule.key)) {
                continue;
            }
            if rule.action == Action::Replace {
                replaced.insert(field, layer);
            }
            out.push(rule);
        }
    }
    out
}

pub fn apply_overrides(rules: &[Rule], meta: &mut ModMeta) {
    apply_to(rules, meta);
    for nested in &mut meta.nested {
        apply_to(rules, nested);
    }
}

fn apply_to(rules: &[Rule], meta: &mut ModMeta) {
    let mine: Vec<&Rule> = rules.iter().filter(|r| r.target == meta.id).collect();
    let mut cleared = BTreeSet::new();
    for rule in mine {
        if rule.field == "provides" {
            match rule.action {
                Action::Replace => {
                    if cleared.insert("provides") {
                        meta.provides.clear();
                    }
                    meta.provides.push(rule.key.clone());
                }
                Action::Add => {
                    if !meta.provides.contains(&rule.key) {
                        meta.provides.push(rule.key.clone());
                    }
                }
                Action::Remove => meta.provides.retain(|p| *p != rule.key),
            }
            continue;
        }
        let (name, map) = match rule.field.as_str() {
            "depends" => ("depends", &mut meta.depends),
            "breaks" => ("breaks", &mut meta.breaks),
            "recommends" => ("recommends", &mut meta.recommends),
            "suggests" => ("suggests", &mut meta.suggests),
            "conflicts" => ("conflicts", &mut meta.conflicts),
            _ => continue,
        };
        match rule.action {
            Action::Replace => {
                if cleared.insert(name) {
                    map.clear();
                }
                map.insert(rule.key.clone(), vec![rule.predicate.clone()]);
            }
            Action::Add => {
                map.insert(rule.key.clone(), vec![rule.predicate.clone()]);
            }
            Action::Remove => {
                map.remove(&rule.key);
            }
        }
    }
}

pub const BUILTIN_OVERRIDES: &str = r#"{
    "version": 1,
    "overrides": {
        "owo-sentinel": {
            "+depends": { "owo": "*", "owo-lib": "*" },
            "-provides": ["owo", "owo-lib"]
        }
    }
}"#;

#[cfg(test)]
mod tests {
    use super::*;

    fn meta(id: &str, version: &str, depends: &[(&str, &str)]) -> ModMeta {
        let mut meta = ModMeta {
            id: id.to_string(),
            version: Version::parse(version),
            provides: Vec::new(),
            depends: Ranges::new(),
            breaks: Ranges::new(),
            recommends: Ranges::new(),
            suggests: Ranges::new(),
            conflicts: Ranges::new(),
            nested: Vec::new(),
            loose_id: false,
        };
        for (dep, range) in depends {
            meta.depends
                .insert((*dep).to_string(), vec![Predicate::parse(range).unwrap()]);
        }
        meta
    }

    fn set(items: &[&str]) -> Set {
        items.iter().map(|s| (*s).to_string()).collect()
    }

    #[test]
    fn resolution_pulls_in_the_newest_matching_provider() {
        let catalog = Catalog::new(
            BTreeMap::from([
                ("a".to_string(), meta("sodium", "1.0", &[("lib", ">=2.0")])),
                ("b".to_string(), meta("lib", "1.0", &[])),
                ("c".to_string(), meta("lib", "2.1", &[])),
                ("d".to_string(), meta("other", "1.0", &[])),
            ]),
            BTreeMap::new(),
        );
        let all = set(&["a", "b", "c", "d"]);
        let resolved = catalog.resolve(&set(&["a"]), &Set::new(), &all);
        assert_eq!(resolved.set, set(&["a", "c"]));
        assert!(resolved.problem.is_none());
    }

    #[test]
    fn a_bundled_library_does_not_pull_in_another_mod() {
        let mut bundler = meta("chat", "1.0", &[("cloth", "*")]);
        bundler.nested.push(meta("cloth", "1.0", &[]));
        let mut other = meta("map", "1.0", &[]);
        other.nested.push(meta("cloth", "2.0", &[]));
        let catalog = Catalog::new(
            BTreeMap::from([("a".to_string(), bundler), ("b".to_string(), other)]),
            BTreeMap::new(),
        );
        let resolved = catalog.resolve(&set(&["a"]), &Set::new(), &set(&["a", "b"]));
        assert_eq!(resolved.set, set(&["a"]));
    }

    #[test]
    fn a_provider_with_an_unknown_version_meets_a_range() {
        let mut lib = meta("lib", "1.0", &[]);
        lib.version = Version::Unknown;
        let catalog = Catalog::new(
            BTreeMap::from([
                ("a".to_string(), meta("create", "1.0", &[("lib", ">=1.0")])),
                ("b".to_string(), lib),
            ]),
            BTreeMap::new(),
        );
        let all = set(&["a", "b"]);
        assert!(catalog.directly_unresolvable(&all, false).is_empty());
        assert_eq!(catalog.resolve(&set(&["a"]), &Set::new(), &all).set, all);
    }

    #[test]
    fn a_guessed_link_to_a_mod_that_is_off_keeps_the_mod_on() {
        let catalog = Catalog::new(
            BTreeMap::from([
                ("a".to_string(), meta("a", "1.0", &[])),
                ("b".to_string(), meta("b", "1.0", &[])),
            ]),
            BTreeMap::from([("a".to_string(), set(&["b"]))]),
        );
        let resolved = catalog.resolve(&set(&["a"]), &Set::new(), &set(&["a"]));
        assert_eq!(resolved.set, set(&["a"]));
        assert!(resolved.problem.is_none());
    }

    #[test]
    fn a_mod_without_its_dependency_is_unresolvable() {
        let catalog = Catalog::new(
            BTreeMap::from([
                ("a".to_string(), meta("a", "1.0", &[("b", "*")])),
                ("b".to_string(), meta("b", "1.0", &[("missing", "*")])),
            ]),
            BTreeMap::new(),
        );
        let details = catalog.unresolvable_details(&set(&["a", "b"]));
        assert_eq!(details.direct.keys().collect::<Vec<_>>(), vec!["b"]);
        assert_eq!(details.transitive["a"], set(&["b"]));
    }

    #[test]
    fn a_bundled_library_with_a_missing_dependency_makes_its_mod_unresolvable() {
        let mut bundler = meta("chat", "1.0", &[]);
        bundler.nested.push(meta("lib", "1.0", &[("api", ">=2.0")]));
        bundler.nested.push(meta("sibling", "1.0", &[("lib", "*")]));
        let catalog = Catalog::new(
            BTreeMap::from([
                ("a".to_string(), bundler),
                ("b".to_string(), meta("api", "1.5", &[])),
            ]),
            BTreeMap::new(),
        );
        let found = catalog.directly_unresolvable(&set(&["a", "b"]), false);
        assert_eq!(found["a"], vec!["api".to_string()]);
    }

    #[test]
    fn ignoring_a_bundled_library_dependency_clears_it() {
        let mut bundler = meta("chat", "1.0", &[]);
        bundler.nested.push(meta("lib", "1.0", &[("api", ">=2.0")]));
        bundler.forget("api");
        let catalog = Catalog::new(
            BTreeMap::from([("a".to_string(), bundler)]),
            BTreeMap::new(),
        );
        assert!(catalog.directly_unresolvable(&set(&["a"]), false).is_empty());
    }

    #[test]
    fn an_override_key_with_an_accent_does_not_crash() {
        assert!(parse_overrides(r#"{"version":1,"overrides":{"x":{"élan":{}}}}"#).is_ok());
    }

    #[test]
    fn overrides_replace_add_and_remove() {
        let rules = parse_overrides(
            r#"{"version":1,"overrides":{"x":{"depends":{"a":"*","b":">=1"},"-provides":["y"]}}}"#,
        )
        .unwrap();
        let mut target = meta("x", "1.0", &[("old", "*")]);
        target.provides.push("y".to_string());
        apply_overrides(&merge_overrides(vec![rules]), &mut target);
        assert_eq!(target.depends.keys().collect::<Vec<_>>(), vec!["a", "b"]);
        assert!(target.provides.is_empty());
    }
}
