use glyphon::FontSystem;

fn main() {
    let mut db = FontSystem::new();
    let font_path = "assets/fonts/MPLUS2-VariableFont_wght.ttf";

    println!("Loading font from: {}", font_path);

    // Load the font file
    db.db_mut()
        .load_font_file(font_path)
        .expect("Failed to load font file");

    // Iterate over all faces
    println!("Font families found:");
    for face in db.db().faces() {
        for (family, _) in face.families.iter() {
            println!("  - {}", family);
        }
    }
}
