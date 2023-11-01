// Copyright (c) ZeroC, Inc.

use slicec::{
    compilation_state::CompilationState,
    grammar::{
        Class, Commentable, CustomType, Entity, Enum, Enumerator, Exception, Field, Identifier,
        Interface, MessageComponent, Module, NamedSymbol, Operation, Parameter, Struct, TypeAlias,
        TypeRef, TypeRefDefinition, Types,
    },
    slice_file::{Location, SliceFile, Span},
    visitor::Visitor,
};
use tower_lsp::lsp_types::{Position, Url};

pub fn get_definition_span(state: &CompilationState, uri: Url, position: Position) -> Option<Span> {
    // Attempt to convert the URL to a file path and then to a string
    let file_path = uri.to_file_path().ok()?.to_str()?.to_owned();

    // Attempt to retrieve the file from the state
    let file = state.files.get(&file_path)?;

    // Convert position to row and column to 1 based
    let col = (position.character + 1) as usize;
    let row = (position.line + 1) as usize;
    let location: slicec::slice_file::Location = (row, col).into();

    let mut visitor = JumpVisitor::new(location);
    file.visit_with(&mut visitor);

    visitor.found_span
}

struct JumpVisitor {
    pub search_location: Location,
    pub found_span: Option<Span>,
}

impl JumpVisitor {
    pub fn new(search_location: Location) -> Self {
        JumpVisitor {
            search_location,
            found_span: None,
        }
    }

    fn check_comment(&mut self, commentable: &dyn Commentable) {
        if let Some(comment) = commentable.comment() {
            comment
                .see
                .iter()
                .for_each(|s| self.check_and_set_span(s.linked_entity(), &s.span));
            comment
                .throws
                .iter()
                .for_each(|s| self.check_and_set_span(s.thrown_type(), &s.span));

            // Check all comment types that can have a link in their message
            if let Some(overview) = &comment.overview {
                self.check_message_links(&overview.message)
            }
            comment
                .returns
                .iter()
                .for_each(|returns| self.check_message_links(&returns.message));
            comment
                .params
                .iter()
                .for_each(|params| self.check_message_links(&params.message));
            comment
                .throws
                .iter()
                .for_each(|throws| self.check_message_links(&throws.message));
        }
    }

    fn check_message_links(&mut self, message: &Vec<MessageComponent>) {
        message.iter().for_each(|m| {
            if let MessageComponent::Link(l) = m {
                self.check_and_set_span(l.linked_entity(), &l.span);
            }
        });
    }

    fn check_and_set_span<T: Entity + ?Sized>(
        &mut self,
        linked_entity_result: Result<&T, &Identifier>,
        span: &Span,
    ) {
        if let Ok(entity) = linked_entity_result {
            if self.search_location.is_within(span) {
                self.found_span = Some(entity.raw_identifier().span.clone());
            };
        }
    }
}

impl Visitor for JumpVisitor {
    fn visit_file(&mut self, _: &SliceFile) {}

    fn visit_module(&mut self, _: &Module) {}

    fn visit_struct(&mut self, struct_def: &Struct) {
        self.check_comment(struct_def);
    }

    fn visit_class(&mut self, class_def: &Class) {
        self.check_comment(class_def);
        if let Some(base_ref) = &class_def.base {
            if self.search_location.is_within(&base_ref.span) {
                self.found_span = Some(base_ref.definition().raw_identifier().span.clone());
            }
        }
    }

    fn visit_exception(&mut self, exception_def: &Exception) {
        self.check_comment(exception_def);
        if let Some(base_ref) = &exception_def.base {
            if self.search_location.is_within(&base_ref.span) {
                self.found_span = Some(base_ref.definition().raw_identifier().span.clone());
            }
        }
    }

    fn visit_interface(&mut self, interface_def: &Interface) {
        self.check_comment(interface_def);
        interface_def.bases.iter().for_each(|base_ref| {
            if self.search_location.is_within(&base_ref.span) {
                self.found_span = Some(base_ref.definition().raw_identifier().span.clone());
            };
        })
    }

    fn visit_enum(&mut self, enum_def: &Enum) {
        self.check_comment(enum_def);
    }

    fn visit_operation(&mut self, operation_def: &Operation) {
        self.check_comment(operation_def);
        operation_def
            .exception_specification
            .iter()
            .for_each(|base_ref| {
                if self.search_location.is_within(&base_ref.span) {
                    self.found_span = Some(base_ref.definition().raw_identifier().span.clone());
                };
            })
    }

    fn visit_custom_type(&mut self, custom_type_def: &CustomType) {
        self.check_comment(custom_type_def);
    }

    fn visit_type_alias(&mut self, type_alias_def: &TypeAlias) {
        self.check_comment(type_alias_def);
    }

    fn visit_field(&mut self, field_def: &Field) {
        self.check_comment(field_def);
    }

    fn visit_parameter(&mut self, _: &Parameter) {}

    fn visit_enumerator(&mut self, enumerator_def: &Enumerator) {
        self.check_comment(enumerator_def);
    }

    fn visit_type_ref(&mut self, typeref_def: &TypeRef) {
        if self.search_location.is_within(&typeref_def.span) {
            let TypeRefDefinition::Patched(type_def) = &typeref_def.definition else {
                return;
            };
            let result: Option<&dyn Entity> = match type_def.borrow().concrete_type() {
                Types::Struct(x) => Some(x),
                Types::Class(x) => Some(x),
                Types::Interface(x) => Some(x),
                Types::Enum(x) => Some(x),
                Types::CustomType(x) => Some(x),
                _ => None,
            };
            self.found_span = result.and_then(|e| Some(e.raw_identifier().span.clone()));
        }
    }
}
