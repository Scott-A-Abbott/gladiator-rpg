mod ecs;

use godot::prelude::{gdextension, ExtensionLibrary};

struct RustExt;

#[gdextension]
unsafe impl ExtensionLibrary for RustExt {}
