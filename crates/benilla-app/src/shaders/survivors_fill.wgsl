// The survivors overlays' large soft gradients (card faces, the level-up and results vignettes),
// dithered. Bevy's UI gradients quantize straight to the 8-bit sRGB target, so a slow fade across
// dark values steps into visible bands; this adds half a byte of noise per pixel in the target's
// own sRGB encoding before it is written, which breaks the steps up below what the eye resolves.
// Colours mix in Oklab, as Bevy's gradients do by default, so the look matches what it replaced.
#import bevy_ui::ui_vertex_output::UiVertexOutput

struct Fill {
    // Linear RGBA. Vertical ramp: `a` at the top, `b` at `shape.z`, `c` at the bottom. Radial:
    // `a` at the centre, `c` at the farthest corner (`b` unused).
    a: vec4<f32>,
    b: vec4<f32>,
    c: vec4<f32>,
    // A soft elliptical light laid over the fill: linear RGB, and its peak alpha.
    glow: vec4<f32>,
    // The light's centre in UV, then its radii, both as fractions of the node's height.
    glow_at: vec4<f32>,
    // x: corner radius (px); y: 0 vertical ramp, 1 radial; z: the ramp's middle stop (0..1).
    shape: vec4<f32>,
};

@group(1) @binding(0) var<uniform> fill: Fill;

fn to_oklab(c: vec3<f32>) -> vec3<f32> {
    let l = 0.4122214708 * c.r + 0.5363325363 * c.g + 0.0514459929 * c.b;
    let m = 0.2119034982 * c.r + 0.6806995451 * c.g + 0.1073969566 * c.b;
    let s = 0.0883024619 * c.r + 0.2817188376 * c.g + 0.6299787005 * c.b;
    let lc = pow(max(vec3<f32>(l, m, s), vec3<f32>(0.0)), vec3<f32>(1.0 / 3.0));
    return vec3<f32>(
        0.2104542553 * lc.x + 0.7936177850 * lc.y - 0.0040720468 * lc.z,
        1.9779984951 * lc.x - 2.4285922050 * lc.y + 0.4505937099 * lc.z,
        0.0259040371 * lc.x + 0.7827717662 * lc.y - 0.8086757660 * lc.z,
    );
}

fn from_oklab(o: vec3<f32>) -> vec3<f32> {
    let l = o.x + 0.3963377774 * o.y + 0.2158037573 * o.z;
    let m = o.x - 0.1055613458 * o.y - 0.0638541728 * o.z;
    let s = o.x - 0.0894841775 * o.y - 1.2914855480 * o.z;
    let l3 = l * l * l;
    let m3 = m * m * m;
    let s3 = s * s * s;
    return vec3<f32>(
        4.0767416621 * l3 - 3.3077115913 * m3 + 0.2309699292 * s3,
        -1.2684380046 * l3 + 2.6097574011 * m3 - 0.3413193965 * s3,
        -0.0041960863 * l3 - 0.7034186147 * m3 + 1.7076127010 * s3,
    );
}

fn mix_ok(x: vec4<f32>, y: vec4<f32>, t: f32) -> vec4<f32> {
    let t1 = clamp(t, 0.0, 1.0);
    let lab = mix(to_oklab(x.rgb), to_oklab(y.rgb), t1);
    return vec4<f32>(max(from_oklab(lab), vec3<f32>(0.0)), mix(x.a, y.a, t1));
}

fn srgb_encode(c: vec3<f32>) -> vec3<f32> {
    let hi = 1.055 * pow(max(c, vec3<f32>(0.0)), vec3<f32>(1.0 / 2.4)) - 0.055;
    return select(hi, c * 12.92, c <= vec3<f32>(0.0031308));
}

fn srgb_decode(c: vec3<f32>) -> vec3<f32> {
    let hi = pow((max(c, vec3<f32>(0.0)) + 0.055) / 1.055, vec3<f32>(2.4));
    return select(hi, c / 12.92, c <= vec3<f32>(0.04045));
}

// Interleaved gradient noise (Jimenez 2014): cheap, even, no texture.
fn ign(p: vec2<f32>) -> f32 {
    return fract(52.9829189 * fract(dot(p, vec2<f32>(0.06711056, 0.00583715))));
}

// Signed distance to a rounded rectangle of half-size `h` and corner radius `r`.
fn rounded(p: vec2<f32>, h: vec2<f32>, r: f32) -> f32 {
    let q = abs(p) - h + vec2<f32>(r);
    return length(max(q, vec2<f32>(0.0))) + min(max(q.x, q.y), 0.0) - r;
}

@fragment
fn fragment(in: UiVertexOutput) -> @location(0) vec4<f32> {
    var col: vec4<f32>;
    if fill.shape.y < 0.5 {
        let t = in.uv.y;
        let m = clamp(fill.shape.z, 0.001, 0.999);
        if t < m {
            col = mix_ok(fill.a, fill.b, t / m);
        } else {
            col = mix_ok(fill.b, fill.c, (t - m) / (1.0 - m));
        }
    } else {
        let p = (in.uv - vec2<f32>(0.5)) * in.size;
        col = mix_ok(fill.a, fill.c, length(p) / max(length(in.size * 0.5), 1.0));
    }

    // The light, composited source-over onto the fill.
    if fill.glow.a > 0.0 {
        let radii = max(fill.glow_at.zw * in.size.y, vec2<f32>(1.0));
        let d = length((in.uv - fill.glow_at.xy) * in.size / radii);
        let g = fill.glow.a * clamp(1.0 - d, 0.0, 1.0);
        let a = g + col.a * (1.0 - g);
        col = vec4<f32>((fill.glow.rgb * g + col.rgb * col.a * (1.0 - g)) / max(a, 1e-4), a);
    }

    // ±half a byte in the target's own encoding, on colour and coverage alike.
    let n = ign(in.position.xy) - 0.5;
    let rgb = srgb_decode(clamp(srgb_encode(col.rgb) + vec3<f32>(n / 255.0), vec3<f32>(0.0), vec3<f32>(1.0)));
    var alpha = clamp(col.a + n / 255.0, 0.0, 1.0);

    if fill.shape.x > 0.0 {
        let sd = rounded((in.uv - vec2<f32>(0.5)) * in.size, in.size * 0.5, fill.shape.x * in.size.y);
        alpha *= clamp(0.5 - sd, 0.0, 1.0);
    }
    return vec4<f32>(rgb, alpha);
}
