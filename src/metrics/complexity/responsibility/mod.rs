//! File-local structural evidence for responsibility advice. This index does not
//! participate in measurements, extraction, or test classification.
mod resolution;

use std::cell::RefCell;
use std::collections::{BTreeMap, BTreeSet, HashMap};
use std::rc::Rc;

use tree_sitter::Node;

use super::{test_context::TestContext, Language};
use crate::snapshot::{ResponsibilityDependency, ResponsibilityProvenance};

#[derive(Clone)]
struct Owner<'tree> {
    node: Node<'tree>,
    id: String,
    label: String,
    is_type: bool,
}

struct Function<'tree> {
    node: Node<'tree>,
    name: String,
    owner: Option<Owner<'tree>>,
    receiver: Option<String>,
    is_test: bool,
}

pub(super) struct ResponsibilityIndex<'tree> {
    functions: Vec<Function<'tree>>,
    /// Function position by node id, so provenance never scans every function.
    positions: HashMap<usize, usize>,
    /// (owner id, name) of every owned function: a member with such a name is a
    /// method reference, not a field.
    owned_names: BTreeSet<(String, String)>,
    source: &'tree [u8],
    lang: Language,
    root: Node<'tree>,
    modified_members: BTreeSet<String>,
    resolution: resolution::ResolutionIndex<'tree>,
    /// Scope bindings and owner fields depend only on their node, yet every
    /// function nested in that node asks for them again: compute each once.
    scope_bindings: RefCell<HashMap<usize, Rc<Bindings>>>,
    owner_fields: RefCell<HashMap<usize, Rc<BTreeSet<String>>>>,
}

/// File-level facts `owner_of` would otherwise recompute per function.
struct FileDeclarations<'tree> {
    /// Top-level Go `type_spec` and `type_alias` declarations by type name,
    /// malformed ones and those inside a top-level ERROR node included.
    go_types: BTreeMap<String, Vec<Node<'tree>>>,
    /// Top-level namespace declarations that end at their semicolon.
    unbraced_namespaces: Vec<Node<'tree>>,
}

impl<'tree> FileDeclarations<'tree> {
    fn build(root: Node<'tree>, source: &[u8], lang: Language) -> Self {
        let top_level: Vec<_> = root.named_children(&mut root.walk()).collect();
        let go_types = if lang == Language::Go {
            top_level
                .iter()
                // A declaration broken by a parse error sits one level deeper, inside
                // the top-level ERROR node that swallowed it.
                .flat_map(|node| {
                    if node.is_error() {
                        node.named_children(&mut node.walk())
                            .filter(|child| child.kind() == "type_declaration")
                            .collect::<Vec<_>>()
                    } else {
                        vec![*node]
                    }
                })
                .flat_map(|declaration| {
                    declaration
                        .named_children(&mut declaration.walk())
                        .collect::<Vec<_>>()
                })
                // Malformed declarations stay indexed: they make ownership unknown rather
                // than letting the receiver fall back to a type declared elsewhere.
                .filter(|node| matches!(node.kind(), "type_spec" | "type_alias"))
                .filter_map(|node| {
                    node.child_by_field_name("name")
                        .map(|name| (text(name, source).to_owned(), node))
                })
                .fold(BTreeMap::new(), |mut types, (name, node)| {
                    types.entry(name).or_insert_with(Vec::new).push(node);
                    types
                })
        } else {
            BTreeMap::new()
        };
        let unbraced_namespaces = top_level
            .into_iter()
            .filter(|candidate| {
                matches!(
                    candidate.kind(),
                    "namespace_definition" | "file_scoped_namespace_declaration"
                ) && candidate.child_by_field_name("body").is_none()
            })
            .collect();
        Self {
            go_types,
            unbraced_namespaces,
        }
    }
}

impl<'tree> ResponsibilityIndex<'tree> {
    pub(super) fn build(
        root: Node<'tree>,
        source: &'tree [u8],
        lang: Language,
        context: &TestContext,
    ) -> Self {
        let nodes = descendants(root);
        let declarations = FileDeclarations::build(root, source, lang);
        let functions: Vec<_> = nodes
            .iter()
            .copied()
            .filter(|node| is_function(node.kind()))
            .filter_map(|node| {
                let name = node.child_by_field_name("name")?;
                let owner = owner_of(node, root, source, lang, &declarations);
                let receiver = receiver_of(node, owner.as_ref(), source, lang);
                Some(Function {
                    node,
                    name: text(name, source).to_owned(),
                    owner,
                    receiver,
                    is_test: context.contains(node.byte_range()),
                })
            })
            .collect();
        let modified_members = nodes
            .iter()
            .copied()
            .filter(|n| {
                matches!(
                    n.kind(),
                    "assignment_expression"
                        | "assignment"
                        | "augmented_assignment"
                        | "assignment_statement"
                )
            })
            .filter_map(|n| n.child_by_field_name("left"))
            .filter_map(|lhs| match lhs.kind() {
                "field_expression" | "field_access" | "selector_expression" => {
                    lhs.child_by_field_name("field")
                }
                "member_expression" => lhs.child_by_field_name("property"),
                "attribute" => lhs.child_by_field_name("attribute"),
                "member_access_expression" => lhs.child_by_field_name("name"),
                "navigation_expression" => lhs.named_child(1),
                _ => None,
            })
            .filter(|n| is_identifier(*n))
            .map(|n| text(n, source).to_owned())
            .collect();
        let resolution = resolution::ResolutionIndex::build(&nodes, &functions, source);
        let positions = functions
            .iter()
            .enumerate()
            .map(|(position, function)| (function.node.id(), position))
            .collect();
        let owned_names = functions
            .iter()
            .filter_map(|function| {
                function
                    .owner
                    .as_ref()
                    .map(|owner| (owner.id.clone(), function.name.clone()))
            })
            .collect();
        Self {
            functions,
            positions,
            owned_names,
            source,
            lang,
            root,
            modified_members,
            resolution,
            scope_bindings: RefCell::default(),
            owner_fields: RefCell::default(),
        }
    }

    fn bindings_for_scope(&self, scope: Node<'tree>) -> Rc<Bindings> {
        let cached = self.scope_bindings.borrow().get(&scope.id()).cloned();
        cached.unwrap_or_else(|| {
            let computed = Rc::new(bindings_for_scope(scope, self.source));
            self.scope_bindings
                .borrow_mut()
                .insert(scope.id(), Rc::clone(&computed));
            computed
        })
    }

    fn declared_fields(&self, owner: Node<'tree>) -> Rc<BTreeSet<String>> {
        let cached = self.owner_fields.borrow().get(&owner.id()).cloned();
        cached.unwrap_or_else(|| {
            let computed = Rc::new(declared_fields(owner, self.source));
            self.owner_fields
                .borrow_mut()
                .insert(owner.id(), Rc::clone(&computed));
            computed
        })
    }

    pub(super) fn provenance(&self, node: Node<'tree>) -> Option<ResponsibilityProvenance> {
        let function = &self.functions[*self.positions.get(&node.id())?];
        let owner = function.owner.as_ref()?;
        let nodes = own_nodes(function.node);
        // Reassignments/imports in surrounding lexical scopes also invalidate
        // a supposedly static binding. Nested bodies are deliberately skipped.
        let bindings = std::iter::successors(function.node.parent(), |scope| scope.parent())
            .filter(|scope| {
                is_scope(scope.kind())
                    || scope.id() == self.root.id()
                    || is_function_boundary(scope.kind())
            })
            .fold(
                bindings(&nodes, function.node, self.source),
                |mut bindings, scope| {
                    let surrounding = self.bindings_for_scope(scope);
                    bindings
                        .reassigned
                        .extend(surrounding.reassigned.iter().cloned());
                    bindings
                        .modified_members
                        .extend(surrounding.modified_members.iter().cloned());
                    if scope.id() != owner.node.id() || !owner.is_type {
                        bindings.local.extend(surrounding.local.iter().cloned());
                    }
                    bindings
                },
            );
        let fields = self.declared_fields(owner.node);
        let body = function.node.child_by_field_name("body").or_else(|| {
            function
                .node
                .named_children(&mut function.node.walk())
                .find(|n| n.kind() == "function_body")
        });
        let body_nodes = body.map(own_nodes).unwrap_or_default();
        let dependencies = body_nodes
            .iter()
            .filter_map(|&node| {
                if let Some((receiver, name)) = call_target(node, self.source) {
                    return self
                        .resolve_call(function, node, receiver, name, &bindings)
                        // Recursion is the function itself, not a shared collaborator.
                        .filter(|callee| callee.node.id() != function.node.id())
                        .map(|callee| ResponsibilityDependency::Callee {
                            identity: format!("function:{}", callee.node.start_byte()),
                            label: callee.name.clone(),
                        });
                }
                if let Some((receiver, name)) = member(node, self.source) {
                    if !is_called_member(node)
                        && !self
                            .owned_names
                            .contains(&(owner.id.clone(), name.to_owned()))
                        && function.receiver.as_deref() == Some(receiver)
                        && !bindings.reassigned.contains(receiver)
                    {
                        return Some(ResponsibilityDependency::Field {
                            identity: if self.lang == Language::JsTs
                                && resolution::is_static(function.node)
                            {
                                format!("{}:static-field:{name}", owner.id)
                            } else {
                                format!("{}:field:{name}", owner.id)
                            },
                            label: name.to_owned(),
                        });
                    }
                }
                if implicit_receiver(self.lang)
                    && owner.is_type
                    && is_identifier(node)
                    && fields.contains(text(node, self.source))
                    && !bindings.local.contains(text(node, self.source))
                    && is_bare_value(node)
                {
                    let name = text(node, self.source);
                    return Some(ResponsibilityDependency::Field {
                        identity: format!("{}:field:{name}", owner.id),
                        label: name.to_owned(),
                    });
                }
                None
            })
            .collect::<BTreeSet<_>>()
            .into_iter()
            .collect();
        Some(ResponsibilityProvenance {
            owner_id: owner.id.clone(),
            owner_label: owner.label.clone(),
            dependencies,
        })
    }
}

fn owner_of<'tree>(
    function: Node<'tree>,
    root: Node<'tree>,
    source: &[u8],
    lang: Language,
    declarations: &FileDeclarations<'tree>,
) -> Option<Owner<'tree>> {
    if function.has_error()
        || function.is_missing()
        // A declaration an ERROR node swallowed cannot own anything: the parse
        // it belongs to failed, so any structure read off it is a guess. The
        // walk goes to the root because the ancestor loop below stops at the
        // first scope, which leaves an error above that scope unseen — and
        // ERROR nodes are not confined to broken code, a grammar lagging a
        // language version produces them on source that compiles.
        || std::iter::successors(function.parent(), |node| node.parent())
            .any(|node| node.is_error() || node.is_missing())
        || (lang == Language::Kotlin && is_extension(function))
    {
        return None;
    }
    if lang == Language::Go && function.kind() == "method_declaration" {
        let receiver = function.child_by_field_name("receiver")?;
        let parameter = receiver.named_child(0)?;
        let receiver_type = parameter.child_by_field_name("type")?;
        let pointer = receiver_type.kind() == "pointer_type";
        let receiver_type = if pointer {
            receiver_type.named_child(0)?
        } else {
            receiver_type
        };
        let receiver_type = if receiver_type.kind() == "generic_type" {
            receiver_type.child_by_field_name("type")?
        } else {
            receiver_type
        };
        if receiver_type.kind() != "type_identifier" {
            return None;
        }
        let name = text(receiver_type, source);
        let mode = if pointer { "pointer" } else { "value" };
        let label = format!("{}{}", if pointer { "*" } else { "" }, name);
        return match declarations
            .go_types
            .get(name)
            .map(Vec::as_slice)
            .unwrap_or_default()
        {
            // An alias names another type, so it cannot stand for the receiver.
            [declaration] if declaration.kind() == "type_spec" && !declaration.has_error() => {
                Some(Owner {
                    node: *declaration,
                    id: format!("type:{}:{mode}", declaration.start_byte()),
                    label,
                    is_type: true,
                })
            }
            // Go spreads a type's methods across its package's files: a receiver
            // type declared elsewhere is still one owner, named by the type. The
            // receiver node stands in for the declaration, so no declared field
            // or same-node resolution is inferred from it.
            [] => Some(Owner {
                node: receiver_type,
                id: format!("go_type:{name}:{mode}"),
                label,
                is_type: true,
            }),
            _ => None,
        };
    }
    let mut ancestor = function.parent();
    while let Some(node) = ancestor {
        // No error check here: the entry guard above already walked every
        // ancestor, and to the root rather than to the first scope.
        if node.kind() == "class_body"
            && node
                .parent()
                .is_some_and(|parent| parent.kind() == "object_creation_expression")
        {
            return Some(Owner {
                node,
                id: format!("anonymous_class:{}", node.start_byte()),
                label: format!("anonymous class (line {})", node.start_position().row + 1),
                is_type: true,
            });
        }
        if is_scope(node.kind()) || is_function_boundary(node.kind()) || node.id() == root.id() {
            // PHP and C# also support namespaces whose declaration ends at
            // the semicolon; the following declarations remain root children.
            let node = if node.id() == root.id() {
                declarations
                    .unbraced_namespaces
                    .iter()
                    .copied()
                    .take_while(|candidate| candidate.start_byte() < function.start_byte())
                    .last()
                    .unwrap_or(node)
            } else {
                node
            };
            let name = node
                .child_by_field_name("name")
                .or_else(|| node.child_by_field_name("type"))
                .map(|n| {
                    text(n, source)
                        .split_whitespace()
                        .collect::<Vec<_>>()
                        .join(" ")
                })
                .unwrap_or_else(|| anonymous_owner_label(node.kind()).to_owned());
            return Some(Owner {
                node,
                id: format!("{}:{}", node.kind(), node.start_byte()),
                label: if node.id() == root.id() {
                    "file".to_owned()
                } else {
                    format!("{name} (line {})", node.start_position().row + 1)
                },
                is_type: is_type(node.kind()),
            });
        }
        ancestor = node.parent();
    }
    None
}

fn receiver_of(
    function: Node<'_>,
    owner: Option<&Owner<'_>>,
    source: &[u8],
    lang: Language,
) -> Option<String> {
    if !owner?.is_type {
        return None;
    }
    match lang {
        Language::Rust => descendants(function.child_by_field_name("parameters")?)
            .into_iter()
            .any(|n| n.kind() == "self_parameter")
            .then(|| "self".to_owned()),
        Language::Python => {
            if function
                .parent()
                .and_then(|n| n.parent())
                .is_some_and(|n| n.kind() != "class_definition")
            {
                return None;
            }
            if function
                .parent()
                .is_some_and(|n| n.kind() == "decorated_definition")
            {
                return None;
            }
            let first = function.child_by_field_name("parameters")?.named_child(0)?;
            let first = if first.kind() == "typed_parameter" {
                first.named_child(0)?
            } else {
                first
            };
            is_identifier(first).then(|| text(first, source).to_owned())
        }
        Language::Go => function
            .child_by_field_name("receiver")?
            .named_child(0)?
            .child_by_field_name("name")
            .map(|n| text(n, source).to_owned()),
        Language::Php => Some("$this".to_owned()),
        Language::JsTs | Language::Java | Language::CSharp | Language::Kotlin => {
            Some("this".to_owned())
        }
        Language::Generic => None,
    }
}

fn is_extension(function: Node<'_>) -> bool {
    function.child_by_field_name("name").is_some_and(|name| {
        function.named_children(&mut function.walk()).any(|node| {
            node.end_byte() <= name.start_byte()
                && matches!(node.kind(), "user_type" | "nullable_type" | "function_type")
        })
    })
}

fn is_function(kind: &str) -> bool {
    matches!(
        kind,
        "function_item"
            | "function_declaration"
            | "function_definition"
            | "method_declaration"
            | "method_definition"
            | "method_signature"
            | "function_signature_item"
    )
}

fn is_function_boundary(kind: &str) -> bool {
    is_function(kind)
        || matches!(
            kind,
            "arrow_function"
                | "function_expression"
                | "generator_function"
                | "closure_expression"
                | "lambda"
                | "lambda_expression"
                | "lambda_literal"
                | "anonymous_function"
                | "anonymous_function_creation_expression"
                | "local_function_statement"
                | "func_literal"
                | "anonymous_method_expression"
                | "generator_function_declaration"
                | "generator_expression"
        )
}

/// Advice names an unnamed owner by what a reader sees, never by a grammar kind.
fn anonymous_owner_label(kind: &str) -> &'static str {
    match kind {
        "object" | "object_literal" => "object literal",
        "companion_object" => "companion object",
        "class" | "anonymous_class" => "anonymous class",
        kind if is_function_boundary(kind) => "anonymous function",
        _ => "block",
    }
}

fn is_type(kind: &str) -> bool {
    matches!(
        kind,
        "impl_item"
            | "trait_item"
            | "class_declaration"
            | "class_definition"
            | "class"
            | "interface_declaration"
            | "trait_declaration"
            | "object_declaration"
            | "object"
            | "object_literal"
            | "anonymous_class"
            | "struct_declaration"
            | "record_declaration"
            | "enum_declaration"
            | "companion_object"
    )
}

fn is_scope(kind: &str) -> bool {
    is_type(kind)
        || matches!(
            kind,
            "mod_item"
                | "internal_module"
                | "module_declaration"
                | "namespace_definition"
                | "namespace_declaration"
                | "file_scoped_namespace_declaration"
        )
}

fn implicit_receiver(lang: Language) -> bool {
    matches!(lang, Language::Java | Language::CSharp | Language::Kotlin)
}

fn text<'source>(node: Node<'_>, source: &'source [u8]) -> &'source str {
    std::str::from_utf8(&source[node.byte_range()]).unwrap_or("")
}

fn descendants(node: Node<'_>) -> Vec<Node<'_>> {
    let mut nodes = vec![node];
    let mut index = 0;
    while index < nodes.len() {
        let parent = nodes[index];
        let mut cursor = parent.walk();
        nodes.extend(parent.named_children(&mut cursor));
        index += 1;
    }
    nodes
}

/// Visit each function body once, stopping before nested functions and types.
fn own_nodes(function: Node<'_>) -> Vec<Node<'_>> {
    let mut nodes = vec![function];
    let mut index = 0;
    while index < nodes.len() {
        let parent = nodes[index];
        if parent.id() == function.id()
            || (!is_function_boundary(parent.kind())
                && !is_scope(parent.kind())
                && !(parent.kind() == "class_body"
                    && parent
                        .parent()
                        .is_some_and(|n| n.kind() == "object_creation_expression")))
        {
            let mut cursor = parent.walk();
            nodes.extend(parent.named_children(&mut cursor));
        }
        index += 1;
    }
    nodes
}

#[derive(Default)]
struct Bindings {
    local: BTreeSet<String>,
    reassigned: BTreeSet<String>,
    modified_members: BTreeSet<String>,
}

fn bindings(nodes: &[Node<'_>], function: Node<'_>, source: &[u8]) -> Bindings {
    let mut result = Bindings::default();
    if let Some(parameters) = function.child_by_field_name("parameters").or_else(|| {
        function
            .named_children(&mut function.walk())
            .find(|n| n.kind() == "function_value_parameters")
    }) {
        result.local.extend(
            descendants(parameters)
                .into_iter()
                .filter(|n| is_identifier(*n))
                .map(|n| text(n, source).to_owned()),
        );
    }
    for &node in nodes {
        let binding = match node.kind() {
            "let_declaration" | "for_expression" | "match_arm" | "let_condition" => {
                node.child_by_field_name("pattern")
            }
            // C# deconstruction `var (a, b) = …` binds a tuple pattern, not a name.
            "variable_declarator" => node.child_by_field_name("name").or_else(|| {
                node.named_children(&mut node.walk())
                    .find(|n| n.kind() == "tuple_pattern")
            }),
            "parameter"
            | "simple_parameter"
            | "catch_formal_parameter"
            | "instanceof_expression"
            | "declaration_pattern"
            | "declaration_expression"
            | "catch_declaration"
            | "from_clause"
            | "resource" => node.child_by_field_name("name"),
            // Java patterns name their binding after the type, with no field label.
            "type_pattern" | "record_pattern_component" => node
                .named_children(&mut node.walk())
                .filter(|n| n.kind() == "identifier")
                .last(),
            "catch_block" => node
                .named_children(&mut node.walk())
                .find(|n| n.kind() == "identifier"),
            "when_subject" => node
                .named_children(&mut node.walk())
                .find(|n| n.kind() == "variable_declaration"),
            "assignment_expression"
            | "assignment"
            | "augmented_assignment"
            | "assignment_statement"
            | "short_var_declaration"
            | "augmented_assignment_expression" => node.child_by_field_name("left"),
            "range_clause" | "for_in_statement" | "for_in_clause" | "foreach_statement" => {
                node.child_by_field_name("left")
            }
            "enhanced_for_statement" => node.child_by_field_name("name"),
            "as_pattern" => node.child_by_field_name("alias"),
            "catch_clause" => node.child_by_field_name("parameter"),
            "local_function_statement" | "generator_function_declaration" => {
                node.child_by_field_name("name")
            }
            "for_statement" => node.child_by_field_name("left").or_else(|| {
                node.named_children(&mut node.walk()).find(|n| {
                    matches!(
                        n.kind(),
                        "variable_declaration" | "multi_variable_declaration"
                    )
                })
            }),
            "property_declaration" => node.named_children(&mut node.walk()).find(|n| {
                matches!(
                    n.kind(),
                    "variable_declaration" | "multi_variable_declaration"
                )
            }),
            "import_statement" | "import_from_statement" | "use_declaration" => Some(node),
            _ => None,
        };
        if let Some(binding) = binding {
            if let Some((_, member)) = member(binding, source) {
                result.modified_members.insert(member.to_owned());
            }
            // Attribute/selector writes do not rebind their receiver.
            let names = binding_names(binding, source);
            result.local.extend(names.iter().cloned());
            result.reassigned.extend(names);
        }
    }
    result
}

fn bindings_for_scope(scope: Node<'_>, source: &[u8]) -> Bindings {
    bindings(&own_nodes(scope), scope, source)
}

fn binding_names(node: Node<'_>, source: &[u8]) -> BTreeSet<String> {
    let name = |n: Node<'_>| is_identifier(n) || n.kind() == "variable_name";
    // Member writes bind no name; patterns may nest as deeply as the source does.
    super::test_context::named_preorder(node, |n| !name(n) && member(n, source).is_none())
        .filter(|n| name(*n))
        .map(|n| text(n, source).to_owned())
        .collect()
}

fn is_identifier(node: Node<'_>) -> bool {
    matches!(
        node.kind(),
        "identifier"
            | "field_identifier"
            | "property_identifier"
            | "private_property_identifier"
            | "name"
            | "simple_identifier"
    )
}

fn member<'source>(node: Node<'_>, source: &'source [u8]) -> Option<(&'source str, &'source str)> {
    let (receiver, name) = match node.kind() {
        "field_expression" => (
            node.child_by_field_name("value")?,
            node.child_by_field_name("field")?,
        ),
        "member_expression" => (
            node.child_by_field_name("object")?,
            node.child_by_field_name("property")?,
        ),
        "attribute" => (
            node.child_by_field_name("object")?,
            node.child_by_field_name("attribute")?,
        ),
        "selector_expression" => (
            node.child_by_field_name("operand")?,
            node.child_by_field_name("field")?,
        ),
        "field_access" => (
            node.child_by_field_name("object")?,
            node.child_by_field_name("field")?,
        ),
        "member_access_expression" | "member_call_expression" => (
            node.child_by_field_name("object")
                .or_else(|| node.child_by_field_name("expression"))
                .or_else(|| node.child(0))?,
            node.child_by_field_name("name")?,
        ),
        "navigation_expression" => (node.named_child(0)?, node.named_child(1)?),
        "scoped_identifier" | "scoped_call_expression" => (
            node.child_by_field_name("path")
                .or_else(|| node.child_by_field_name("scope"))?,
            node.child_by_field_name("name")?,
        ),
        _ => return None,
    };
    if !is_identifier(name)
        && !(node.kind() == "field_expression" && name.kind() == "integer_literal")
    {
        return None;
    }
    let receiver_text = text(receiver, source);
    if !(is_identifier(receiver)
        || matches!(
            receiver.kind(),
            "self"
                | "this"
                | "this_expression"
                | "variable_name"
                | "type_identifier"
                | "relative_scope"
        ))
    {
        return None;
    }
    Some((receiver_text, text(name, source)))
}

fn call_target<'source>(
    node: Node<'_>,
    source: &'source [u8],
) -> Option<(Option<&'source str>, &'source str)> {
    match node.kind() {
        "method_invocation" => {
            let name = text(node.child_by_field_name("name")?, source);
            let receiver = node.child_by_field_name("object").map(|n| text(n, source));
            Some((receiver, name))
        }
        "member_call_expression" | "scoped_call_expression" => {
            member(node, source).map(|(r, n)| (Some(r), n))
        }
        "call_expression" | "call" | "invocation_expression" | "function_call_expression" => {
            let callee = node
                .child_by_field_name("function")
                .or_else(|| node.named_child(0))?;
            if is_identifier(callee) {
                Some((None, text(callee, source)))
            } else {
                member(callee, source).map(|(r, n)| (Some(r), n))
            }
        }
        _ => None,
    }
}

fn is_called_member(node: Node<'_>) -> bool {
    matches!(
        node.kind(),
        "member_call_expression" | "scoped_call_expression"
    ) || node.parent().is_some_and(|parent| {
        matches!(
            parent.kind(),
            "call_expression" | "call" | "invocation_expression" | "function_call_expression"
        ) && parent
            .child_by_field_name("function")
            .or_else(|| parent.named_child(0))
            .is_some_and(|callee| callee.id() == node.id())
    })
}

fn declared_fields(owner: Node<'_>, source: &[u8]) -> BTreeSet<String> {
    let mut fields = BTreeSet::new();
    let mut nodes = vec![owner];
    while let Some(node) = nodes.pop() {
        if node.id() != owner.id() && (is_function_boundary(node.kind()) || is_scope(node.kind())) {
            continue;
        }
        if matches!(
            node.kind(),
            "field_declaration" | "property_declaration" | "class_parameter"
        ) {
            fields.extend(descendants(node).into_iter().filter_map(|n| {
                n.child_by_field_name("name")
                    .filter(|name| is_identifier(*name))
                    .map(|name| text(name, source).to_owned())
            }));
            if node.kind() == "class_parameter" {
                if let Some(name) = node.named_child(0).filter(|n| is_identifier(*n)) {
                    fields.insert(text(name, source).to_owned());
                }
            }
            // Kotlin property names are inside variable_declaration without a field label.
            // A C# variable_declaration labels its type instead: its first child is no name.
            for variable in descendants(node).into_iter().filter(|n| {
                n.kind() == "variable_declaration" && n.child_by_field_name("type").is_none()
            }) {
                if let Some(name) = variable.named_child(0).filter(|n| is_identifier(*n)) {
                    fields.insert(text(name, source).to_owned());
                }
            }
        }
        let mut cursor = node.walk();
        nodes.extend(node.named_children(&mut cursor));
    }
    fields
}

fn is_bare_value(node: Node<'_>) -> bool {
    node.parent().is_some_and(|parent| {
        !parent
            .child_by_field_name("name")
            .is_some_and(|name| name.id() == node.id())
            && !(parent.kind() == "value_argument"
                && node.next_sibling().is_some_and(|next| next.kind() == "="))
            && !matches!(
                parent.kind(),
                "member_expression"
                    | "member_access_expression"
                    | "field_access"
                    | "navigation_expression"
                    | "method_invocation"
                    | "call_expression"
                    | "invocation_expression"
                    | "variable_declarator"
                    | "variable_declaration"
                    | "formal_parameter"
                    | "parameter"
                    | "user_type"
                    | "function_declaration"
                    | "method_declaration"
            )
    })
}

#[cfg(test)]
mod mutation_tests;
#[cfg(test)]
mod tests;
