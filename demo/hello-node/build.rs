fn main() {
    // Tells the linker to leave Node's symbols unresolved; they are supplied
    // by the host process when the addon is loaded.
    napi_build::setup();
}
