//! The look of the app at rest is kept as pictures on disk, and a change that moves a pixel by
//! more than the tolerance fails here until a person looks at the new picture and approves it.

mod rows;
mod steps;

use egui_kittest::SnapshotOptions;

use rows::ROWS;

const TOLERANCE: f32 = 0.6;
const FOLDER: &str = "tests/snapshots";

#[test]
fn every_resting_scene_looks_as_it_was_approved() {
    let options = SnapshotOptions::new()
        .threshold(TOLERANCE)
        .output_path(FOLDER);
    let differing: Vec<String> = ROWS
        .iter()
        .filter_map(|row| row.compare(&options).err())
        .collect();
    assert!(
        differing.is_empty(),
        "{} of {} pictures differ from the approved ones:\n{}",
        differing.len(),
        ROWS.len(),
        differing.join("\n")
    );
}
