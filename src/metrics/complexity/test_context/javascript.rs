//! Test API evidence resolved to declarations in the current lexical scope.
use std::collections::{HashMap, HashSet};
use std::ops::Range;
use tree_sitter::Node;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum Provider {
    Jest,
    Vitest,
    Mocha,
    Node,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum Binding {
    Api(Provider, &'static str),
    Namespace(Provider),
    /// `require('node:test')` is both callable and exposes named exports.
    NodeDefault,
}

struct Declaration {
    name: String,
    scope: Range<usize>,
    available: usize,
    value: Option<Binding>,
    require_at: Option<usize>,
}

struct Bindings {
    declarations: Vec<Declaration>,
    by_name: HashMap<String, Vec<usize>>,
    invalid: HashSet<usize>,
}

impl Bindings {
    fn new(declarations: Vec<Declaration>) -> Self {
        let by_name = declarations.iter().enumerate().fold(
            HashMap::<String, Vec<usize>>::new(),
            |mut map, (i, d)| {
                map.entry(d.name.clone()).or_default().push(i);
                map
            },
        );
        Self {
            declarations,
            by_name,
            invalid: HashSet::new(),
        }
    }

    fn visible(&self, name: &str, at: usize) -> Vec<usize> {
        let candidates: Vec<_> = self
            .by_name
            .get(name)
            .into_iter()
            .flatten()
            .copied()
            .filter(|i| self.declarations[*i].scope.contains(&at))
            .collect();
        let size = candidates
            .iter()
            .map(|i| self.declarations[*i].scope.len())
            .min();
        candidates
            .into_iter()
            .filter(|i| Some(self.declarations[*i].scope.len()) == size)
            .collect()
    }

    fn resolve(&self, name: &str, at: usize) -> Option<Binding> {
        let entries = self.visible(name, at);
        let [index] = entries.as_slice() else {
            return None;
        };
        let declaration = &self.declarations[*index];
        (declaration.available <= at && !self.invalid.contains(index))
            .then_some(declaration.value)
            .flatten()
    }
}

pub(super) fn ranges(root: Node<'_>, source: &[u8]) -> Vec<Range<usize>> {
    let nodes = descendants(root);
    // Collect declaration identities once, including unknown locals that shadow imports.
    let declarations = nodes
        .iter()
        .flat_map(|node| declarations(*node, source))
        .collect();
    let mut bindings = Bindings::new(declarations);
    let assignments: Vec<_> = nodes
        .iter()
        .filter_map(|node| match node.kind() {
            "assignment_expression" | "augmented_assignment_expression" => {
                node.child_by_field_name("left")
            }
            "for_in_statement"
                if !children(*node)
                    .iter()
                    .any(|n| matches!(n.kind(), "const" | "let" | "var")) =>
            {
                node.child_by_field_name("left")
            }
            "unary_expression"
                if node
                    .child_by_field_name("operator")
                    .is_some_and(|op| op.kind() == "delete") =>
            {
                node.child_by_field_name("argument")
            }
            "update_expression" => node
                .child_by_field_name("argument")
                .or_else(|| node.named_child(0)),
            _ => None,
        })
        .flat_map(|target| {
            assignment_names(target, source)
                .into_iter()
                .map(move |name| (name, target.start_byte()))
        })
        .collect();
    let loader_reassigned = assignments
        .iter()
        .any(|(name, at)| name == "require" && bindings.visible(name, *at).is_empty());
    bindings.invalid = bindings
        .declarations
        .iter()
        .enumerate()
        .filter_map(|(i, d)| {
            d.require_at
                .filter(|at| loader_reassigned || !bindings.visible("require", *at).is_empty())
                .map(|_| i)
        })
        .chain(
            assignments
                .iter()
                .flat_map(|(name, at)| bindings.visible(name, *at)),
        )
        .collect();
    // `valid` walks every ancestor, each through a root-down `parent()`, so it
    // runs only for the few calls that already resolved to a test API.
    nodes
        .into_iter()
        .filter(|node| node.kind() == "call_expression")
        .filter_map(|call| callback_range(call, source, &bindings).filter(|_| valid(call)))
        .collect()
}

fn provider(specifier: &str) -> Option<Provider> {
    match specifier {
        "@jest/globals" => Some(Provider::Jest),
        "vitest" => Some(Provider::Vitest),
        "mocha" => Some(Provider::Mocha),
        "node:test" => Some(Provider::Node),
        _ => None,
    }
}

fn canonical_api(provider: Provider, api: &str) -> Option<&'static str> {
    let catalog: &[&str] = match provider {
        Provider::Jest => &[
            "test",
            "it",
            "describe",
            "beforeAll",
            "afterAll",
            "beforeEach",
            "afterEach",
        ],
        Provider::Vitest => &[
            "test",
            "it",
            "describe",
            "suite",
            "beforeAll",
            "afterAll",
            "beforeEach",
            "afterEach",
        ],
        Provider::Mocha | Provider::Node => &[
            "test",
            "it",
            "describe",
            "suite",
            "before",
            "after",
            "beforeEach",
            "afterEach",
        ],
    };
    catalog.iter().copied().find(|candidate| *candidate == api)
}

fn function(node: Node<'_>) -> bool {
    matches!(
        node.kind(),
        "function_declaration"
            | "generator_function_declaration"
            | "function_expression"
            | "generator_function"
            | "arrow_function"
            | "method_definition"
    )
}

fn lexical_scope(node: Node<'_>, var: bool) -> Range<usize> {
    std::iter::successors(node.parent(), |p| p.parent())
        .find(|p| {
            matches!(p.kind(), "program" | "internal_module" | "module")
                || function(*p)
                || (!var
                    && matches!(
                        p.kind(),
                        "statement_block"
                            | "for_statement"
                            | "for_in_statement"
                            | "switch_body"
                            | "catch_clause"
                    ))
        })
        .unwrap_or(node)
        .byte_range()
}

fn local(
    name: &str,
    scope: Range<usize>,
    available: usize,
    value: Option<Binding>,
    require_at: Option<usize>,
) -> Declaration {
    Declaration {
        name: name.to_owned(),
        scope,
        available,
        value,
        require_at,
    }
}

fn declarations(node: Node<'_>, source: &[u8]) -> Vec<Declaration> {
    match node.kind() {
        "import_statement" => import_declarations(node, source),
        "import_alias" => node
            .named_child(0)
            .filter(|n| n.kind() == "identifier")
            .map(|name| {
                local(
                    text(name, source),
                    lexical_scope(node, false),
                    0,
                    None,
                    None,
                )
            })
            .into_iter()
            .collect(),
        "variable_declarator" => variable_declarations(node, source),
        "class" => node
            .child_by_field_name("name")
            .map(|name| local(text(name, source), node.byte_range(), 0, None, None))
            .into_iter()
            .collect(),
        "catch_clause" => node
            .child_by_field_name("parameter")
            .into_iter()
            .flat_map(|p| pattern_names(p, source))
            .map(|name| local(name, node.byte_range(), 0, None, None))
            .collect(),
        "class_declaration"
        | "function_declaration"
        | "generator_function_declaration"
        | "enum_declaration"
        | "internal_module"
        | "module" => {
            let mut found = parameter_declarations(node, source);
            if let Some(name) = node.child_by_field_name("name") {
                found.push(local(
                    text(name, source),
                    lexical_scope(node, false),
                    0,
                    None,
                    None,
                ));
            }
            found
        }
        "function_expression" | "generator_function" | "arrow_function" | "method_definition" => {
            let mut found = parameter_declarations(node, source);
            if matches!(node.kind(), "function_expression" | "generator_function") {
                if let Some(name) = node.child_by_field_name("name") {
                    found.push(local(text(name, source), node.byte_range(), 0, None, None));
                }
            }
            found
        }
        // JS/TS for-in/of declarations do not contain variable_declarator nodes.
        "for_in_statement"
            if children(node)
                .iter()
                .any(|n| matches!(n.kind(), "const" | "let" | "var")) =>
        {
            node.child_by_field_name("left")
                .into_iter()
                .flat_map(|p| pattern_names(p, source))
                .map(|name| {
                    local(
                        name,
                        if children(node).iter().any(|n| n.kind() == "var") {
                            lexical_scope(node, true)
                        } else {
                            node.byte_range()
                        },
                        0,
                        None,
                        None,
                    )
                })
                .collect()
        }
        _ => Vec::new(),
    }
}

fn parameter_declarations(node: Node<'_>, source: &[u8]) -> Vec<Declaration> {
    node.child_by_field_name("parameters")
        .or_else(|| node.child_by_field_name("parameter"))
        .into_iter()
        .flat_map(|p| pattern_names(p, source))
        .map(|name| local(name, node.byte_range(), 0, None, None))
        .collect()
}

fn type_only(node: Node<'_>) -> bool {
    children(node).iter().any(|n| n.kind() == "type")
}

fn import_declarations(node: Node<'_>, source: &[u8]) -> Vec<Declaration> {
    if let Some(clause) = node
        .named_children(&mut node.walk())
        .find(|n| n.kind() == "import_require_clause")
    {
        let value = clause
            .child_by_field_name("source")
            .and_then(|n| string_value(n, source))
            .and_then(provider)
            .filter(|_| valid(node) && !type_only(node))
            .map(|p| {
                if p == Provider::Node {
                    Binding::NodeDefault
                } else {
                    Binding::Namespace(p)
                }
            });
        return clause
            .named_child(0)
            .filter(|n| n.kind() == "identifier")
            .map(|name| {
                local(
                    text(name, source),
                    lexical_scope(node, false),
                    node.end_byte(),
                    value,
                    Some(node.start_byte()),
                )
            })
            .into_iter()
            .collect();
    }
    let provider = node
        .child_by_field_name("source")
        .and_then(|s| string_value(s, source))
        .and_then(provider)
        .filter(|_| valid(node) && !type_only(node));
    let Some(clause) = node
        .named_children(&mut node.walk())
        .find(|n| n.kind() == "import_clause")
    else {
        return Vec::new();
    };
    clause
        .named_children(&mut clause.walk())
        .flat_map(|item| match item.kind() {
            "identifier" => vec![local(
                text(item, source),
                lexical_scope(node, false),
                0,
                provider
                    .filter(|p| *p == Provider::Node)
                    .map(|_| Binding::NodeDefault),
                None,
            )],
            "namespace_import" => item
                .named_child(0)
                .map(|name| {
                    local(
                        text(name, source),
                        lexical_scope(node, false),
                        0,
                        provider.map(Binding::Namespace),
                        None,
                    )
                })
                .into_iter()
                .collect(),
            "named_imports" => item
                .named_children(&mut item.walk())
                .filter(|n| n.kind() == "import_specifier")
                .filter_map(|specifier| {
                    let imported = specifier.child_by_field_name("name")?;
                    let name = specifier.child_by_field_name("alias").unwrap_or(imported);
                    let value = provider.filter(|_| !type_only(specifier)).and_then(|p| {
                        canonical_api(p, text(imported, source)).map(|api| Binding::Api(p, api))
                    });
                    Some(local(
                        text(name, source),
                        lexical_scope(node, false),
                        0,
                        value,
                        None,
                    ))
                })
                .collect(),
            _ => Vec::new(),
        })
        .collect()
}

fn variable_declarations(node: Node<'_>, source: &[u8]) -> Vec<Declaration> {
    let Some(pattern) = node.child_by_field_name("name") else {
        return Vec::new();
    };
    let scope = lexical_scope(
        node,
        node.parent()
            .is_some_and(|p| p.kind() == "variable_declaration"),
    );
    let required = node
        .child_by_field_name("value")
        .filter(|_| valid(node))
        .and_then(|value| required_provider(value, source));
    pattern_names(pattern, source)
        .into_iter()
        .map(|name| {
            let binding = required.and_then(|(p, selected, _)| {
                if pattern.kind() == "identifier" {
                    match selected {
                        Some(api) => canonical_api(p, api).map(|api| Binding::Api(p, api)),
                        None if p == Provider::Node => Some(Binding::NodeDefault),
                        None => Some(Binding::Namespace(p)),
                    }
                } else if pattern.kind() == "object_pattern" && selected.is_none() {
                    pattern
                        .named_children(&mut pattern.walk())
                        .find_map(|item| {
                            let api = match item.kind() {
                                "shorthand_property_identifier_pattern"
                                    if text(item, source) == name =>
                                {
                                    Some(name)
                                }
                                "object_assignment_pattern" => item
                                    .child_by_field_name("left")
                                    .filter(|left| text(*left, source) == name)
                                    .map(|_| name),
                                "pair_pattern" => item
                                    .child_by_field_name("value")
                                    .and_then(|value| {
                                        if value.kind() == "assignment_pattern" {
                                            value.child_by_field_name("left")
                                        } else {
                                            Some(value)
                                        }
                                    })
                                    .filter(|v| {
                                        v.kind() == "identifier" && text(*v, source) == name
                                    })
                                    .and_then(|_| item.child_by_field_name("key"))
                                    .filter(|key| key.kind() == "property_identifier")
                                    .map(|key| text(key, source)),
                                _ => None,
                            }?;
                            canonical_api(p, api).map(|api| Binding::Api(p, api))
                        })
                } else {
                    None
                }
            });
            local(
                name,
                scope.clone(),
                node.end_byte(),
                binding,
                required.map(|(_, _, at)| at),
            )
        })
        .collect()
}

fn required_provider<'a>(
    value: Node<'_>,
    source: &'a [u8],
) -> Option<(Provider, Option<&'a str>, usize)> {
    let (call, selected) = if value.kind() == "member_expression" {
        (
            value.child_by_field_name("object")?,
            Some(text(value.child_by_field_name("property")?, source)),
        )
    } else {
        (value, None)
    };
    if call.kind() != "call_expression" {
        return None;
    }
    let callee = call.child_by_field_name("function")?;
    let args = arguments(call);
    if callee.kind() != "identifier" || text(callee, source) != "require" || args.len() != 1 {
        return None;
    }
    Some((
        provider(string_value(args[0], source)?)?,
        selected,
        call.start_byte(),
    ))
}

/// Nodes reached from `node` by repeatedly applying `next`, in pre-order, with an
/// explicit stack: binding patterns can nest as deeply as the source allows.
fn expand<'tree>(
    node: Node<'tree>,
    next: impl Fn(Node<'tree>) -> Vec<Node<'tree>>,
) -> impl Iterator<Item = Node<'tree>> {
    let mut pending = vec![node];
    std::iter::from_fn(move || {
        let current = pending.pop()?;
        pending.extend(next(current).into_iter().rev());
        Some(current)
    })
}

fn pattern_names<'a>(node: Node<'_>, source: &'a [u8]) -> Vec<&'a str> {
    expand(node, |current| match current.kind() {
        "pair_pattern" => current.child_by_field_name("value").into_iter().collect(),
        "assignment_pattern" | "object_assignment_pattern" => {
            current.child_by_field_name("left").into_iter().collect()
        }
        "required_parameter" | "optional_parameter" => {
            current.child_by_field_name("pattern").into_iter().collect()
        }
        "object_pattern"
        | "array_pattern"
        | "rest_pattern"
        | "formal_parameters"
        | "parenthesized_expression" => current.named_children(&mut current.walk()).collect(),
        _ => Vec::new(),
    })
    .filter(|current| {
        matches!(
            current.kind(),
            "identifier" | "shorthand_property_identifier_pattern"
        )
    })
    .map(|current| text(current, source))
    .collect()
}

fn assignment_names(node: Node<'_>, source: &[u8]) -> Vec<String> {
    // Destructuring and member targets are unwrapped here; whatever remains is a
    // binding pattern with the ordinary declaration rules.
    let unwrapping = |kind: &str| {
        matches!(
            kind,
            "member_expression"
                | "subscript_expression"
                | "pair_pattern"
                | "assignment_pattern"
                | "object_assignment_pattern"
                | "object_pattern"
                | "array_pattern"
                | "rest_pattern"
                | "parenthesized_expression"
        )
    };
    expand(node, |current| match current.kind() {
        "member_expression" | "subscript_expression" => {
            current.child_by_field_name("object").into_iter().collect()
        }
        "pair_pattern" => current.child_by_field_name("value").into_iter().collect(),
        "assignment_pattern" | "object_assignment_pattern" => {
            current.child_by_field_name("left").into_iter().collect()
        }
        "object_pattern" | "array_pattern" | "rest_pattern" | "parenthesized_expression" => {
            current.named_children(&mut current.walk()).collect()
        }
        _ => Vec::new(),
    })
    .filter(|current| !unwrapping(current.kind()))
    .flat_map(|current| pattern_names(current, source))
    .map(str::to_owned)
    .collect()
}

struct Callee<'a> {
    root: &'a str,
    parts: Vec<&'a str>,
    each_called: bool,
}

/// The longest callee any provider accepts is five links (a namespace, its API,
/// modifiers and one `.each(...)` call). The cap of eight links leaves headroom
/// above that: longer chains can never name a test API, so they are rejected
/// without walking to their root; otherwise every call of an `x.a().b()...`
/// chain would re-walk the whole chain.
const MAX_CALLEE_LINKS: usize = 8;

fn callee<'a>(node: Node<'_>, source: &'a [u8]) -> Option<Callee<'a>> {
    // Walk down to the chain's root, then replay the links outwards.
    let chain: Vec<_> = std::iter::successors(Some(node), |link| match link.kind() {
        "member_expression" => link.child_by_field_name("object"),
        "call_expression" => link.child_by_field_name("function"),
        _ => None,
    })
    .take(MAX_CALLEE_LINKS + 2)
    .collect();
    if chain.len() > MAX_CALLEE_LINKS + 1 {
        return None;
    }
    let (root, links) = chain.split_last()?;
    if root.kind() != "identifier" {
        return None;
    }
    let start = Callee {
        root: text(*root, source),
        parts: Vec::new(),
        each_called: false,
    };
    links.iter().rev().try_fold(start, |mut value, link| {
        if link.kind() == "member_expression" {
            if value.each_called {
                return None;
            }
            value
                .parts
                .push(text(link.child_by_field_name("property")?, source));
        } else if value.each_called
            || value.parts.last() != Some(&"each")
            || !(tagged_template(*link) || arguments(*link).len() == 1)
        {
            return None;
        } else {
            value.each_called = true;
        }
        Some(value)
    })
}

fn modifiers_valid(provider: Provider, api: &str, parts: &[&str], each_called: bool) -> bool {
    let suite_or_test = matches!(api, "test" | "it" | "describe" | "suite");
    if !suite_or_test {
        return parts.is_empty() && !each_called;
    }
    if parts.contains(&"each") != each_called {
        return false;
    }
    match provider {
        Provider::Node | Provider::Mocha => matches!(parts, [] | ["only"] | ["skip"]),
        Provider::Jest => {
            matches!(
                parts,
                [] | ["only"] | ["skip"] | ["each"] | ["only", "each"] | ["skip", "each"]
            ) || (matches!(api, "test" | "it")
                && matches!(
                    parts,
                    ["concurrent"]
                        | ["concurrent", "only"]
                        | ["concurrent", "skip"]
                        | ["concurrent", "each"]
                        | ["concurrent", "only", "each"]
                        | ["concurrent", "skip", "each"]
                ))
        }
        Provider::Vitest => {
            let options = if each_called {
                &parts[..parts.len() - 1]
            } else {
                parts
            };
            options
                .iter()
                .all(|p| matches!(*p, "only" | "skip" | "concurrent"))
                && options
                    .iter()
                    .enumerate()
                    .all(|(i, p)| !options[..i].contains(p))
                && !(options.contains(&"only") && options.contains(&"skip"))
        }
    }
}

fn callback_range(call: Node<'_>, source: &[u8], bindings: &Bindings) -> Option<Range<usize>> {
    let called = callee(call.child_by_field_name("function")?, source)?;
    let binding = bindings.resolve(called.root, call.start_byte())?;
    let (provider, api, modifiers) = match binding {
        Binding::Api(p, api) => (p, api, called.parts.as_slice()),
        Binding::Namespace(p) => {
            let (api, parts) = called.parts.split_first()?;
            (p, canonical_api(p, api)?, parts)
        }
        Binding::NodeDefault => {
            if let Some(api) = called
                .parts
                .first()
                .and_then(|name| canonical_api(Provider::Node, name))
            {
                (Provider::Node, api, &called.parts[1..])
            } else {
                (Provider::Node, "test", called.parts.as_slice())
            }
        }
    };
    if !modifiers_valid(provider, api, modifiers, called.each_called) {
        return None;
    }
    callback_argument(provider, api, &arguments(call)).map(|node| node.byte_range())
}

fn callback(node: Node<'_>) -> bool {
    matches!(
        node.kind(),
        "arrow_function" | "function_expression" | "generator_function"
    )
}

fn callback_argument<'a>(provider: Provider, api: &str, args: &[Node<'a>]) -> Option<Node<'a>> {
    let hook = !matches!(api, "test" | "it" | "describe" | "suite");
    let index = if hook {
        match provider {
            Provider::Mocha if args.len() == 2 && !callback(args[0]) => 1,
            _ if (1..=2).contains(&args.len()) => 0,
            _ => return None,
        }
    } else {
        match provider {
            Provider::Node if (1..=3).contains(&args.len()) => args.len() - 1,
            Provider::Vitest if args.len() == 3 && !callback(args[1]) => 2,
            Provider::Jest | Provider::Vitest if (2..=3).contains(&args.len()) => 1,
            Provider::Mocha if args.len() == 2 => 1,
            _ => return None,
        }
    };
    let candidate = *args.get(index)?;
    let slots_valid = if hook {
        match (provider, args) {
            (_, [_]) => true,
            (Provider::Mocha, [name, _]) => signature_slot(*name, &["string", "template_string"]),
            (Provider::Node, [_, options]) => signature_slot(*options, &["object"]),
            (_, [_, timeout]) => signature_slot(*timeout, &["number"]),
            _ => false,
        }
    } else {
        let name_valid = |name| signature_slot(name, &["string", "template_string"]);
        match (provider, args) {
            (Provider::Node, [_]) => true,
            (Provider::Node, [name_or_options, _]) => {
                signature_slot(*name_or_options, &["string", "template_string", "object"])
            }
            (Provider::Node, [name, options, _]) => {
                name_valid(*name) && signature_slot(*options, &["object"])
            }
            (Provider::Vitest, [name, options, _]) if index == 2 => {
                (name_valid(*name) || callback(*name)) && signature_slot(*options, &["object"])
            }
            (Provider::Vitest, [name, _, timeout]) => {
                (name_valid(*name) || callback(*name)) && signature_slot(*timeout, &["number"])
            }
            (Provider::Jest, [name, _, timeout]) => {
                !callback(*name) && signature_slot(*timeout, &["number"])
            }
            (Provider::Vitest, [name, _]) => name_valid(*name) || callback(*name),
            (Provider::Jest, [name, _]) => !callback(*name),
            (Provider::Mocha, [name, _]) => name_valid(*name),
            _ => false,
        }
    };
    (callback(candidate) && slots_valid).then_some(candidate)
}

/// Reject incompatible literal slots without attempting to infer dynamic expression types.
fn signature_slot(node: Node<'_>, allowed: &[&str]) -> bool {
    let Some(node) = std::iter::successors(Some(Some(node)), |current| {
        current
            .filter(|n| n.kind() == "parenthesized_expression")
            .map(|n| n.named_child(0))
    })
    .last()
    .flatten() else {
        return false;
    };
    match node.kind() {
        "string"
        | "template_string"
        | "number"
        | "object"
        | "array"
        | "true"
        | "false"
        | "null"
        | "regex"
        | "arrow_function"
        | "function_expression"
        | "generator_function" => allowed.contains(&node.kind()),
        "spread_element" => false,
        _ => true,
    }
}

fn valid(node: Node<'_>) -> bool {
    !node.has_error()
        && !std::iter::successors(node.parent(), |p| p.parent())
            .any(|p| p.is_error() || p.kind() == "with_statement")
}

/// A tagged template call (`` test.each`a | b ...` ``) passes one table, whatever
/// the number of pieces its template string holds.
fn tagged_template(call: Node<'_>) -> bool {
    call.child_by_field_name("arguments")
        .is_some_and(|arguments| arguments.kind() == "template_string")
}

fn arguments(call: Node<'_>) -> Vec<Node<'_>> {
    call.child_by_field_name("arguments")
        .map(|n| {
            n.named_children(&mut n.walk())
                .filter(|n| n.kind() != "comment")
                .collect()
        })
        .unwrap_or_default()
}

fn string_value<'a>(node: Node<'_>, source: &'a [u8]) -> Option<&'a str> {
    if node.kind() != "string" || node.has_error() {
        return None;
    }
    let value = text(node, source);
    value
        .strip_prefix('\'')
        .and_then(|s| s.strip_suffix('\''))
        .or_else(|| value.strip_prefix('"').and_then(|s| s.strip_suffix('"')))
}

fn text<'a>(node: Node<'_>, source: &'a [u8]) -> &'a str {
    node.utf8_text(source).unwrap_or_default()
}
fn children(node: Node<'_>) -> Vec<Node<'_>> {
    node.children(&mut node.walk()).collect()
}
fn descendants(node: Node<'_>) -> Vec<Node<'_>> {
    super::named_preorder(node, |_| true).collect()
}

#[cfg(test)]
#[path = "javascript_tests.rs"]
mod tests;
