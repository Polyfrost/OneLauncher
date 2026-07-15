use std::cell::RefCell;

use bytes::Bytes;
use freya::engine::prelude::{
    Data, Paint, SamplingOptions, Shader, SkData, SkImage, SkRect, TileMode,
};
use freya::prelude::*;
use skia_safe::RuntimeEffect;
use skia_safe::runtime_effect::ChildPtr;

use crate::hooks::use_player_skin;

const CAM_DIST: f32 = 80.0;
const CAM_FOCAL: f32 = 1.85;
const DRAG_SENSITIVITY: f32 = 0.01;
const PITCH_LIMIT: f32 = 1.3;

thread_local! {
    static EFFECT: RefCell<Option<RuntimeEffect>> = const { RefCell::new(None) };
}

fn player_effect() -> Option<RuntimeEffect> {
    EFFECT.with(|cell| {
        let mut slot = cell.borrow_mut();

        if slot.is_none() {
            match RuntimeEffect::make_for_shader(PLAYER_SKSL, None) {
                Ok(effect) => *slot = Some(effect),
                Err(err) => tracing::error!("player model SkSL failed to compile: {err}"),
            }
        }

        slot.clone()
    })
}

#[derive(PartialEq, Clone)]
pub struct PlayerModel {
    uuid: String,
    width: Size,
    height: Size,
    yaw: f32,
    pitch: f32,
}

impl PlayerModel {
    pub fn new(uuid: impl Into<String>) -> Self {
        Self {
            uuid: uuid.into(),
            width: Size::fill(),
            height: Size::fill(),
            yaw: 0.5,
            pitch: -0.1,
        }
    }

    pub fn width(mut self, width: Size) -> Self {
        self.width = width;
        self
    }

    pub fn height(mut self, height: Size) -> Self {
        self.height = height;
        self
    }

    #[allow(dead_code)]
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

impl Component for PlayerModel {
    fn render(&self) -> impl IntoElement {
        let (skin_bytes, is_slim) = use_player_skin(self.uuid.clone());

        let mut cache = use_state(|| None::<(usize, Shader)>);
        let src_ptr = skin_bytes.as_ptr() as usize;
        let cache_copy = cache.peek().cloned();
        let skin_shader = match cache_copy {
            Some((ptr, shader)) if ptr == src_ptr => Some(shader),
            _ => {
                let shader = decode_skin_shader(&skin_bytes);
                if let Some(shader) = &shader {
                    cache.set(Some((src_ptr, shader.clone())));
                }
                shader
            }
        };

        let mut yaw = use_state({
            let v = self.yaw;
            move || v
        });

        let mut pitch = use_state({
            let v = self.pitch;
            move || v
        });

        let mut drag = use_state(|| None::<(f32, f32, f32, f32)>);

        let mut last_uuid = use_state({
            let u = self.uuid.clone();
            move || u
        });

        if *last_uuid.peek() != self.uuid {
            last_uuid.set(self.uuid.clone());
            yaw.set(self.yaw);
            pitch.set(self.pitch);
            drag.set(None);
        }

        let effect = player_effect();

        let render_cb = RenderCallback::new(move |ctx: &mut CanvasContext| {
            let (Some(effect), Some(skin)) = (&effect, &skin_shader) else {
                return;
            };

            let (w, h) = (ctx.size.width, ctx.size.height);

            if w <= 0.0 || h <= 0.0 {
                return;
            }

            let (cur_yaw, cur_pitch) = (yaw(), pitch());

            let slim = if is_slim { 1.0 } else { 0.0 };

            let mut uniforms = Vec::with_capacity(28);
            for value in [w, h, cur_yaw, cur_pitch, CAM_DIST, CAM_FOCAL, slim] {
                uniforms.extend_from_slice(&value.to_le_bytes());
            }

            let Some(shader) = effect.make_shader(
                Data::new_copy(&uniforms),
                &[ChildPtr::from(skin.clone())],
                None,
            ) else {
                return;
            };

            let mut paint = Paint::default();
            paint.set_anti_alias(true);
            paint.set_shader(shader);
            ctx.canvas.draw_rect(SkRect::new(0.0, 0.0, w, h), &paint);
        });

        canvas(render_cb)
            .key(src_ptr as u64)
            .width(self.width.clone())
            .height(self.height.clone())
            .on_pointer_enter(|_| Cursor::set(CursorIcon::Grab))
            .on_pointer_leave(move |_| {
                if drag.peek().is_none() {
                    Cursor::set(CursorIcon::default());
                }
            })
            .on_pointer_down(move |e: Event<PointerEventData>| {
                let loc = e.global_location();
                drag.set(Some((loc.x as f32, loc.y as f32, yaw(), pitch())));
                Cursor::set(CursorIcon::Grabbing);
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
            .on_global_pointer_press(move |_: Event<PointerEventData>| {
                if drag.peek().is_some() {
                    drag.set(None);
                    Cursor::set(CursorIcon::default());
                }
            })
    }
}

fn decode_skin_shader(skin_bytes: &Bytes) -> Option<Shader> {
    let data = unsafe { SkData::new_bytes(skin_bytes) };
    let skin = SkImage::from_encoded(data)?;
    let skin = skin.make_raster_image(None, None).unwrap_or(skin);

    skin.to_shader(
        (TileMode::Clamp, TileMode::Clamp),
        SamplingOptions::default(),
        None,
    )
}

const PLAYER_SKSL: &str = r#"
uniform float uResX;
uniform float uResY;
uniform float uYaw;
uniform float uPitch;
uniform float uDist;
uniform float uFocal;
uniform float uSlim;
uniform shader uSkin;

void hitBox(float3 ro, float3 rd, float3 cmin, float3 cmax, float3 dims, float3 td,
            float2 base, inout float best, inout half4 col) {
    float3 t1 = (cmin - ro) / rd;
    float3 t2 = (cmax - ro) / rd;
    float3 tmin = min(t1, t2);
    float3 tmax = max(t1, t2);
    float tN = max(max(tmin.x, tmin.y), tmin.z);
    float tF = min(min(tmax.x, tmax.y), tmax.z);
    if (tN > tF || tF < 0.0) { return; }
    float t = tN;
    if (t < 0.0 || t >= best) { return; }

    float3 p = ro + rd * t;
    float3 n = clamp((p - cmin) / dims, 0.0, 1.0); // 0..1 inside the box
    float w = td.x, h = td.y, d = td.z;            // atlas region sizes

    float2 uv;
    float3 nrm;
    if (tN == tmin.x) {
        if (rd.x > 0.0) {            // -X face (player right)
            uv = float2(base.x + n.z * d, base.y + d + (1.0 - n.y) * h);
            nrm = float3(-1.0, 0.0, 0.0);
        } else {                     // +X face (player left)
            uv = float2(base.x + d + w + (1.0 - n.z) * d, base.y + d + (1.0 - n.y) * h);
            nrm = float3(1.0, 0.0, 0.0);
        }
    } else if (tN == tmin.y) {
        if (rd.y > 0.0) {            // -Y face (bottom): 180deg from top, flip both axes
            uv = float2(base.x + d + w + (1.0 - n.x) * w, base.y + (1.0 - n.z) * d);
            nrm = float3(0.0, -1.0, 0.0);
        } else {                     // +Y face (top)
            uv = float2(base.x + d + n.x * w, base.y + n.z * d);
            nrm = float3(0.0, 1.0, 0.0);
        }
    } else {
        if (rd.z > 0.0) {            // -Z face (back)
            uv = float2(base.x + 2.0 * d + w + (1.0 - n.x) * w, base.y + d + (1.0 - n.y) * h);
            nrm = float3(0.0, 0.0, -1.0);
        } else {                     // +Z face (front)
            uv = float2(base.x + d + n.x * w, base.y + d + (1.0 - n.y) * h);
            nrm = float3(0.0, 0.0, 1.0);
        }
    }

    half4 s = uSkin.eval(uv);

    if (s.a < 0.5) {
        return;
    }

    float3 L = normalize(float3(0.4, 0.8, 0.6));
    float diff = 0.65 + 0.35 * clamp(dot(nrm, L), 0.0, 1.0);
    col = half4(s.rgb * half(diff), 1.0);
    best = t;
}

half4 main(float2 fragCoord) {
    float2 res = float2(uResX, uResY);
    float2 uv = (fragCoord - 0.5 * res) / res.y;
    uv.y = -uv.y;                    // screen y is down; world y is up

    float cy = cos(uYaw), sy = sin(uYaw);
    float cp = cos(uPitch), sp = sin(uPitch);
    float3 ro = float3(sy * cp, sp, cy * cp) * uDist;
    float3 fwd = normalize(-ro);
    float3 right = normalize(cross(fwd, float3(0.0, 1.0, 0.0)));
    float3 up = cross(right, fwd);
    float3 rd = normalize(fwd * uFocal + right * uv.x + up * uv.y);

    float best = 1e9;
    half4 col = half4(0.0);

    float aw = uSlim > 0.5 ? 3.0 : 4.0;  // arm width: slim = 3px, classic = 4px
    float sw = aw + 0.5;                  // arm overlay (sleeve) width

    hitBox(ro, rd, float3(-4.0, 8.0, -4.0), float3(4.0, 16.0, 4.0),
           float3(8.0, 8.0, 8.0), float3(8.0, 8.0, 8.0), float2(0.0, 0.0), best, col);   // head
    hitBox(ro, rd, float3(-4.0, -4.0, -2.0), float3(4.0, 8.0, 2.0),
           float3(8.0, 12.0, 4.0), float3(8.0, 12.0, 4.0), float2(16.0, 16.0), best, col); // body
    hitBox(ro, rd, float3(-4.0 - aw, -4.0, -2.0), float3(-4.0, 8.0, 2.0),
           float3(aw, 12.0, 4.0), float3(aw, 12.0, 4.0), float2(40.0, 16.0), best, col); // right arm
    hitBox(ro, rd, float3(4.0, -4.0, -2.0), float3(4.0 + aw, 8.0, 2.0),
           float3(aw, 12.0, 4.0), float3(aw, 12.0, 4.0), float2(32.0, 48.0), best, col); // left arm
    hitBox(ro, rd, float3(-4.0, -16.0, -2.0), float3(0.0, -4.0, 2.0),
           float3(4.0, 12.0, 4.0), float3(4.0, 12.0, 4.0), float2(0.0, 16.0), best, col);  // right leg
    hitBox(ro, rd, float3(0.0, -16.0, -2.0), float3(4.0, -4.0, 2.0),
           float3(4.0, 12.0, 4.0), float3(4.0, 12.0, 4.0), float2(16.0, 48.0), best, col); // left leg

    hitBox(ro, rd, float3(-4.5, 7.5, -4.5), float3(4.5, 16.5, 4.5),
           float3(9.0, 9.0, 9.0), float3(8.0, 8.0, 8.0), float2(32.0, 0.0), best, col);    // hat
    hitBox(ro, rd, float3(-4.25, -4.25, -2.25), float3(4.25, 8.25, 2.25),
           float3(8.5, 12.5, 4.5), float3(8.0, 12.0, 4.0), float2(16.0, 32.0), best, col);  // jacket
    hitBox(ro, rd, float3(-3.75 - sw, -4.25, -2.25), float3(-3.75, 8.25, 2.25),
           float3(sw, 12.5, 4.5), float3(aw, 12.0, 4.0), float2(40.0, 32.0), best, col);  // right sleeve
    hitBox(ro, rd, float3(3.75, -4.25, -2.25), float3(3.75 + sw, 8.25, 2.25),
           float3(sw, 12.5, 4.5), float3(aw, 12.0, 4.0), float2(48.0, 48.0), best, col);  // left sleeve
    hitBox(ro, rd, float3(-4.25, -16.25, -2.25), float3(0.25, -3.75, 2.25),
           float3(4.5, 12.5, 4.5), float3(4.0, 12.0, 4.0), float2(0.0, 32.0), best, col);   // right pant
    hitBox(ro, rd, float3(-0.25, -16.25, -2.25), float3(4.25, -3.75, 2.25),
           float3(4.5, 12.5, 4.5), float3(4.0, 12.0, 4.0), float2(0.0, 48.0), best, col);   // left pant

    return col;
}
"#;
