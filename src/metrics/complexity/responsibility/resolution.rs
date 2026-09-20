//! Resolve names at the call site, before considering a declaration's methods.
use std::collections::{BTreeMap, BTreeSet};

use tree_sitter::Node;

use super::{
    implicit_receiver, is_function_boundary, is_scope, is_type, text, Bindings, Function, Language,
    ResponsibilityIndex,
};

type ScopedNames<T> = BTreeMap<usize, BTreeMap<String, Vec<T>>>;

/// Built once from the file walk. Calls only inspect declarations with the
/// requested name in a visible scope; they never traverse the file again.
#[derive(Default)]
pub(super) struct ResolutionIndex<'tree> {
    shadowing: BTreeMap<usize, BTreeSet<String>>,
    receivers: ScopedNames<Node<'tree>>,
    scoped_functions: ScopedNames<usize>,
    functions: BTreeMap<String, Vec<usize>>,
    namespaces: BTreeMap<usize, usize>,
}

impl<'tree> ResolutionIndex<'tree> {
    pub(super) fn build(
        nodes: &[Node<'tree>],
        functions: &[Function<'tree>],
        source: &[u8],
    ) -> Self {
        let mut index = Self::default();
        for &node in nodes {
            // Kind first: `parent()` re-descends from the root, so asking it of
            // every node would make this loop quadratic in the file's depth.
            if matches!(
                node.kind(),
                "namespace_definition" | "file_scoped_namespace_declaration"
            ) && node.child_by_field_name("body").is_none()
                && node.parent().is_some_and(|p| p.parent().is_none())
            {
                index.namespaces.insert(node.start_byte(), node.id());
            }
            if !(is_scope(node.kind())
                || node.kind() == "type_spec"
                || is_function_boundary(node.kind()))
            {
                continue;
            }
            let Some(scope) = declaration_scope(node) else {
                continue;
            };
            if is_scope(node.kind()) {
                if let Some(name) = node.child_by_field_name("name") {
                    index
                        .shadowing
                        .entry(scope.id())
                        .or_default()
                        .insert(text(name, source).to_owned());
                }
            }
            if let Some(name) = node
                .child_by_field_name("name")
                .or_else(|| node.child_by_field_name("type"))
            {
                index
                    .receivers
                    .entry(scope.id())
                    .or_default()
                    .entry(text(name, source).to_owned())
                    .or_default()
                    .push(node);
            }
        }
        for (position, function) in functions.iter().enumerate() {
            index
                .functions
                .entry(function.name.clone())
                .or_default()
                .push(position);
            if function.owner.is_some() {
                if let Some(scope) = declaration_scope(function.node) {
                    index
                        .scoped_functions
                        .entry(scope.id())
                        .or_default()
                        .entry(function.name.clone())
                        .or_default()
                        .push(position);
                }
            }
        }
        index
    }

    fn namespace_at(&self, position: usize) -> Option<usize> {
        self.namespaces
            .range(..position)
            .next_back()
            .map(|(_, id)| *id)
    }
}

impl<'tree> ResponsibilityIndex<'tree> {
    /// A name alone can pick a local declaration while the call reaches an
    /// inherited overload or a library function: the argument count must agree
    /// with the declared parameters, and an undecidable count is no evidence.
    pub(super) fn resolve_call(
        &self,
        caller: &Function<'tree>,
        call: Node<'tree>,
        receiver: Option<&str>,
        name: &str,
        bindings: &Bindings,
    ) -> Option<&Function<'tree>> {
        let instance_call = receiver.is_some() && caller.receiver.as_deref() == receiver;
        // Rust and Python methods declare their receiver; method syntax passes it implicitly.
        let implicit_receiver = usize::from(
            matches!(self.lang, Language::Rust | Language::Python)
                && instance_call
                && call
                    .child_by_field_name("function")
                    .is_some_and(|f| matches!(f.kind(), "field_expression" | "attribute")),
        );
        // A Go method expression `T.method(recv, …)` passes the receiver the
        // declaration keeps outside its parameter list.
        let explicit_receiver =
            usize::from(self.lang == Language::Go && receiver.is_some() && !instance_call);
        self.resolve_named_call(caller, call, receiver, name, bindings)
            .filter(|callee| {
                argument_count(call)
                    .zip(parameter_count(callee.node))
                    .is_some_and(|(arguments, parameters)| {
                        arguments + implicit_receiver == parameters + explicit_receiver
                    })
            })
    }

    fn resolve_named_call(
        &self,
        caller: &Function<'tree>,
        call: Node<'tree>,
        receiver: Option<&str>,
        name: &str,
        bindings: &Bindings,
    ) -> Option<&Function<'tree>> {
        let owner = caller.owner.as_ref()?;
        if bindings.modified_members.contains(name) || self.modified_members.contains(name) {
            return None;
        }
        if let Some(receiver) = receiver {
            // Rust `self::helper()` names the lexical module; `self.helper()`
            // names the instance. Their receiver text alone cannot distinguish them.
            if self.lang == Language::Rust
                && receiver == "self"
                && call
                    .child_by_field_name("function")
                    .is_some_and(|n| n.kind() == "scoped_identifier")
            {
                let mut scope = caller.node.parent();
                while let Some(node) = scope {
                    if node.kind() == "mod_item" || node.id() == self.root.id() {
                        return self.unique_callee(name, |candidate| {
                            candidate
                                .owner
                                .as_ref()
                                .is_some_and(|o| o.node.id() == node.id())
                        });
                    }
                    scope = node.parent();
                }
                return None;
            }
            if bindings.reassigned.contains(receiver) {
                return None;
            }
            if caller.receiver.as_deref() == Some(receiver) {
                return self.unique_callee(name, |candidate| {
                    candidate.owner.as_ref().is_some_and(|o| {
                        o.id == owner.id
                            || (self.lang == Language::Go && o.node.id() == owner.node.id())
                    }) && (self.lang != Language::JsTs
                        || is_static(candidate.node) == is_static(caller.node))
                });
            }
            if (receiver == "Self" && self.lang == Language::Rust)
                || (receiver == "self" && self.lang == Language::Php)
            {
                return self.unique_callee(name, |candidate| {
                    candidate.owner.as_ref().is_some_and(|o| o.id == owner.id)
                        && callable_through_type(candidate, self.lang)
                });
            }
            if bindings.local.contains(receiver) {
                return None;
            }
            let declaration = self.resolve_receiver(call, receiver)?;
            return self.unique_callee(name, |candidate| {
                candidate
                    .owner
                    .as_ref()
                    .is_some_and(|o| o.node.id() == declaration.id())
                    && callable_through_type(candidate, self.lang)
            });
        }
        if bindings.local.contains(name) || bindings.reassigned.contains(name) {
            return None;
        }
        let mut scope = Some(call);
        while let Some(node) = scope {
            if node.id() == self.root.id()
                && matches!(
                    owner.node.kind(),
                    "namespace_definition" | "file_scoped_namespace_declaration"
                )
            {
                break;
            }
            if !type_scope(node) || implicit_receiver(self.lang) {
                if self
                    .resolution
                    .shadowing
                    .get(&node.id())
                    .is_some_and(|names| names.contains(name))
                {
                    return None;
                }
                let candidates = self
                    .resolution
                    .scoped_functions
                    .get(&node.id())
                    .and_then(|names| names.get(name))
                    .map(Vec::as_slice)
                    .unwrap_or_default();
                if !candidates.is_empty() {
                    return (candidates.len() == 1
                        && !self.functions[candidates[0]].is_test
                        && has_body(self.functions[candidates[0]].node))
                    .then_some(&self.functions[candidates[0]]);
                }
            }
            if (self.lang == Language::Rust && node.kind() == "mod_item")
                || (implicit_receiver(self.lang) && type_scope(node))
            {
                break;
            }
            scope = node.parent();
        }
        if matches!(
            owner.node.kind(),
            "namespace_definition" | "file_scoped_namespace_declaration"
        ) {
            return self.unique_callee(name, |candidate| {
                candidate.owner.as_ref().is_some_and(|o| o.id == owner.id)
            });
        }
        None
    }

    fn unique_callee(
        &self,
        name: &str,
        matches: impl Fn(&Function<'tree>) -> bool,
    ) -> Option<&Function<'tree>> {
        let candidates: Vec<_> = self
            .resolution
            .functions
            .get(name)
            .into_iter()
            .flatten()
            .map(|&position| &self.functions[position])
            .filter(|candidate| matches(candidate))
            .collect();
        (candidates.len() == 1 && !candidates[0].is_test && has_body(candidates[0].node))
            .then(|| candidates[0])
    }

    fn resolve_receiver(&self, call: Node<'tree>, name: &str) -> Option<Node<'tree>> {
        // Include empty declarations: an inner class still shadows an outer
        // class when the requested method exists only on the outer one.
        let namespace = self.resolution.namespace_at(call.start_byte());
        let mut scope = Some(call);
        while let Some(node) = scope {
            let candidates: Vec<_> = self
                .resolution
                .receivers
                .get(&node.id())
                .and_then(|names| names.get(name))
                .into_iter()
                .flatten()
                .copied()
                .filter(|candidate| {
                    self.resolution.namespace_at(candidate.start_byte()) == namespace
                })
                .collect();
            if !candidates.is_empty() {
                return (candidates.len() == 1
                    && !candidates[0].has_error()
                    && !is_function_boundary(candidates[0].kind()))
                .then_some(candidates[0]);
            }
            scope = node.parent();
        }
        None
    }
}

/// Blocks matter for Rust and JavaScript declarations even when responsibility
/// groups continue to use their enclosing function as their owner.
fn declaration_scope(node: Node<'_>) -> Option<Node<'_>> {
    let mut scope = node.parent();
    while let Some(parent) = scope {
        if parent.kind() == "block" && parent.parent().is_some_and(type_scope) {
            return parent.parent();
        }
        if type_scope(parent)
            || is_scope(parent.kind())
            || is_function_boundary(parent.kind())
            || parent.parent().is_none()
            || matches!(
                parent.kind(),
                "block" | "statement_block" | "compound_statement"
            )
        {
            return Some(parent);
        }
        scope = parent.parent();
    }
    None
}

fn type_scope(node: Node<'_>) -> bool {
    is_type(node.kind())
        || (node.kind() == "class_body"
            && node
                .parent()
                .is_some_and(|p| p.kind() == "object_creation_expression"))
}

fn has_body(node: Node<'_>) -> bool {
    node.child_by_field_name("body").is_some()
        || node
            .named_children(&mut node.walk())
            .any(|n| n.kind() == "function_body")
}

/// Positional parameters of a declaration whose arity is fixed; `None` for
/// defaults, optional, rest or variadic parameters and any unrecognised form.
fn parameter_count(function: Node<'_>) -> Option<usize> {
    if function
        .parent()
        .is_some_and(|parent| parent.kind() == "decorated_definition")
    {
        return None;
    }
    let list = function.child_by_field_name("parameters").or_else(|| {
        function
            .named_children(&mut function.walk())
            .find(|n| n.kind() == "function_value_parameters")
    })?;
    // Kotlin places default values beside the parameter, directly in the list.
    if list.children(&mut list.walk()).any(|n| n.kind() == "=") {
        return None;
    }
    let has_default = |parameter: Node<'_>| {
        parameter.child_by_field_name("value").is_some()
            || parameter.child_by_field_name("default_value").is_some()
            || parameter
                .children(&mut parameter.walk())
                .any(|n| n.kind() == "=")
    };
    list.named_children(&mut list.walk())
        .try_fold(0, |count, parameter| match parameter.kind() {
            "comment"
            | "attribute_item"
            | "attribute_list"
            | "receiver_parameter"
            | "positional_separator" => Some(count),
            "required_parameter"
                if parameter
                    .child_by_field_name("pattern")
                    .is_some_and(|pattern| pattern.kind() == "this") =>
            {
                Some(count)
            }
            "required_parameter"
                if parameter
                    .child_by_field_name("pattern")
                    .is_some_and(|pattern| pattern.kind() == "rest_pattern") =>
            {
                None
            }
            "parameter_declaration" => Some(
                count
                    + parameter
                        .children_by_field_name("name", &mut parameter.walk())
                        .count()
                        .max(1),
            ),
            "self_parameter"
            | "parameter"
            | "formal_parameter"
            | "simple_parameter"
            | "property_promotion_parameter"
            | "required_parameter"
            | "identifier"
            | "typed_parameter"
            | "object_pattern"
            | "array_pattern"
                if !has_default(parameter) =>
            {
                Some(count + 1)
            }
            _ => None,
        })
}

/// Arguments a call passes explicitly, trailing Kotlin lambdas included; `None`
/// when a spread or unpacked argument makes the count unknowable.
fn argument_count(call: Node<'_>) -> Option<usize> {
    let lambdas = |node: Node<'_>| {
        node.named_children(&mut node.walk())
            .filter(|n| n.kind() == "annotated_lambda")
            .count()
    };
    let listed = match call.child_by_field_name("arguments").or_else(|| {
        call.named_children(&mut call.walk())
            .find(|n| n.kind() == "value_arguments")
    }) {
        None => 0,
        // A tagged template passes its strings array plus one value per substitution.
        Some(list) if list.kind() == "template_string" => return None,
        Some(list) if list.kind() == "generator_expression" => 1,
        Some(list) => list
            .named_children(&mut list.walk())
            .try_fold(0, |count, argument| match argument.kind() {
                "comment" => Some(count),
                "spread_element" | "list_splat" | "dictionary_splat" | "variadic_argument" => None,
                "argument"
                    if argument
                        .named_children(&mut argument.walk())
                        .any(|n| n.kind() == "variadic_unpacking") =>
                {
                    None
                }
                // kotlin-ng parses `*xs` as value_argument > spread_expression.
                "value_argument"
                    if argument
                        .named_children(&mut argument.walk())
                        .any(|n| n.kind() == "spread_expression") =>
                {
                    None
                }
                _ => Some(count + 1),
            })?,
    };
    // `check(v) { … }` nests the argument call inside the call carrying the lambda.
    let trailing = call
        .parent()
        .filter(|parent| {
            parent.kind() == "call_expression"
                && parent.named_child(0).is_some_and(|n| n.id() == call.id())
        })
        .map(lambdas)
        .unwrap_or(0);
    Some(listed + lambdas(call) + trailing)
}

pub(super) fn is_static(node: Node<'_>) -> bool {
    node.children(&mut node.walk()).any(|child| {
        child.kind() == "static"
            || child.kind() == "static_modifier"
            || (matches!(child.kind(), "modifier" | "modifiers")
                && child
                    .children(&mut child.walk())
                    .any(|token| token.kind() == "static"))
    })
}

fn callable_through_type(function: &Function<'_>, lang: Language) -> bool {
    let Some(owner) = function.owner.as_ref() else {
        return false;
    };
    if !owner.is_type {
        return true;
    }
    match lang {
        Language::JsTs | Language::Java | Language::CSharp | Language::Php => {
            is_static(function.node)
        }
        Language::Kotlin => matches!(owner.node.kind(), "object_declaration" | "companion_object"),
        Language::Go => owner.id.ends_with(":value"),
        Language::Rust | Language::Python => true,
        Language::Generic => false,
    }
}

#[cfg(test)]
mod tests;
