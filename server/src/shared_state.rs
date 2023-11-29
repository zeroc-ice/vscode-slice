// Copyright (c) ZeroC, Inc.

use slicec::compilation_state::CompilationState;

#[derive(Debug)]
pub struct SharedState {
    pub compilation_state: CompilationState,
}

impl SharedState {
    pub fn new() -> Self {
        Self {
            compilation_state: CompilationState::create(),
        }
    }
}
