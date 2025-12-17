// Copyright (c) ZeroC, Inc.

use crate::utils::position_to_location;
use slicec::grammar::{Element, Enum, Primitive, Symbol, TypeRef, TypeRefDefinition, Types};
use slicec::slice_file::{Location, SliceFile};
use slicec::visitor::Visitor;
use tower_lsp::lsp_types::Position;

pub fn get_hover_message(file: &SliceFile, position: Position) -> Option<String> {
    let mut visitor = HoverVisitor::new(position_to_location(position));
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

    fn construct_message<T: Element + ?Sized>(primitive: &Primitive, typeref: &TypeRef<T>) -> String {
        let (prefix, description) = Self::describe_primitive_type(primitive);
        if typeref.is_optional {
            format!("An optional {description}")
        } else {
            format!("{prefix} {description}")
        }
    }

    fn describe_primitive_type(primitive_type: &Primitive) -> (&'static str, &'static str) {
        match primitive_type {
            Primitive::Bool => ("A", "boolean type."),
            Primitive::Int8 => ("An", "8-bit signed integer type."),
            Primitive::UInt8 => ("An", "8-bit unsigned integer type."),
            Primitive::Int16 => ("A", "16-bit signed integer type."),
            Primitive::UInt16 => ("A", "16-bit unsigned integer type."),
            Primitive::Int32 => ("A", "32-bit signed integer type."),
            Primitive::UInt32 => ("A", "32-bit unsigned integer type."),
            Primitive::VarInt32 => ("A", "variable-length signed integer type."),
            Primitive::VarUInt32 => ("A", "variable-length unsigned integer type."),
            Primitive::Int64 => ("A", "64-bit signed integer type."),
            Primitive::UInt64 => ("A", "64-bit unsigned integer type."),
            Primitive::VarInt62 => ("A", "variable-length signed integer type."),
            Primitive::VarUInt62 => ("A", "variable-length unsigned integer type."),
            Primitive::Float32 => ("A", "32-bit floating point type."),
            Primitive::Float64 => ("A", "64-bit floating point type."),
            Primitive::String => ("A", "UTF-8 string."),
            Primitive::AnyClass => ("A", "instance of any Slice class."),
        }
    }
}

impl Visitor for HoverVisitor {
    fn visit_enum(&mut self, enum_def: &Enum) {
        if let Some(underlying) = &enum_def.underlying {
            if !&self.search_location.is_within(underlying.span()) {
                return;
            }
            if let Some(underlying_def) = &enum_def.underlying {
                let TypeRefDefinition::Patched(definition) = &underlying_def.definition else {
                    return;
                };
                self.found_message = Some(Self::construct_message(definition.borrow(), underlying))
            }
        }
    }

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
            Types::Primitive(x) => Some(Self::construct_message(x, typeref)),
            _ => None,
        };
        self.found_message = type_description;
    }
}
