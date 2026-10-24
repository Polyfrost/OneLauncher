use std::cell::RefCell;
use std::rc::Rc;
use std::time::{Duration, Instant};

use bytes::Bytes;
use freya::engine::prelude::{SkData, SkImage, SkRect};
use freya::prelude::*;
use oneclient_polyplus::api;
use plusrenderer::build::Buffers;
use plusrenderer::{Arms, BodySlot, Camera, Scene, bundle};
use skia_safe::vertices::VertexMode;
use skia_safe::{
    BlendMode, Canvas, FilterMode, ImageInfo, Matrix, MipmapMode, Paint, Point, SamplingOptions,
    Surface, TileMode, Vertices, surfaces,
};

use crate::hooks::use_player_skin;
use crate::hooks::wardrobe::CosmeticBundle;

const PLAYER: usize = 0;
const CAPE: usize = 1;
const FIRST_COSMETIC: usize = 2;
const CAM_DIST: f32 = 52.;
const MODEL_CENTER_Y: f32 = 16.;
const DRAG_SENSITIVITY: f32 = 0.01;
const PITCH_LIMIT: f32 = 1.3;
// Every animated frame repaints the whole window, so cosmetics animate slower than the display.
const ANIMATION_FRAME: Duration = Duration::from_millis(33);

#[derive(PartialEq, Clone)]
pub struct PlayerModel {
    uuid: String,
    width: Size,
    height: Size,
    yaw: f32,
    pitch: f32,
    skin: Option<(Bytes, bool)>,
    cape: Option<Bytes>,
    cosmetics: Vec<(api::BodySlot, CosmeticBundle)>,
    emote: Option<Bytes>,
    zoom: f32,
    focus_y: f32,
    draggable: bool,
}

impl PlayerModel {
    pub fn new(uuid: impl Into<String>) -> Self {
        Self {
            uuid: uuid.into(),
            width: Size::fill(),
            height: Size::fill(),
            yaw: 0.5,
            pitch: -0.1,
            skin: None,
            cape: None,
            cosmetics: Vec::new(),
            emote: None,
            zoom: 1.0,
            focus_y: 0.0,
            draggable: true,
        }
    }

    pub fn skin(mut self, skin: Option<(Bytes, bool)>) -> Self {
        self.skin = skin;
        self
    }

    pub fn zoom(mut self, zoom: f32) -> Self {
        self.zoom = zoom;
        self
    }

    pub fn draggable(mut self, draggable: bool) -> Self {
        self.draggable = draggable;
        self
    }

    pub fn focus_y(mut self, focus_y: f32) -> Self {
        self.focus_y = focus_y;
        self
    }

    pub fn cape(mut self, cape: Option<Bytes>) -> Self {
        self.cape = cape;
        self
    }

    pub fn cosmetics(mut self, cosmetics: Vec<(api::BodySlot, CosmeticBundle)>) -> Self {
        self.cosmetics = cosmetics;
        self
    }

    pub fn emote(mut self, emote: Option<Bytes>) -> Self {
        self.emote = emote;
        self
    }

    pub fn width(mut self, width: Size) -> Self {
        self.width = width;
        self
    }

    pub fn height(mut self, height: Size) -> Self {
        self.height = height;
        self
    }

    pub fn yaw(mut self, yaw: f32) -> Self {
        self.yaw = yaw;
        self
    }

    #[allow(dead_code)]
    pub fn pitch(mut self, pitch: f32) -> Self {
        self.pitch = pitch;
        self
    }
}

#[derive(PartialEq)]
struct Look {
    skin: Bytes,
    slim: bool,
    cape: Option<Bytes>,
    cosmetics: Vec<(api::BodySlot, CosmeticBundle)>,
    emote: Option<Bytes>,
}

#[derive(PartialEq, Clone, Copy)]
struct Frame {
    yaw: f32,
    pitch: f32,
    distance: f32,
    target_y: f32,
    seconds: f32,
    size: (f32, f32),
    scale: f32,
}

#[derive(Default)]
struct Player {
    look: Option<Look>,
    scene: Scene,
    textures: Vec<Option<SkImage>>,
    start: Option<Instant>,
    surface: Option<Surface>,
    frame: Option<(Frame, SkImage)>,
}

impl Player {
    // Freya repaints every visible model each frame, so draw from the last frame's image while nothing changed.
    fn paint(&mut self, canvas: &Canvas, frame: Frame) {
        let (w, h) = frame.size;
        let pixels = (
            (w * frame.scale).ceil() as i32,
            (h * frame.scale).ceil() as i32,
        );
        if self.frame.as_ref().map(|(last, _)| last) != Some(&frame) {
            self.frame = None;
            if self.surface.as_ref().map(|s| (s.width(), s.height())) != Some(pixels) {
                let info = ImageInfo::new_n32_premul(pixels, None);
                self.surface = canvas
                    .new_surface(&info, None)
                    .or_else(|| surfaces::raster(&info, None, None));
            }
            let Some(surface) = self.surface.as_mut() else {
                return;
            };

            // Fit the full height, or twice the width when the box is narrow.
            let fit = h.min(w * 2.);
            let camera = Camera::orbit(
                -frame.yaw.to_degrees(),
                frame.pitch.to_degrees(),
                frame.distance,
                [0., frame.target_y, 0.],
            );
            self.scene.pose(frame.seconds);
            let buffers = self.scene.build(&camera, [w, fit]);

            let offscreen = surface.canvas();
            offscreen.clear(skia_safe::Color::TRANSPARENT);
            offscreen.save();
            offscreen.scale((frame.scale, frame.scale));
            offscreen.translate((0., (h - fit) / 2.));
            draw(offscreen, buffers, &self.textures);
            offscreen.restore();
            self.frame = Some((frame, surface.image_snapshot()));
        }

        if let Some((_, image)) = &self.frame {
            canvas.draw_image_rect(image, None, SkRect::from_wh(w, h), &Paint::default());
        }
    }

    fn seconds(&self) -> f32 {
        self.start.map_or(0., |start| start.elapsed().as_secs_f32())
    }

    fn load(&mut self, look: Look) {
        let mut scene = Scene::default();
        let mut textures = vec![decode(&look.skin), None];

        scene.load_player(PLAYER, if look.slim { Arms::Slim } else { Arms::Wide });
        if let Some(json) = look.emote.as_deref().and_then(utf8) {
            match scene.load_animation(PLAYER, json) {
                Ok(()) => scene.play(PLAYER, 0.),
                Err(err) => tracing::warn!("emote failed to load: {err}"),
            }
        }

        if let Some(cape) = look.cape.as_ref().and_then(decode) {
            let [w, h] = [cape.width() as u32, cape.height() as u32];
            let (frames, frame_seconds) = bundle::cape_sheet([w, w / 2], [w, h], "cape.sheet");
            scene.load_cape(CAPE, frames, frame_seconds);
            textures[CAPE] = Some(cape);
        }

        for (index, (slot, cosmetic)) in look.cosmetics.iter().enumerate() {
            let index = FIRST_COSMETIC + index;
            let Some(body) = body_slot(*slot) else {
                continue;
            };
            let Some(geometry) = cosmetic.geometry.as_deref().and_then(utf8) else {
                continue;
            };
            let pet = cosmetic
                .pet
                .as_deref()
                .and_then(|json| serde_json::from_slice::<serde_json::Value>(json).ok());
            // Flying and walking pets roam beside the player in game; only shoulder pets attach.
            if pet.as_ref().is_some_and(|pet| {
                !pet["archetype"]
                    .as_str()
                    .is_some_and(|kind| kind.eq_ignore_ascii_case("shoulder"))
            }) {
                continue;
            }
            let model = match plusrenderer::bedrock::parse_one(geometry) {
                Ok(model) => model,
                Err(err) => {
                    tracing::warn!("cosmetic geometry failed to parse: {err}");
                    continue;
                }
            };

            let texture = cosmetic.texture.as_ref().and_then(decode);
            let size = texture
                .as_ref()
                .map(|image| [image.width() as u32, image.height() as u32]);
            let anchor = pet.as_ref().and_then(|pet| pet["anchor"].as_str());
            scene.load_cosmetic(index, model, body, anchor, size);
            if let Some(scale) = pet.as_ref().and_then(|pet| pet["scale"].as_f64()) {
                scene.set_scale(index, scale as f32);
            }
            if let Some(json) = cosmetic.animation.as_deref().and_then(utf8)
                && scene.load_animation(index, json).is_ok()
            {
                scene.play(index, 0.);
            }

            textures.resize(index + 1, None);
            textures[index] = texture;
        }

        self.scene = scene;
        self.textures = textures;
        self.start = Some(Instant::now());
        self.frame = None;
        self.look = Some(look);
    }
}

impl Component for PlayerModel {
    fn render(&self) -> impl IntoElement {
        let (profile_skin, profile_slim) = use_player_skin(self.uuid.clone());
        let (skin, slim) = self.skin.clone().unwrap_or((profile_skin, profile_slim));

        let player = use_hook(|| Rc::new(RefCell::new(Player::default())));
        let mut seconds = use_state(|| 0f32);

        let look = Look {
            skin,
            slim,
            cape: self.cape.clone(),
            cosmetics: self.cosmetics.clone(),
            emote: self.emote.clone(),
        };
        {
            let mut player = player.borrow_mut();
            if player.look.as_ref() != Some(&look) {
                player.load(look);
            }
        }

        use_hook({
            let player = player.clone();
            let animate = self.draggable;
            move || {
                if !animate {
                    return;
                }
                spawn(async move {
                    loop {
                        tokio::time::sleep(ANIMATION_FRAME).await;
                        let player = player.borrow();
                        if player.scene.is_animating() {
                            seconds.set(player.seconds());
                        }
                    }
                });
            }
        });

        let mut yaw = use_state({
            let v = self.yaw;
            move || v
        });

        let mut pitch = use_state({
            let v = self.pitch;
            move || v
        });

        let mut drag = use_state(|| None::<(f32, f32, f32, f32)>);

        let mut last_view = use_state({
            let view = (self.uuid.clone(), self.yaw.to_bits());
            move || view
        });

        let view = (self.uuid.clone(), self.yaw.to_bits());
        if *last_view.peek() != view {
            last_view.set(view);
            yaw.set(self.yaw);
            pitch.set(self.pitch);
            drag.set(None);
        }

        let draggable = self.draggable;
        let distance = CAM_DIST / self.zoom;
        let target_y = MODEL_CENTER_Y + self.focus_y;
        let (cur_yaw, cur_pitch, now) = (yaw(), pitch(), seconds());
        let draw_player = player.clone();

        let render_cb = RenderCallback::new(move |ctx| {
            let (w, h) = (ctx.size.width, ctx.size.height);
            if w <= 0.0 || h <= 0.0 {
                return;
            }

            draw_player.borrow_mut().paint(
                ctx.canvas,
                Frame {
                    yaw: cur_yaw,
                    pitch: cur_pitch,
                    distance,
                    target_y,
                    seconds: now,
                    size: (w, h),
                    scale: ctx.canvas.local_to_device_as_3x3().scale_x(),
                },
            );
        });

        rect()
            .width(self.width.clone())
            .height(self.height.clone())
            .cursor(if draggable {
                CursorIcon::Grab
            } else {
                CursorIcon::Pointer
            })
            .background(render_cb)
            .on_pointer_down(move |e: Event<PointerEventData>| {
                if !draggable {
                    return;
                }
                let loc = e.global_location();
                drag.set(Some((loc.x as f32, loc.y as f32, yaw(), pitch())));
            })
            .on_global_pointer_move(move |e: Event<PointerEventData>| {
                let Some((sx, sy, yaw0, pitch0)) = *drag.read() else {
                    return;
                };
                let loc = e.global_location();
                if loc.to_tuple() == (-1.0, -1.0) {
                    return;
                }

                let dx = loc.x as f32 - sx;
                let dy = loc.y as f32 - sy;

                yaw.set(yaw0 - dx * DRAG_SENSITIVITY);
                pitch.set((pitch0 + dy * DRAG_SENSITIVITY).clamp(-PITCH_LIMIT, PITCH_LIMIT));

                Platform::get().send(UserEvent::RequestRedraw);
            })
            // Global so a release outside the widget still ends the drag
            .on_global_pointer_up(move |_: Event<PointerEventData>| {
                if drag.peek().is_some() {
                    drag.set(None);
                }
            })
    }
}

fn utf8(bytes: &[u8]) -> Option<&str> {
    std::str::from_utf8(bytes).ok()
}

fn decode(bytes: &Bytes) -> Option<SkImage> {
    let image = SkImage::from_encoded(unsafe { SkData::new_bytes(bytes) })?;
    Some(image.make_raster_image(None, None).unwrap_or(image))
}

fn body_slot(slot: api::BodySlot) -> Option<BodySlot> {
    Some(match slot {
        api::BodySlot::Cape => BodySlot::Cape,
        api::BodySlot::Backpack => BodySlot::Backpack,
        api::BodySlot::Glasses => BodySlot::Glasses,
        api::BodySlot::Wings => BodySlot::Wings,
        api::BodySlot::LeftHand => BodySlot::LeftHand,
        api::BodySlot::RightHand => BodySlot::RightHand,
        api::BodySlot::Hat => BodySlot::Hat,
        api::BodySlot::Aura => BodySlot::Aura,
        api::BodySlot::Boots => BodySlot::Boots,
        api::BodySlot::Shoulder => BodySlot::Shoulder,
        api::BodySlot::Pet => BodySlot::Pet,
        api::BodySlot::Unknown => return None,
    })
}

// plusrenderer-skia's `draw`, on the Skia build Freya links (the crate pins a newer one).
fn draw(canvas: &Canvas, buffers: &Buffers, textures: &[Option<SkImage>]) {
    let points = |v: &[f32]| {
        v.as_chunks::<2>()
            .0
            .iter()
            .map(|&[x, y]| Point::new(x, y))
            .collect::<Vec<_>>()
    };
    let positions = points(&buffers.positions);
    let uvs = points(&buffers.uvs);
    let colors: Vec<skia_safe::Color> = buffers
        .colors
        .iter()
        .map(|&c| skia_safe::Color::new(c))
        .collect();
    let sampling = SamplingOptions::new(FilterMode::Nearest, MipmapMode::None);

    for run in &buffers.runs {
        let Some(Some(image)) = textures.get(run.texture as usize) else {
            continue;
        };

        let to_uv = Matrix::scale((1.0 / image.width() as f32, 1.0 / image.height() as f32));
        let Some(shader) = image.to_shader((TileMode::Clamp, TileMode::Clamp), sampling, &to_uv)
        else {
            continue;
        };

        let indices = &buffers.indices[run.start as usize..(run.start + run.count) as usize];
        let (Some(&first), Some(&last)) = (indices.iter().min(), indices.iter().max()) else {
            continue;
        };
        let range = first as usize..=last as usize;
        let local: Vec<u16> = indices.iter().map(|&i| i - first).collect();
        let vertices = Vertices::new_copy(
            VertexMode::Triangles,
            &positions[range.clone()],
            &uvs[range.clone()],
            &colors[range],
            Some(&local),
        );

        let mut paint = Paint::default();
        paint.set_shader(shader);
        canvas.draw_vertices(&vertices, BlendMode::Modulate, &paint);
    }
}
