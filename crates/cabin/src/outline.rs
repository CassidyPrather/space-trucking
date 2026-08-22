//! **The outline, drawn round the body's own edge.**
//!
//! A highlight in this cabin says one of three things — the crosshair is
//! on this, the room has claimed it, you have asked for it — and it has
//! always said them by drawing round the piece rather than painting the
//! ground under it. What it drew round was a BOX: twelve bars off
//! `pieces::drawn_box`, cut into brackets, a ring or dashes. A box round
//! a body is not an outline of a body, and the difference shows on
//! anything that is not a crate.
//!
//! So this is the thing the editors do. The piece is drawn a second time
//! into a **mask** — the alpha channel of the crunch target, and nothing
//! else — and a full-screen pass finds the mask's edge and paints the
//! outline a fixed number of crunch texels outside it. What comes out
//! follows the silhouette of whatever geometry was there, which is the
//! whole reason for the technique: the day a purchased mesh replaces a
//! hand-rolled `Cuboid`, the outline is re-cut with it and nothing here
//! is told.
//!
//! ## Where the mask lives, and why it costs no pass of its own
//!
//! The obvious build is a second camera rendering the selection to its
//! own target. That is an extra render pass, an extra clear, and — worse
//! — a mask with no depth in it, so the outline draws through walls.
//!
//! The mask rides in the crunch target's **alpha** instead, written by
//! proxy meshes drawn in the cabin camera's own pass ([`MaskInk`], whose
//! pipeline writes `ColorWrites::ALPHA` and no depth at all). Three
//! things fall out of that and each one is load-bearing:
//!
//! - **Occlusion is free and correct.** A proxy is depth-tested against
//!   the very frame it rides in, so a bollard standing in front of a
//!   crate takes a bite out of the crate's mask and the outline closes
//!   round the bite — which is what a partly hidden thing looks like in
//!   Blender and in Unity, and what a box vocabulary cannot do at all.
//! - **The alpha channel is genuinely free.** Every opaque surface in
//!   this engine writes `a = 1`, and every blend mode the cabin uses
//!   composites alpha as `src.a + dst.a·(1−src.a)`, which is 1 wherever
//!   the destination was already 1. Bloom's composite leaves destination
//!   alpha alone and tonemapping hands `hdr.a` straight through. So the
//!   scene arrives at the crunch with alpha 1 everywhere, and anything
//!   below 1 is this module's.
//! - **Every reading a piece can wear fits in one channel.** The whole
//!   of what is said about a rig is one small number — which of five
//!   things the aim is doing, plus the room's two claims — and it is
//!   sent as `code / 64`, which lands four of the channel's 256 steps
//!   apart and never comes near the 1 the scene writes.
//!
//! ## Three readings, three forms, one line
//!
//! An edge detect gives one line, and the bars this replaces were
//! telling three sentences apart by shape. They still are, by the two
//! things a screen-space line has that a bar does not: **which side of
//! the body's own edge it is on**, and **whether it is whole**.
//!
//! | Reading | Form |
//! | --- | --- |
//! | aim | a thin line on the body's rim, **inside** it |
//! | mark | a **broken** line hugging the outside |
//! | offer | a **thick whole** line standing off outside both |
//!
//! Each is legible alone — the aim sits on the thing, the mark is
//! visibly in pieces, the offer stands off with clear air inside it —
//! and worn together they nest in that order. Hue does none of the
//! telling, the same as everywhere else in this cabin; what hue carries
//! is *which* aim, and the carry's ruling has a shape channel of its own
//! (`pieces::carry_slash`).

use bevy::asset::uuid_handle;
use bevy::camera::visibility::RenderLayers;
use bevy::camera::{ClearColorConfig, RenderTarget, ScalingMode};
use bevy::core_pipeline::tonemapping::{DebandDither, Tonemapping};
use bevy::image::ImageSampler;
use bevy::mesh::MeshVertexBufferLayoutRef;
use bevy::pbr::{Material, MaterialPipeline, MaterialPipelineKey, MaterialPlugin};
use bevy::prelude::*;
use bevy::render::render_resource::{
    AsBindGroup, ColorWrites, RenderPipelineDescriptor, ShaderType, SpecializedMeshPipelineError,
    TextureFormat,
};
use bevy::shader::{Shader, ShaderRef};

use crate::palette;
use crate::{Phase, Shell};

/// The layer the composite quad lives on: its own, so the cabin camera
/// never sees the thing that is drawing the cabin camera's own picture.
const COMPOSITE_LAYER: usize = 30;

/// **What a rig may be wearing, as one small integer.** The three
/// standing readings are independent — a good can be offered, asked
/// for, and under the crosshair at once — so they are BITS. What the
/// aim is doing is one of five things at a time, so that is a NUMBER in
/// the low bits.
///
/// | | |
/// | --- | --- |
/// | 1 | the crosshair rests on this |
/// | 2 | …and on the amber it is carried by |
/// | 3 | it is in your hands and it would land |
/// | 4 | it is in your hands and it would not |
/// | 5 | you are flying through it |
const HOVER: u8 = 1;
const HANDLE: u8 = 2;
const CARRY_OK: u8 = 3;
const CARRY_NO: u8 = 4;
const GHOST: u8 = 5;
/// The room has claimed this, and the player has asked for it.
const OFFER: u8 = 8;
const MARK: u8 = 16;

/// The highest code any of that can add up to, and the number of inks
/// the pool therefore holds.
const CODES: usize = (GHOST | OFFER | MARK) as usize;

/// How a code travels in eight bits of alpha: sixty-fourths, so the
/// largest of them is under half and every one of them is four of the
/// channel's 256 steps clear of its neighbours — a margin the round trip
/// through a float target and back cannot spend.
const SCALE: f32 = 64.0;

/// **How far each form is drawn from the body's own edge**, as a band of
/// crunch texels, negative INSIDE the silhouette and positive outside.
/// The three are the whole of the vocabulary, and they are here rather
/// than in the shader so that a law can be stated about them
/// ([`tests::no_two_readings_draw_one_line_over_another`]).
///
/// - The **aim** is a thin line on the body's own rim, painted just
///   INSIDE it: the lightest reading, and the one that comes and goes
///   with where you happen to be looking, sits ON the thing.
/// - The **mark** is a **broken** line hugging the outside — a stub
///   reads as a mark on a thing.
/// - The **offer** is a **thick whole** line standing off outside both,
///   with clear air between it and the body: the strongest claim gets
///   the heaviest and most complete form, and a band round a thing is
///   not a remark about it.
///
/// **The outer two are bands and not hairlines, and that is arithmetic
/// rather than taste.** What the pass has is the distance to the nearest
/// masked TEXEL, which on a grid can only be one of √0, 1, √2, 2, √5,
/// √8, 3 — so a form cut one texel wide lights only where the nearest
/// mask happens to land on one of those, and a ring drawn that way comes
/// out in pieces. A band wide enough to hold several of them is closed
/// whatever the edge is doing.
const AIM_BAND: (f32, f32) = (-1.5, -0.5);
const MARK_BAND: (f32, f32) = (0.5, 1.5);
const OFFER_BAND: (f32, f32) = (1.5, 3.05);

/// How far the edge pass looks, in texels. **It must cover the outermost
/// band and no more**: a band reaching past the taps is a band drawn
/// only where the geometry happens to be kind, and every texel of the
/// picture pays for the area of this disc.
const REACH: i32 = 3;

/// How coarse the mark's chequer is, in texels. A dash has to survive
/// the crunch and then the window's upscale of it, so it is blocks and
/// not stipple.
const DASH: i32 = 3;

/// **A part of a rig that would be masked if anything were said about
/// its piece**, and whose piece that is. Every body a kind draws wears
/// one (`pieces::RigParts::mask`); none of them costs anything until
/// [`paint`] cuts the copy.
#[derive(Component, Clone, Copy)]
pub struct MaskBody {
    piece: u32,
    /// Whether the copy has been cut yet. **Cut once and kept**: a
    /// player looks at the same crate again, and a rig respawns with
    /// its piece anyway, so there is nothing to reclaim and a spawn per
    /// frame would be churn for its own sake.
    cut: bool,
}

impl MaskBody {
    /// A part of `piece`, not yet copied.
    #[must_use]
    pub const fn of(piece: u32) -> Self {
        Self { piece, cut: false }
    }
}

/// A body drawn a second time into the mask: whose piece it is, and
/// which part of that piece it is a copy of.
///
/// It names its part because it must answer to that part's own hiding
/// and to nothing above it. A rat's bite wedge that is not shown is not
/// outlined; a rig the focus is FLYING THROUGH has its whole body
/// hidden and had better still be outlined, because an outline round
/// nothing is the entire reading there.
#[derive(Component, Clone, Copy)]
pub struct MaskProxy {
    pub piece: u32,
    pub part: Entity,
}

/// Which pieces the x-ray is ghosting this frame, as `crate::pieces`
/// works it out. Read rather than re-derived: whether a body stands
/// between the eye and the panel it is flying to is that module's
/// question and it already asks it.
#[derive(Resource, Default)]
pub struct Ghosts(pub Vec<u32>);

/// Every ink a proxy may wear, indexed by `code - 1`.
#[derive(Resource)]
pub struct MaskInks([Handle<MaskInk>; CODES]);

impl MaskInks {
    /// The ink carrying one code. A proxy is spawned wearing the aim's
    /// and hidden; [`paint`] hands it whichever the frame calls for.
    #[must_use]
    pub fn ink(&self, code: u8) -> Handle<MaskInk> {
        self.0[usize::from(code).clamp(1, CODES) - 1].clone()
    }
}

/// The crunch pipeline's two images: what the cabin renders into, and
/// what the window is shown. The outline pass is the step between them.
#[derive(Resource, Clone)]
pub struct Screen {
    pub crunch: Handle<Image>,
    pub shown: Handle<Image>,
}

impl Screen {
    /// Build both targets — the same recipe the crunch has always used,
    /// twice, because the pass between them has to read one and write
    /// the other and no texture may be both at once.
    pub fn build(images: &mut Assets<Image>, w: u32, h: u32) -> Self {
        let mut target = || {
            let mut image = Image::new_target_texture(
                w,
                h,
                TextureFormat::Rgba8Unorm,
                Some(TextureFormat::Rgba8UnormSrgb),
            );
            image.sampler = ImageSampler::nearest();
            images.add(image)
        };
        Self {
            crunch: target(),
            shown: target(),
        }
    }
}

/// The mask ink: a body painted into the alpha channel and nowhere else.
#[derive(Asset, TypePath, AsBindGroup, Clone)]
pub struct MaskInk {
    #[uniform(0)]
    code: Vec4,
}

impl Material for MaskInk {
    fn fragment_shader() -> ShaderRef {
        ShaderRef::Handle(MASK_SHADER.clone())
    }

    /// **The whole of the trick, in two lines.** Writing only alpha
    /// leaves the picture the cabin drew exactly as it drew it, and
    /// writing no depth leaves a proxy unable to hide the very body it
    /// is a copy of. What it still DOES do is read the depth buffer, so
    /// a proxy behind a bollard is discarded by the bollard.
    fn specialize(
        _pipeline: &MaterialPipeline,
        descriptor: &mut RenderPipelineDescriptor,
        _layout: &MeshVertexBufferLayoutRef,
        _key: MaterialPipelineKey<Self>,
    ) -> Result<(), SpecializedMeshPipelineError> {
        if let Some(fragment) = descriptor.fragment.as_mut() {
            for target in fragment.targets.iter_mut().flatten() {
                target.write_mask = ColorWrites::ALPHA;
            }
        }
        if let Some(depth) = descriptor.depth_stencil.as_mut() {
            depth.depth_write_enabled = Some(false);
        }
        Ok(())
    }
}

/// The colours the composite pass paints each reading in. Hue does none
/// of the telling — the forms do — but a line has to be some colour, and
/// these are the ones the cabin was already saying these things in.
#[derive(Clone, ShaderType)]
struct Inks {
    /// Indexed by the aim's own number, with a dead entry at nought.
    aim: [Vec4; 6],
    offer: Vec4,
    mark: Vec4,
}

/// The composite ink: the crunch, plus the three colours the edge is
/// painted in.
#[derive(Asset, TypePath, AsBindGroup, Clone)]
struct OutlineInk {
    #[uniform(0)]
    inks: Inks,
    #[texture(1)]
    #[sampler(2)]
    crunch: Handle<Image>,
}

impl Material for OutlineInk {
    fn fragment_shader() -> ShaderRef {
        ShaderRef::Handle(EDGE_SHADER.clone())
    }
}

const MASK_SHADER: Handle<Shader> = uuid_handle!("a2f0d3e4-0b71-4c88-9d21-6f2e5b1c7a10");
const EDGE_SHADER: Handle<Shader> = uuid_handle!("a2f0d3e4-0b71-4c88-9d21-6f2e5b1c7a11");

const MASK_WGSL: &str = r"
#import bevy_pbr::forward_io::VertexOutput

@group(#{MATERIAL_BIND_GROUP}) @binding(0) var<uniform> code: vec4<f32>;

@fragment
fn fragment(in: VertexOutput) -> @location(0) vec4<f32> {
    return vec4<f32>(0.0, 0.0, 0.0, code.x);
}
";

const EDGE_WGSL: &str = r#"
#import bevy_pbr::forward_io::VertexOutput

struct Inks {
    aim: array<vec4<f32>, 6>,
    offer: vec4<f32>,
    mark: vec4<f32>,
};

@group(#{MATERIAL_BIND_GROUP}) @binding(0) var<uniform> inks: Inks;
@group(#{MATERIAL_BIND_GROUP}) @binding(1) var crunch: texture_2d<f32>;
@group(#{MATERIAL_BIND_GROUP}) @binding(2) var crunch_sampler: sampler;

// The mask code at one texel: nought where the scene wrote its own
// opaque alpha, and the code otherwise.
fn code_at(px: vec2<i32>, size: vec2<i32>) -> u32 {
    let at = clamp(px, vec2<i32>(0, 0), size - vec2<i32>(1, 1));
    let a = textureLoad(crunch, at, 0).a;
    if (a > 0.7) {
        return 0u;
    }
    return u32(round(a * %scale%));
}

@fragment
fn fragment(in: VertexOutput) -> @location(0) vec4<f32> {
    let size = vec2<i32>(textureDimensions(crunch));
    let px = vec2<i32>(in.uv * vec2<f32>(size));
    let scene = textureLoad(crunch, clamp(px, vec2<i32>(0, 0), size - vec2<i32>(1, 1)), 0).rgb;
    let here = code_at(px, size);

    // How far the nearest texel carrying each reading is, in texels,
    // measured ROUND: a square reach would stand the corner of an
    // outline half again as far out as its flank, which reads as a box
    // creeping back into the picture that exists to stop being one.
    // Nine is "further than any form reaches".
    var aim = 9.0;
    var state = here & 7u;
    var offer = 9.0;
    var mark = 9.0;
    for (var dy: i32 = -%reach%; dy <= %reach%; dy = dy + 1) {
        for (var dx: i32 = -%reach%; dx <= %reach%; dx = dx + 1) {
            let span = f32(dx * dx + dy * dy);
            if (span > %reach2%) {
                continue;
            }
            let code = code_at(px + vec2<i32>(dx, dy), size);
            let d = sqrt(span);
            // The aim is painted on the body's own rim from the INSIDE,
            // so what it measures is how near this texel of the body
            // comes to not being the body any more.
            if (state != 0u && (code & 7u) != state) {
                aim = min(aim, d);
            }
            if ((code & 8u) != 0u) { offer = min(offer, d); }
            if ((code & 16u) != 0u) { mark = min(mark, d); }
        }
    }

    // The mark is broken by a coarse chequer of the screen's own texels,
    // so a dash reads as a dash whichever way the edge runs — a stripe
    // would vanish along every edge that ran with it.
    let block = px / vec2<i32>(%dash%, %dash%);
    let dash = ((block.x + block.y) & 1) == 0;

    // Outermost last, so three readings worn at once come out as three
    // forms in the order they are claimed in and never as one thick line.
    var out = scene;
    if (state != 0u && aim >= %aim_lo% && aim < %aim_hi%) { out = inks.aim[state].rgb; }
    if (mark >= %mark_lo% && mark < %mark_hi% && dash) { out = inks.mark.rgb; }
    if (offer >= %offer_lo% && offer < %offer_hi%) { out = inks.offer.rgb; }
    return vec4<f32>(out, 1.0);
}
"#;

/// The edge pass's source, with the vocabulary above written into it —
/// the bands are declared in Rust because a law is stated about them,
/// and a shader that carried its own copy of them could drift.
fn edge_wgsl() -> String {
    let mut wgsl = EDGE_WGSL.to_owned();
    for (name, count) in [("%reach%", REACH), ("%dash%", DASH)] {
        wgsl = wgsl.replace(name, &count.to_string());
    }
    for (name, distance) in [
        ("%scale%", SCALE),
        ("%reach2%", (REACH * REACH) as f32 + 0.05),
        ("%aim_lo%", -AIM_BAND.1),
        ("%aim_hi%", -AIM_BAND.0),
        ("%mark_lo%", MARK_BAND.0),
        ("%mark_hi%", MARK_BAND.1),
        ("%offer_lo%", OFFER_BAND.0),
        ("%offer_hi%", OFFER_BAND.1),
    ] {
        wgsl = wgsl.replace(name, &format!("{distance:.1}"));
    }
    wgsl
}

pub struct OutlinePlugin;

impl Plugin for OutlinePlugin {
    /// Everything the rest of the cabin has to be able to READ is made
    /// here rather than in a startup system: the two crunch targets the
    /// camera and the window node are built from, and the inks a rig's
    /// proxies are spawned wearing. A plugin's build runs before any
    /// startup system, so nothing downstream has to be ordered.
    fn build(&self, app: &mut App) {
        {
            let mut shaders = app.world_mut().resource_mut::<Assets<Shader>>();
            for (handle, source, name) in [
                (
                    &MASK_SHADER,
                    MASK_WGSL.to_owned(),
                    "cabin/outline_mask.wgsl",
                ),
                (&EDGE_SHADER, edge_wgsl(), "cabin/outline_edge.wgsl"),
            ] {
                shaders
                    .insert(handle.id(), Shader::from_wgsl(source, name))
                    .expect("a shader this module wrote itself");
            }
        }
        app.add_plugins((
            MaterialPlugin::<MaskInk>::default(),
            MaterialPlugin::<OutlineInk>::default(),
        ));
        let screen = {
            let mut images = app.world_mut().resource_mut::<Assets<Image>>();
            Screen::build(&mut images, crate::rig::CRUNCH_W, crate::rig::CRUNCH_H)
        };
        let inks = {
            let mut masks = app.world_mut().resource_mut::<Assets<MaskInk>>();
            MaskInks(std::array::from_fn(|i| {
                masks.add(MaskInk {
                    code: Vec4::new((i + 1) as f32 / SCALE, 0.0, 0.0, 0.0),
                })
            }))
        };
        app.insert_resource(screen)
            .insert_resource(inks)
            .init_resource::<Ghosts>()
            .add_systems(Startup, spawn)
            .add_systems(Update, paint.in_set(Phase::View));
    }
}

/// The composite pass: one quad, one camera, drawn after the cabin's own
/// and into the picture the window is actually shown.
fn spawn(
    mut commands: Commands,
    screen: Res<Screen>,
    mut meshes: ResMut<Assets<Mesh>>,
    mut inks: ResMut<Assets<OutlineInk>>,
) {
    let mut aim = [Vec4::ZERO; 6];
    aim[HOVER as usize] = palette::ICON_LIT.to_linear().to_vec4();
    // The handle rule's hover half: over a click-functional piece the
    // line reads amber where the crosshair is on the amber that carries
    // it, so the split is told before it is spent.
    aim[HANDLE as usize] = palette::AMBER.to_linear().to_vec4();
    aim[CARRY_OK as usize] = palette::LAMP_OK.to_linear().to_vec4();
    aim[CARRY_NO as usize] = palette::LAMP_NO.to_linear().to_vec4();
    // A body being flown through is not being told about; it is being
    // apologised to. Half the duty of the aim it borrows its hue from.
    aim[GHOST as usize] = palette::mix(palette::VOID, palette::ICON_LIT, GHOST_DUTY)
        .to_linear()
        .to_vec4();
    let ink = inks.add(OutlineInk {
        inks: Inks {
            aim,
            offer: palette::AMBER.to_linear().to_vec4(),
            mark: palette::AMBER.to_linear().to_vec4(),
        },
        crunch: screen.crunch.clone(),
    });
    let layer = RenderLayers::layer(COMPOSITE_LAYER);
    commands.spawn((
        Mesh3d(meshes.add(Rectangle::new(1.0, 1.0))),
        MeshMaterial3d(ink),
        Transform::default(),
        layer.clone(),
    ));
    commands.spawn((
        Camera3d::default(),
        Camera {
            order: 0,
            clear_color: ClearColorConfig::Custom(palette::VOID),
            ..default()
        },
        Projection::Orthographic(OrthographicProjection {
            scaling_mode: ScalingMode::Fixed {
                width: 1.0,
                height: 1.0,
            },
            ..OrthographicProjection::default_3d()
        }),
        RenderTarget::Image(screen.shown.clone().into()),
        Msaa::Off,
        Tonemapping::None,
        DebandDither::Disabled,
        Transform::from_xyz(0.0, 0.0, 1.0).looking_at(Vec3::ZERO, Vec3::Y),
        layer,
    ));
}

/// How bright the ghost's line stands against the void it is drawn on.
const GHOST_DUTY: f32 = 0.45;

/// **Say which reading each rig is wearing this frame**, by lighting its
/// proxies with the ink that carries the code. Derived every frame from
/// the sim, the pointer and the carry, and never stored — exactly as the
/// bars this replaces were.
#[allow(clippy::too_many_arguments)]
fn paint(
    mut commands: Commands,
    shell: Res<Shell>,
    pointer: Res<crate::surface::VirtualPointer>,
    rig: Res<crate::rig::CameraRig>,
    ghosts: Res<Ghosts>,
    inks: Res<MaskInks>,
    parts: Query<&Visibility, Without<MaskProxy>>,
    mut bodies: Query<(Entity, &Mesh3d, &mut MaskBody)>,
    mut proxies: Query<(&MaskProxy, &mut Visibility, &mut MeshMaterial3d<MaskInk>)>,
) {
    let sim = &shell.bridge.sim;
    // **What the aim is doing is one thing at a time**, so these REPLACE
    // rather than pile up: a body cannot be both flown through and held,
    // and a number that was two of them at once would name an ink that
    // is not there.
    let mut aims: Vec<(u32, u8)> = ghosts.0.iter().map(|id| (*id, GHOST)).collect();
    let mut aiming = |id: u32, state: u8| {
        if let Some(seen) = aims.iter_mut().find(|(other, _)| *other == id) {
            seen.1 = state;
        } else {
            aims.push((id, state));
        }
    };
    let held = sim.held(0);
    if let Some(held) = held {
        aiming(held.piece, if held.legal { CARRY_OK } else { CARRY_NO });
    } else if rig.roaming()
        && let Some(piece) =
            space_trucking::sim::layout::piece_at(sim.rooms(), sim.pieces(), pointer.sim)
    {
        let on_handle = crate::pieces::carry_handle_rect(
            piece.kind,
            space_trucking::sim::layout::piece_rect(sim.rooms(), sim.pieces(), piece),
        )
        .is_some_and(|handle| handle.contains(pointer.sim));
        aiming(piece.id, if on_handle { HANDLE } else { HOVER });
    }
    // And what the room is saying about it, which may be both at once
    // and may be said about a body the aim is on as well.
    let mut codes = aims;
    for (id, marked) in crate::room::lit_footprints(sim) {
        let claim = if marked { MARK } else { OFFER };
        if let Some(seen) = codes.iter_mut().find(|(other, _)| *other == id) {
            seen.1 |= claim;
        } else {
            codes.push((id, claim));
        }
    }
    // Nothing is filtered by where a piece is berthed, and that is
    // deliberate: a proxy exists only where a rig does, `sync_pieces`
    // retires a rig the sim no longer knows, and a good SHELVED in a
    // cabinet is drawn as a mini behind its own doors — so it is a thing
    // the crosshair can rest on and a thing that has to answer when it
    // does. A code naming a piece that is not drawn simply finds no
    // proxy wearing that number.
    // **Cut the copies the moment there is anything to say**, and not
    // before: a cabin nobody is pointing at carries no mask at all, and
    // the difference is some hundreds of bodies that would otherwise
    // stand hidden in every frame of the game forever.
    for (part, mesh, mut body) in &mut bodies {
        if body.cut || !codes.iter().any(|(id, _)| *id == body.piece) {
            continue;
        }
        body.cut = true;
        commands.spawn((
            Mesh3d(mesh.0.clone()),
            MeshMaterial3d(inks.ink(HOVER)),
            Transform::default(),
            Visibility::Hidden,
            MaskProxy {
                piece: body.piece,
                part,
            },
            // A CHILD of the part, so it rides every transform the part
            // rides and needs no copy of any of them.
            ChildOf(part),
        ));
    }
    for (proxy, mut visibility, mut ink) in &mut proxies {
        let shown = codes
            .iter()
            .find(|(id, _)| *id == proxy.piece)
            .filter(|_| parts.get(proxy.part) != Ok(&Visibility::Hidden));
        let Some(&(_, code)) = shown else {
            visibility.set_if_neq(Visibility::Hidden);
            continue;
        };
        // Unconditionally visible, and that is the ghost's whole
        // mechanism: the x-ray hides the body this is a copy of, and a
        // copy that inherited the hiding would leave nothing to outline.
        visibility.set_if_neq(Visibility::Visible);
        let want = inks.ink(code);
        if ink.0.id() != want.id() {
            ink.0 = want;
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// **No two readings draw one line over another.**
    ///
    /// Three of them may be worn at once — the crosshair rests on a good
    /// the room has offered and you have asked for — so the forms have
    /// to be able to share a body. Each has a band of its own, they run
    /// in the order they are claimed in, and the aim's is on the other
    /// side of the body's own edge from the other two.
    ///
    /// It is the old coplanar question asked where an outline can answer
    /// it. Two bands overlapping is one of them silently winning, which
    /// is a reading nobody can see and a defect no screenshot shows.
    #[test]
    fn no_two_readings_draw_one_line_over_another() {
        let bands = [
            ("aim", AIM_BAND),
            ("mark", MARK_BAND),
            ("offer", OFFER_BAND),
        ];
        for (name, band) in bands {
            assert!(band.0 < band.1, "the {name} band runs backwards: {band:?}");
        }
        for (i, (name, band)) in bands.iter().enumerate() {
            for (other, next) in bands.iter().skip(i + 1) {
                assert!(
                    band.1 <= next.0,
                    "the {name} band {band:?} runs into the {other}'s {next:?}"
                );
            }
        }
    }

    /// **A reading is never drawn on the body of another.**
    ///
    /// The defect this whole layer was rebuilt for, stated as a law. A
    /// mark used to be four short bars set 62% of the way in from its
    /// footprint's rim, which is *inside the footprint* — and a painting
    /// hangs flat on a wall and fills its footprint, so the mark on a
    /// picture for sale was drawn behind the picture. Press a good, and
    /// nothing on screen changed.
    ///
    /// One reading does sit on the body and it is the one that may: the
    /// aim is painted on the body's own rim from the inside, where it
    /// covers a texel of the thing it is about and nothing else. Every
    /// other band stands clear of the silhouette entirely.
    #[test]
    fn only_the_aim_is_drawn_on_the_body() {
        assert!(
            AIM_BAND.1 <= 0.0,
            "the aim is the inside reading: {AIM_BAND:?}"
        );
        assert!(
            AIM_BAND.0 >= -2.0,
            "an aim this deep would swallow a thin body whole: {AIM_BAND:?}"
        );
        for (name, band) in [("mark", MARK_BAND), ("offer", OFFER_BAND)] {
            assert!(
                band.0 >= 0.0,
                "the {name} is drawn inside the body it is about: {band:?}"
            );
        }
    }

    /// **An outline has no holes in it.**
    ///
    /// What the pass has is the distance to the nearest masked TEXEL,
    /// and on a grid that can only ever be one of √0, 1, √2, 2, √5, √8,
    /// 3 — so a band drawn between two of those values lights only where
    /// the geometry happens to land a texel inside it, and the ring
    /// comes out in pieces that move as the camera does. It was drawn
    /// that way once, at a texel wide with a texel of air inside it, and
    /// what a berthed body wore was a dotted arc down each flank.
    ///
    /// So the outer bands ABUT: between the body's own edge and the far
    /// side of the outermost form there is no distance a texel can be at
    /// and be drawn by nothing. And nothing reaches past the taps, which
    /// is the same hole from the other end — a band whose far edge is
    /// outside the disc lights only where the disc happens to reach.
    #[test]
    fn an_outline_has_no_holes_in_it() {
        let reach = REACH as f32;
        assert!(
            MARK_BAND.0 <= 1.0,
            "the innermost outside band starts past the nearest texel there is: {MARK_BAND:?}"
        );
        assert!(
            (MARK_BAND.1 - OFFER_BAND.0).abs() < 1e-6,
            "a texel between {:?} and {:?} is drawn by nothing",
            MARK_BAND.1,
            OFFER_BAND.0
        );
        assert!(
            OFFER_BAND.1 > reach,
            "the outermost band stops at {} inside the {reach} texels looked at",
            OFFER_BAND.1
        );
        assert!(
            OFFER_BAND.1 <= reach + 0.5,
            "the outermost band claims {} of a disc that reaches {reach}",
            OFFER_BAND.1
        );
        assert!(
            -AIM_BAND.1 <= 1.0 && -AIM_BAND.0 > 1.0,
            "the aim's own band misses the one texel inside a rim: {AIM_BAND:?}"
        );
    }

    /// **Every code a piece can wear has an ink.** The aim's five states
    /// and the two claims combine freely, and the pool is indexed by the
    /// sum: a code past the end would be a reading that silently drew
    /// the wrong one.
    #[test]
    fn every_code_has_an_ink_of_its_own() {
        let mut seen = Vec::new();
        for aim in [0, HOVER, HANDLE, CARRY_OK, CARRY_NO, GHOST] {
            for offer in [0, OFFER] {
                for mark in [0, MARK] {
                    let code = aim | offer | mark;
                    if code == 0 {
                        continue;
                    }
                    assert!(
                        usize::from(code) <= CODES,
                        "code {code} has no ink among {CODES}"
                    );
                    // And it travels in the alpha channel without ever
                    // reaching the 1.0 every opaque surface writes.
                    let alpha = f32::from(code) / SCALE;
                    assert!(
                        alpha < 0.7,
                        "code {code} rides at {alpha}, which reads as scene"
                    );
                    assert!(
                        !seen.contains(&code),
                        "code {code} is two readings wearing one number"
                    );
                    seen.push(code);
                }
            }
        }
    }
}
