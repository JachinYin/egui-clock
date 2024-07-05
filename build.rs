extern crate embed_resource;

fn main() {
    // embed_resource::compile("app-manifest.rc", embed_resource::NONE);
    embed_resource::compile("./assets/icon.rc", embed_resource::NONE);
}