use slicec::{
    grammar::{
        Class, CustomType, Entity, Enum, Enumerator, Exception, Field, Interface, Module,
        Operation, Parameter, Struct, TypeAlias, TypeRef, TypeRefDefinition, Types,
    },
    slice_file::{Location, SliceFile, Span},
    visitor::Visitor,
};

pub struct LspVisitor {
    pub search_location: Location,
    pub found_span: Option<Span>,
}

impl LspVisitor {
    pub fn new(search_location: Location) -> Self {
        LspVisitor {
            search_location,
            found_span: None,
        }
    }

    fn is_location_in_span(&self, location: &Location, span: &Span) -> bool {
        let start = &span.start;
        let end = &span.end;

        location.row >= start.row
            && location.row <= end.row
            && location.col >= start.col
            && location.col <= end.col
    }
}

impl Visitor for LspVisitor {
    fn visit_file(&mut self, _: &SliceFile) {}

    fn visit_module(&mut self, _: &Module) {}

    fn visit_struct(&mut self, _: &Struct) {}

    fn visit_class(&mut self, class_def: &Class) {
        if let Some(base_ref) = &class_def.base {
            if self.is_location_in_span(&self.search_location, &base_ref.span) {
                self.found_span = Some(base_ref.definition().span.clone());
            }
        }
    }

    fn visit_exception(&mut self, exception_def: &Exception) {
        if let Some(base_ref) = &exception_def.base {
            if self.is_location_in_span(&self.search_location, &base_ref.span) {
                self.found_span = Some(base_ref.definition().span.clone());
            }
        }
    }

    fn visit_interface(&mut self, interface_def: &Interface) {
        interface_def.bases.iter().for_each(|base_ref| {
            if self.is_location_in_span(&self.search_location, &base_ref.span) {
                self.found_span = Some(base_ref.definition().span.clone());
            };
        })
    }

    fn visit_enum(&mut self, _: &Enum) {}

    fn visit_operation(&mut self, _: &Operation) {}

    fn visit_custom_type(&mut self, _: &CustomType) {}

    fn visit_type_alias(&mut self, _: &TypeAlias) {}

    fn visit_field(&mut self, _: &Field) {}

    fn visit_parameter(&mut self, _: &Parameter) {}

    fn visit_enumerator(&mut self, _: &Enumerator) {}

    fn visit_type_ref(&mut self, typeref: &TypeRef) {
        if self.is_location_in_span(&self.search_location, &typeref.span) {
            let TypeRefDefinition::Patched(type_def) = &typeref.definition else {
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
            self.found_span = result.and_then(|e| Some(e.span())).cloned()
        }
    }
}
