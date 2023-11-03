// Copyright (c) ZeroC, Inc.

use slicec::{compilation_state::CompilationState, slice_options::SliceOptions};

#[derive(Debug)]
pub struct SharedState {
    pub compilation_state: CompilationState,
    pub compilation_options: SliceOptions,
}

impl SharedState {
    pub fn new() -> Self {
        Self {
            compilation_state: CompilationState::create(),
            compilation_options: SliceOptions::default(),
        }
    }
}
