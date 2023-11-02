// Copyright (c) ZeroC, Inc.

use slicec::{
    compilation_state::CompilationState,
    grammar::{
        Class, CustomType, Enum, Enumerator, Exception, Field, Interface, Module, Operation,
        Parameter, Primitive, Struct, Symbol, TypeAlias, TypeRef, TypeRefDefinition, Types,
    },
    slice_file::{Location, SliceFile},
    visitor::Visitor,
};
use tower_lsp::lsp_types::{Position, Url};

pub fn get_hover_info(state: &CompilationState, uri: Url, position: Position) -> Option<String> {
    // Attempt to convert the URL to a file path and then to a string
    let file_path = uri.to_file_path().ok()?.to_str()?.to_owned();

    // Attempt to retrieve the file from the state
    let file = state.files.get(&file_path)?;

    // Convert position to row and column 1 based
    let col = (position.character + 1) as usize;
    let row = (position.line + 1) as usize;

    let mut visitor = HoverVisitor::new(slicec::slice_file::Location { row, col });
    file.visit_with(&mut visitor);

    visitor.found_message
}

struct HoverVisitor {
    pub search_location: Location,
    pub found_message: Option<String>,
}

impl HoverVisitor {
    pub fn new(search_location: Location) -> Self {
        HoverVisitor {
            search_location,
            found_message: None,
        }
    }

    fn describe_primitive_type(primitive_type: &Primitive) -> &'static str {
        match primitive_type {
            Primitive::Bool => "The boolean type.",
            Primitive::Int8 => "The 8-bit signed integer type.",
            Primitive::UInt8 => "The 8-bit unsigned integer type.",
            Primitive::Int16 => "The 16-bit signed integer type.",
            Primitive::UInt16 => "The 16-bit unsigned integer type.",
            Primitive::Int32 => "The 32-bit signed integer type.",
            Primitive::UInt32 => "The 32-bit unsigned integer type.",
            Primitive::VarInt32 => "The variable-length signed integer type.",
            Primitive::VarUInt32 => "The variable-length unsigned integer type.",
            Primitive::Int64 => "The 64-bit signed integer type.",
            Primitive::UInt64 => "The 64-bit unsigned integer type.",
            Primitive::VarInt62 => "The variable-length signed integer type.",
            Primitive::VarUInt62 => "The variable-length unsigned integer type.",
            Primitive::Float32 => "The 32-bit floating point type.",
            Primitive::Float64 => "The 64-bit floating point type.",
            Primitive::String => "A UTF-8 string.",
            Primitive::AnyClass => "An instance of any Slice class.",
        }
    }
}

impl Visitor for HoverVisitor {
    fn visit_file(&mut self, _: &SliceFile) {}

    fn visit_module(&mut self, _: &Module) {}

    fn visit_struct(&mut self, _: &Struct) {}

    fn visit_class(&mut self, _: &Class) {}

    fn visit_exception(&mut self, _: &Exception) {}

    fn visit_interface(&mut self, _: &Interface) {}

    fn visit_enum(&mut self, enum_def: &Enum) {
        if let Some(underlying) = &enum_def.underlying {
            if !&self.search_location.is_within(underlying.span()) {
                return;
            }
            Some(HoverVisitor::describe_primitive_type(underlying.definition()).to_owned());
        }
    }

    fn visit_operation(&mut self, _: &Operation) {}

    fn visit_custom_type(&mut self, _: &CustomType) {}

    fn visit_type_alias(&mut self, _: &TypeAlias) {}

    fn visit_field(&mut self, _: &Field) {}

    fn visit_parameter(&mut self, _: &Parameter) {}

    fn visit_enumerator(&mut self, _: &Enumerator) {}

    fn visit_type_ref(&mut self, typeref: &TypeRef) {
        if self.found_message.is_some() {
            return;
        }
        if !&self.search_location.is_within(typeref.span()) {
            return;
        }
        let TypeRefDefinition::Patched(type_def) = &typeref.definition else {
            return;
        };

        let type_description = match type_def.borrow().concrete_type() {
            Types::Primitive(x) => Some(HoverVisitor::describe_primitive_type(x).to_owned()),
            _ => None,
        };

        self.found_message = type_description;
    }
}
