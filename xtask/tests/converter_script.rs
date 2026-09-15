//! The Blender script's decisions, taken without Blender.
//!
//! `xtask/blender/fbx_to_gltf.py` is the one part of this pipeline that
//! no other guard can reach. `tests/pipeline.rs` drives the resolver
//! against stand-in converters and proves what is HANDED to a converter;
//! what happens after the arguments arrive has, until now, been proved
//! nowhere, because there is no Blender in continuous integration or in
//! the container this was written in.
//!
//! That gap shipped three grey crates. The first was an atlas nobody
//! handed over. The second was an atlas handed over and thrown away: a
//! Synty FBX names its texture by the path of the tree it was exported
//! from, Blender answers a reference it cannot resolve with an empty
//! placeholder image, and the script's skip rule — "leave alone any
//! material that already names an image" — read that placeholder as a
//! material that knew its own texture. The third was the second a level
//! further in: the rewritten rule asked the placeholder whether its
//! pixels were in memory, and Blender 5.0's placeholder says they are,
//! at nought by nought. Each conversion succeeded, printed its
//! measurement, wrote a file the same size as the colourless one it
//! replaced, and was found out by somebody looking at a grey box.
//!
//! The fourth was not grey. A floor lamp came out lit all over, at the
//! brightness of its own paint, because its FBX named the pack's emissive
//! atlas as well as the colour one — both by Synty's own paths — and the
//! rebind painted the colour atlas onto both placeholders alike. A colour
//! is not a light.
//!
//! None of the four was in Blender. All were in the script's control
//! flow, which is a thing a fake `bpy` can execute — see
//! `tests/fixtures/blender/bpy.py`, which builds a scene, records every
//! image binding and node the script makes, and lets a guard read the
//! decisions back. The third is also the reason the fake answers `size`
//! the way Blender does rather than the way the script hoped: a fake
//! that encodes the assumption under test proves nothing about it.
//!
//! **What this proves and what it cannot.** Proved here: that a broken
//! reference is repainted rather than skipped, that a placeholder which
//! claims data and measures nought by nought is repainted too, that the
//! repaint reuses the importer's own node instead of building a second
//! one beside it, that a reference which resolves is left alone, that a
//! conversion handed an atlas and painting nothing refuses and writes no
//! file, that a conversion handed no atlas is unchanged, that a broken
//! emissive reference is never fed the colour atlas — unwired when no
//! emissive was declared, rebound to the declared one when it was — and
//! that a material naming no emissive gets one built for it. Not proved
//! here, and not provable without the owner's disk: that a repainted
//! material exports as a textured glTF. `docs/ART_PIPELINE.md` names the
//! command that asks that.

use std::path::{Path, PathBuf};
use std::process::Command;

/// The interpreter, or nothing.
///
/// Python is not a dependency of this repository and is not going to
/// become one — `xtask/Cargo.toml` says why the errand runner is Rust.
/// It is, however, on every machine that has Blender, since Blender is
/// most of a Python distribution. A machine with neither is told that
/// these guards did not run, the same way the absence of a converter is
/// a sentence rather than a failure.
fn python() -> Option<&'static str> {
    ["python3", "python"].into_iter().find(|name| {
        Command::new(name)
            .arg("--version")
            .output()
            .is_ok_and(|out| out.status.success())
    })
}

struct Ran {
    ok: bool,
    said: String,
    glb: PathBuf,
}

/// Run the real converter script against the fake Blender, in `scene`,
/// with or without a texture argument, and with or without the emissive
/// that rides behind it.
///
/// `None` when there is no interpreter, which every guard below reports
/// and skips rather than passing quietly.
fn drive(name: &str, scene: &str, texture: bool, emissive: bool) -> Option<Ran> {
    let python = python()?;
    let xtask = Path::new(env!("CARGO_MANIFEST_DIR"));
    let dir = std::env::temp_dir().join(format!("space-trucking-script-{name}"));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).expect("a scratch directory");

    let source = dir.join("SM_Prop_Crate_01.fbx");
    let glb = dir.join("out.glb");
    let atlas = dir.join("atlas.png");
    let lit = dir.join("emissive.png");
    // The one scene that needs an image reference which actually
    // resolves needs a file for it to resolve to.
    let already_there = dir.join("already_there.png");
    for (path, contents) in [
        (&source, "a crate"),
        (&atlas, "the atlas the whole pack is painted from"),
        (&lit, "the emissive atlas beside it"),
        (&already_there, "a texture the FBX knew the way to"),
    ] {
        std::fs::write(path, contents).expect("a fixture file");
    }

    let mut command = Command::new(python);
    command
        .arg(xtask.join("blender/fbx_to_gltf.py"))
        .arg("--")
        .arg(&source)
        .arg(&glb);
    if texture {
        command.arg(&atlas);
    }
    // Positional, behind the atlas: the resolver refuses a lone one.
    if texture && emissive {
        command.arg(&lit);
    }
    command
        .env("PYTHONPATH", xtask.join("tests/fixtures/blender"))
        // Nothing may be left in the source tree by running a guard.
        .env("PYTHONDONTWRITEBYTECODE", "1")
        .env("FAKE_BLENDER_SCENE", scene)
        .env("FAKE_BLENDER_LOADED", &already_there);
    let output = command.output().expect("the script runs");
    let mut said = String::from_utf8_lossy(&output.stdout).into_owned();
    said.push_str(&String::from_utf8_lossy(&output.stderr));
    Some(Ran {
        ok: output.status.success(),
        said,
        glb,
    })
}

/// Say that a guard did not run, rather than letting it read as passed.
macro_rules! script {
    ($name:literal, $scene:literal, $texture:literal) => {
        script!($name, $scene, $texture, false)
    };
    ($name:literal, $scene:literal, $texture:literal, $emissive:literal) => {
        match drive($name, $scene, $texture, $emissive) {
            Some(ran) => ran,
            None => {
                println!("no python3: the converter script's decisions went unchecked");
                return;
            }
        }
    };
}

/// **A material whose texture reference is broken counts as unpainted.**
///
/// The defect, exactly. Synty ship their FBX files with the texture
/// paths of the machine they were exported on — often a `.psd` that is
/// not in the pack at all — and Blender's importer answers a path it
/// cannot find with a placeholder image datablock: a name, a filepath
/// pointing nowhere, no pixels. `node.image is not None` is true of that
/// placeholder, so the old skip rule left the material exactly as it
/// was, and the crate rendered grey with the declared atlas sitting
/// unused beside it.
///
/// A reference is not knowledge. The rule now asks whether the image
/// could be LOADED, and a material whose every reference is a
/// placeholder is silence for the declaration to fill.
///
/// It is rebound onto the importer's own node rather than painted with a
/// new one, and that is the second half of the guard. The nodes, their
/// links and the UV coordinates feeding them are all exactly what the
/// FBX asked for; only the pixels are missing. A fresh Image Texture
/// node beside them would leave two claims on one Base Color input and
/// drop whatever the importer wired in between.
#[test]
fn a_broken_texture_reference_is_repainted_on_the_node_the_importer_built() {
    let ran = script!("broken", "broken_reference", true);
    assert!(ran.ok, "{}", ran.said);
    assert!(
        ran.said
            .contains("M_Crate/imported_diffuse image = atlas.png"),
        "a material naming a texture that is not on this machine kept it:\n{}",
        ran.said
    );
    assert!(
        !ran.said.contains("made ShaderNodeTexImage"),
        "a second image node was built beside the importer's wiring:\n{}",
        ran.said
    );
    assert!(ran.glb.is_file(), "{}", ran.said);
}

/// **A placeholder that says it holds data, and is nought by nought,
/// counts as unpainted.**
///
/// The third grey crate, and the second one a level further in. With
/// the atlas declared, staged and handed over, and the rebind above in
/// place, the same crate came out grey again — because Blender 5.0's
/// importer answers the same unresolvable `.psd` reference with a
/// placeholder whose `has_data` is true and whose size is 0×0, and the
/// skip rule took the flag at its word. The refusal, asking the same
/// question, agreed with it. A flag is not pixels: the rule now asks
/// the size, which only the file can answer.
#[test]
fn a_placeholder_that_claims_data_and_has_no_pixels_is_repainted() {
    let ran = script!("claims-data", "placeholder_claiming_data", true);
    assert!(ran.ok, "{}", ran.said);
    assert!(
        ran.said
            .contains("M_Crate/imported_diffuse image = atlas.png"),
        "a placeholder with `has_data` set and no pixels was taken for a texture that loaded:\n{}",
        ran.said
    );
    assert!(
        !ran.said.contains("made ShaderNodeTexImage"),
        "a second image node was built beside the importer's wiring:\n{}",
        ran.said
    );
    assert!(ran.glb.is_file(), "{}", ran.said);
}

/// **A Blender that has shed `use_nodes` is painted, not crashed.**
///
/// Blender 5.0 deprecates `Material.use_nodes` — "expected to be removed
/// in Blender 6.0", when every material will simply have its node tree.
/// The script reads the flag in two places, and a bare read on that
/// Blender is an `AttributeError` where a conversion should be. So the
/// fake models the future: a tree already built, an image node holding
/// nothing, and no `use_nodes` to ask — and the script has to treat the
/// absence as "always nodes", repaint the node in place, and export.
#[test]
fn a_blender_that_has_shed_use_nodes_is_painted_not_crashed() {
    let ran = script!("shed-use-nodes", "always_nodes", true);
    assert!(ran.ok, "{}", ran.said);
    assert!(
        ran.said
            .contains("M_Crate/imported_diffuse image = atlas.png"),
        "a material on a Blender without `use_nodes` was not repainted:\n{}",
        ran.said
    );
    assert!(
        !ran.said.contains("made ShaderNodeTexImage"),
        "{}",
        ran.said
    );
    assert!(ran.glb.is_file(), "{}", ran.said);
}

/// **An image node holding no image at all is repainted the same way.**
/// The other shape a broken reference takes, depending on which importer
/// answered — and Blender 5.0 dropped the `io_scene_fbx` add-on that
/// provided the older one, so nothing here may assume which did.
#[test]
fn an_image_node_holding_no_image_is_repainted_in_place() {
    let ran = script!("no-image", "image_node_without_image", true);
    assert!(ran.ok, "{}", ran.said);
    assert!(
        ran.said
            .contains("M_Crate/imported_diffuse image = atlas.png"),
        "an image node with nothing on it was taken for a material that knew its own texture:\n{}",
        ran.said
    );
    assert!(
        !ran.said.contains("made ShaderNodeTexImage"),
        "{}",
        ran.said
    );
}

/// **A material whose texture reference resolves is left exactly
/// alone.**
///
/// The boundary the whole fix has to respect. The manifest's `texture`
/// line is a fallback for the FBX files that name nothing usable, not a
/// correction to the ones that name something real — an FBX that knew
/// where its texture was still wins, and the staged copies beside the
/// mesh are what it wins against. Widen the repaint past broken
/// references and the declaration stops filling a silence and starts
/// overruling a statement.
#[test]
fn a_material_whose_texture_reference_resolves_is_left_alone() {
    let ran = script!("loaded", "loaded_reference", true);
    assert!(ran.ok, "{}", ran.said);
    assert!(
        !ran.said.contains("imported_diffuse image ="),
        "a material that knew where its own texture was had it overwritten:\n{}",
        ran.said
    );
    assert!(
        !ran.said.contains("made ShaderNodeTexImage"),
        "{}",
        ran.said
    );
    assert!(ran.glb.is_file(), "{}", ran.said);
}

/// **A material with no image node at all still gets one made for it.**
/// The original Synty case, which the repaint above must not have broken:
/// materials assigned in Unity through the `.unitypackage`'s `.mat`
/// files leave an FBX carrying a material that names nothing whatever.
#[test]
fn a_material_naming_nothing_still_gets_the_atlas_built_for_it() {
    let ran = script!("bare", "bare_material", true);
    assert!(ran.ok, "{}", ran.said);
    assert!(ran.said.contains("made ShaderNodeTexImage"), "{}", ran.said);
    assert!(
        ran.said
            .contains("linked ShaderNodeTexImage.Color -> Principled BSDF.Base Color"),
        "the atlas was built and not plugged in:\n{}",
        ran.said
    );
    assert!(ran.glb.is_file(), "{}", ran.said);
}

/// **A conversion handed an atlas that reached no material refuses, and
/// writes nothing.**
///
/// Silence becoming refusal, which is the part of this that outlives the
/// particular defect. Every grey crate was a conversion that SUCCEEDED —
/// exit zero, a measurement printed, a file of an entirely plausible
/// size — and a `.glb` with no image in it is a perfectly valid `.glb`,
/// so nothing between the converter and the cabin could tell. The script
/// is the only program in the pipeline that can see a material, so the
/// check belongs here and nowhere else — and it asks `usable_image`, so
/// it is exactly as sharp as that question and no sharper.
#[test]
fn an_atlas_that_reached_no_material_is_refused_rather_than_exported() {
    let ran = script!("refusal", "withheld_node_tree", true);
    assert!(
        !ran.ok,
        "a conversion that painted nothing succeeded:\n{}",
        ran.said
    );
    assert!(
        ran.said.contains("the declared atlas reached no material"),
        "{}",
        ran.said
    );
    assert!(
        ran.said.contains("atlas.png"),
        "the refusal does not name the atlas:\n{}",
        ran.said
    );
    assert!(
        !ran.glb.exists(),
        "a grey .glb was written and then complained about"
    );
}

/// **A conversion handed no texture is unchanged, and refuses nothing.**
///
/// The refusal is about a declaration that was made and came to nothing.
/// A manifest with no `texture` line has declared nothing, its FBX is
/// left to resolve whatever it names against the copies staged beside
/// it, and a grey result there is a manifest that has not been finished
/// rather than a converter that failed.
#[test]
fn a_conversion_handed_no_texture_paints_nothing_and_refuses_nothing() {
    let ran = script!("silent", "broken_reference", false);
    assert!(ran.ok, "{}", ran.said);
    assert!(
        !ran.said.contains("loaded atlas.png"),
        "an atlas nobody declared was loaded anyway:\n{}",
        ran.said
    );
    assert!(
        !ran.said.contains("reached no material"),
        "a manifest that declared no texture was refused for not using one:\n{}",
        ran.said
    );
    assert!(ran.glb.is_file(), "{}", ran.said);
    assert!(
        ran.said.contains("aabb "),
        "the measurement stopped being printed:\n{}",
        ran.said
    );
}

/// **A colour is not a light: an emissive reference the FBX cannot
/// find is never painted with the colour atlas.**
///
/// The fourth defect. A fitting the pack lights names two atlases by
/// Synty's own paths, the importer leaves a placeholder on Base Color
/// and another on Emission Color, and the rebind — asked only "which
/// nodes hold no pixels?" — fed the colour atlas to both. The lamp came
/// out lit all over at the brightness of its own paint, which is a
/// conversion that succeeded like the three grey ones did.
///
/// With no `emissive` declared, the emission node is unwired and the
/// strength the importer set is put back to nought, so the mesh is
/// converted unlit; the colour node is rebound as before; and standard
/// error names the file the FBX asked for, because the cure is one line.
#[test]
fn a_broken_emissive_reference_is_unwired_rather_than_painted_with_the_colour_atlas() {
    let ran = script!("stray-emissive", "broken_emissive_reference", true);
    assert!(ran.ok, "{}", ran.said);
    assert!(
        ran.said
            .contains("MAT_01A/imported_diffuse image = atlas.png"),
        "the colour reference was not rebound:\n{}",
        ran.said
    );
    assert!(
        !ran.said.contains("imported_emissive image = atlas.png"),
        "the colour atlas was painted onto the emission node — a lamp lit by its own paint:\n{}",
        ran.said
    );
    assert!(
        ran.said
            .contains("unlinked imported_emissive.Color -> Principled BSDF.Emission Color"),
        "the broken emission reference was left wired:\n{}",
        ran.said
    );
    assert!(
        ran.said
            .contains("MAT_01A/Principled BSDF.Emission Strength = 0.0"),
        "the strength the importer set was left on, over a colour that defaults to white:\n{}",
        ran.said
    );
    assert!(
        ran.said.contains("names an emissive it cannot find")
            && ran.said.contains("PolygonHorrorSpace_Emissive_01_A.png"),
        "nothing said which file the FBX asked for:\n{}",
        ran.said
    );
    assert!(ran.glb.is_file(), "{}", ran.said);
}

/// **A declared emissive is rebound onto the emission node the importer
/// built**, the way the colour atlas is rebound onto the colour one: the
/// wiring is what the FBX asked for, only the pixels are missing, and a
/// second node would be two claims on one socket.
#[test]
fn a_declared_emissive_is_rebound_onto_the_node_the_importer_built() {
    let ran = script!("rebound-emissive", "broken_emissive_reference", true, true);
    assert!(ran.ok, "{}", ran.said);
    assert!(
        ran.said
            .contains("MAT_01A/imported_emissive image = emissive.png"),
        "the declared emissive did not reach the importer's emission node:\n{}",
        ran.said
    );
    assert!(
        ran.said
            .contains("MAT_01A/imported_diffuse image = atlas.png"),
        "the colour reference was not rebound beside it:\n{}",
        ran.said
    );
    assert!(
        !ran.said.contains("made ShaderNodeTexImage"),
        "a second image node was built beside the importer's wiring:\n{}",
        ran.said
    );
    assert!(
        !ran.said.contains("unlinked"),
        "a reference the declaration was about to fill was unwired instead:\n{}",
        ran.said
    );
    assert!(ran.glb.is_file(), "{}", ran.said);
}

/// **A material naming no emissive gets one built for it, and switched
/// on.** The common Synty case — emission assigned in Unity, the FBX
/// silent — with the trap `light_material` was written around: Blender
/// 4.x defaults Emission Strength to nought, and a texture into a colour
/// multiplied by nothing exports a dark mesh that reports success.
#[test]
fn a_material_naming_no_emissive_gets_one_built_and_switched_on() {
    let ran = script!("built-emissive", "bare_material", true, true);
    assert!(ran.ok, "{}", ran.said);
    assert!(
        ran.said
            .contains("linked ShaderNodeTexImage.Color -> Principled BSDF.Emission Color"),
        "no emission node was wired for a material that named none:\n{}",
        ran.said
    );
    assert!(
        ran.said
            .contains("M_Crate/Principled BSDF.Emission Strength = 1.0"),
        "the emission was wired and left at a strength of nought:\n{}",
        ran.said
    );
    assert!(ran.glb.is_file(), "{}", ran.said);
}
