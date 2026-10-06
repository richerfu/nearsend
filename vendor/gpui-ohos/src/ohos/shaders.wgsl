/* Functions useful for debugging:

// A heat map color for debugging (blue -> cyan -> green -> yellow -> red).
fn heat_map_color(value: f32, minValue: f32, maxValue: f32, position: vec2<f32>) -> vec4<f32> {
    // Normalize value to 0-1 range
    let t = clamp((value - minValue) / (maxValue - minValue), 0.0, 1.0);

    // Heat map color calculation
    let r = t * t;
    let g = 4.0 * t * (1.0 - t);
    let b = (1.0 - t) * (1.0 - t);
    let heat_color = vec3<f32>(r, g, b);

    // Create a checkerboard pattern (black and white)
    let sum = floor(position.x / 3) + floor(position.y / 3);
    let is_odd = fract(sum * 0.5); // 0.0 for even, 0.5 for odd
    let checker_value = is_odd * 2.0; // 0.0 for even, 1.0 for odd
    let checker_color = vec3<f32>(checker_value);

    // Determine if value is in range (1.0 if in range, 0.0 if out of range)
    let in_range = step(minValue, value) * step(value, maxValue);

    // Mix checkerboard and heat map based on whether value is in range
    let final_color = mix(checker_color, heat_color, in_range);

    return vec4<f32>(final_color, 1.0);
}

*/

// ============================================================================
// OHOS SHADER - Uses vertex attributes instead of SSBO (storage buffers)
// because GL_MAX_VERTEX_SHADER_STORAGE_BLOCKS = 0 on OHOS
// ============================================================================

// Contrast and gamma correction adapted from https://github.com/microsoft/terminal/blob/1283c0f5b99a2961673249fa77c6b986efb5086c/src/renderer/atlas/dwrite.hlsl
// Copyright (c) Microsoft Corporation.
// Licensed under the MIT license.
fn color_brightness(color: vec3<f32>) -> f32 {
    // REC. 601 luminance coefficients for perceived brightness
    return dot(color, vec3<f32>(0.30, 0.59, 0.11));
}

fn light_on_dark_contrast(enhancedContrast: f32, color: vec3<f32>) -> f32 {
    let brightness = color_brightness(color);
    let multiplier = saturate(4.0 * (0.75 - brightness));
    return enhancedContrast * multiplier;
}

fn enhance_contrast(alpha: f32, k: f32) -> f32 {
    return alpha * (k + 1.0) / (alpha * k + 1.0);
}

fn apply_alpha_correction(a: f32, b: f32, g: vec4<f32>) -> f32 {
    let brightness_adjustment = g.x * b + g.y;
    let correction = brightness_adjustment * a + (g.z * b + g.w);
    return a + a * (1.0 - a) * correction;
}

fn apply_contrast_and_gamma_correction(sample: f32, color: vec3<f32>, enhanced_contrast_factor: f32, gamma_ratios: vec4<f32>) -> f32 {
    let enhanced_contrast = light_on_dark_contrast(enhanced_contrast_factor, color);
    let brightness = color_brightness(color);

    let contrasted = enhance_contrast(sample, enhanced_contrast);
    return apply_alpha_correction(contrasted, brightness, gamma_ratios);
}

struct GlobalParams {
    viewport_size: vec2<f32>,
    premultiplied_alpha: u32,
    pad: u32,
}

struct GammaParams {
    gamma_ratios: vec4<f32>,
    grayscale_enhanced_contrast: f32,
    subpixel_enhanced_contrast: f32,
    pad: vec2<f32>,
}

@group(0) @binding(0) var<uniform> globals: GlobalParams;
@group(0) @binding(1) var<uniform> gamma: GammaParams;
@group(1) @binding(0) var t_sprite: texture_2d<f32>;
@group(1) @binding(1) var s_sprite: sampler;

const M_PI_F: f32 = 3.1415926;
const GRAYSCALE_FACTORS: vec3<f32> = vec3<f32>(0.2126, 0.7152, 0.0722);

struct Bounds {
    origin: vec2<f32>,
    size: vec2<f32>,
}

struct Corners {
    top_left: f32,
    top_right: f32,
    bottom_right: f32,
    bottom_left: f32,
}

struct Edges {
    top: f32,
    right: f32,
    bottom: f32,
    left: f32,
}

struct Hsla {
    h: f32,
    s: f32,
    l: f32,
    a: f32,
}

struct LinearColorStop {
    color: Hsla,
    percentage: f32,
}

struct Background {
    // 0u is Solid
    // 1u is LinearGradient
    // 2u is PatternSlash
    tag: u32,
    // 0u is sRGB linear color
    // 1u is Oklab color
    color_space: u32,
    solid: Hsla,
    gradient_angle_or_pattern_height: f32,
    colors: array<LinearColorStop, 2>,
    pad: u32,
}

struct AtlasTextureId {
    index: u32,
    kind: u32,
}

struct AtlasBounds {
    origin: vec2<i32>,
    size: vec2<i32>,
}

struct AtlasTile {
    texture_id: AtlasTextureId,
    tile_id: u32,
    padding: u32,
    bounds: AtlasBounds,
}

struct TransformationMatrix {
    rotation_scale: mat2x2<f32>,
    translation: vec2<f32>,
}

fn to_device_position_impl(position: vec2<f32>) -> vec4<f32> {
    let device_position = position / globals.viewport_size * vec2<f32>(2.0, -2.0) + vec2<f32>(-1.0, 1.0);
    return vec4<f32>(device_position, 0.0, 1.0);
}

fn to_device_position(unit_vertex: vec2<f32>, bounds: Bounds) -> vec4<f32> {
    let position = unit_vertex * vec2<f32>(bounds.size) + bounds.origin;
    return to_device_position_impl(position);
}

fn to_device_position_transformed(unit_vertex: vec2<f32>, bounds: Bounds, transform: TransformationMatrix) -> vec4<f32> {
    let position = unit_vertex * vec2<f32>(bounds.size) + bounds.origin;
    //Note: Rust side stores it as row-major, so transposing here
    let transformed = transpose(transform.rotation_scale) * position + transform.translation;
    return to_device_position_impl(transformed);
}

fn to_tile_position(unit_vertex: vec2<f32>, tile: AtlasTile) -> vec2<f32> {
  let atlas_size = vec2<f32>(textureDimensions(t_sprite, 0));
  return (vec2<f32>(tile.bounds.origin) + unit_vertex * vec2<f32>(tile.bounds.size)) / atlas_size;
}

fn distance_from_clip_rect_impl(position: vec2<f32>, clip_bounds: Bounds) -> vec4<f32> {
    let tl = position - clip_bounds.origin;
    let br = clip_bounds.origin + clip_bounds.size - position;
    return vec4<f32>(tl.x, br.x, tl.y, br.y);
}

fn distance_from_clip_rect(unit_vertex: vec2<f32>, bounds: Bounds, clip_bounds: Bounds) -> vec4<f32> {
    let position = unit_vertex * vec2<f32>(bounds.size) + bounds.origin;
    return distance_from_clip_rect_impl(position, clip_bounds);
}

fn distance_from_clip_rect_transformed(unit_vertex: vec2<f32>, bounds: Bounds, clip_bounds: Bounds, transform: TransformationMatrix) -> vec4<f32> {
    let position = unit_vertex * vec2<f32>(bounds.size) + bounds.origin;
    let transformed = transpose(transform.rotation_scale) * position + transform.translation;
    return distance_from_clip_rect_impl(transformed, clip_bounds);
}

// https://gamedev.stackexchange.com/questions/92015/optimized-linear-to-srgb-glsl
fn srgb_to_linear(srgb: vec3<f32>) -> vec3<f32> {
    let cutoff = srgb < vec3<f32>(0.04045);
    let higher = pow((srgb + vec3<f32>(0.055)) / vec3<f32>(1.055), vec3<f32>(2.4));
    let lower = srgb / vec3<f32>(12.92);
    return select(higher, lower, cutoff);
}

fn srgb_to_linear_component(a: f32) -> f32 {
    let cutoff = a < 0.04045;
    let higher = pow((a + 0.055) / 1.055, 2.4);
    let lower = a / 12.92;
    return select(higher, lower, cutoff);
}

fn linear_to_srgb(linear: vec3<f32>) -> vec3<f32> {
    let cutoff = linear < vec3<f32>(0.0031308);
    let higher = vec3<f32>(1.055) * pow(linear, vec3<f32>(1.0 / 2.4)) - vec3<f32>(0.055);
    let lower = linear * vec3<f32>(12.92);
    return select(higher, lower, cutoff);
}

/// Convert a linear color to sRGBA space.
fn linear_to_srgba(color: vec4<f32>) -> vec4<f32> {
    return vec4<f32>(linear_to_srgb(color.rgb), color.a);
}

/// Convert a sRGBA color to linear space.
fn srgba_to_linear(color: vec4<f32>) -> vec4<f32> {
    return vec4<f32>(srgb_to_linear(color.rgb), color.a);
}

/// Hsla to linear RGBA conversion.
fn hsla_to_rgba(hsla: Hsla) -> vec4<f32> {
    let h = hsla.h * 6.0; // Now, it's an angle but scaled in [0, 6) range
    let s = hsla.s;
    let l = hsla.l;
    let a = hsla.a;

    let c = (1.0 - abs(2.0 * l - 1.0)) * s;
    let x = c * (1.0 - abs(h % 2.0 - 1.0));
    let m = l - c / 2.0;
    var color = vec3<f32>(m);

    if (h >= 0.0 && h < 1.0) {
        color.r += c;
        color.g += x;
    } else if (h >= 1.0 && h < 2.0) {
        color.r += x;
        color.g += c;
    } else if (h >= 2.0 && h < 3.0) {
        color.g += c;
        color.b += x;
    } else if (h >= 3.0 && h < 4.0) {
        color.g += x;
        color.b += c;
    } else if (h >= 4.0 && h < 5.0) {
        color.r += x;
        color.b += c;
    } else {
        color.r += c;
        color.b += x;
    }

    return vec4<f32>(color, a);
}

/// Convert a linear sRGB to Oklab space.
/// Reference: https://bottosson.github.io/posts/oklab/#converting-from-linear-srgb-to-oklab
fn linear_srgb_to_oklab(color: vec4<f32>) -> vec4<f32> {
	let l = 0.4122214708 * color.r + 0.5363325363 * color.g + 0.0514459929 * color.b;
	let m = 0.2119034982 * color.r + 0.6806995451 * color.g + 0.1073969566 * color.b;
	let s = 0.0883024619 * color.r + 0.2817188376 * color.g + 0.6299787005 * color.b;

	let l_ = pow(l, 1.0 / 3.0);
	let m_ = pow(m, 1.0 / 3.0);
	let s_ = pow(s, 1.0 / 3.0);

	return vec4<f32>(
		0.2104542553 * l_ + 0.7936177850 * m_ - 0.0040720468 * s_,
		1.9779984951 * l_ - 2.4285922050 * m_ + 0.4505937099 * s_,
		0.0259040371 * l_ + 0.7827717662 * m_ - 0.8086757660 * s_,
		color.a
	);
}

/// Convert an Oklab color to linear sRGB space.
fn oklab_to_linear_srgb(color: vec4<f32>) -> vec4<f32> {
	let l_ = color.r + 0.3963377774 * color.g + 0.2158037573 * color.b;
	let m_ = color.r - 0.1055613458 * color.g - 0.0638541728 * color.b;
	let s_ = color.r - 0.0894841775 * color.g - 1.2914855480 * color.b;

	let l = l_ * l_ * l_;
	let m = m_ * m_ * m_;
	let s = s_ * s_ * s_;

	return vec4<f32>(
		4.0767416621 * l - 3.3077115913 * m + 0.2309699292 * s,
		-1.2684380046 * l + 2.6097574011 * m - 0.3413193965 * s,
		-0.0041960863 * l - 0.7034186147 * m + 1.7076147010 * s,
		color.a
	);
}

fn over(below: vec4<f32>, above: vec4<f32>) -> vec4<f32> {
    let alpha = above.a + below.a * (1.0 - above.a);
    let color = (above.rgb * above.a + below.rgb * below.a * (1.0 - above.a)) / alpha;
    return vec4<f32>(color, alpha);
}

// A standard gaussian function, used for weighting samples
fn gaussian(x: f32, sigma: f32) -> f32{
    return exp(-(x * x) / (2.0 * sigma * sigma)) / (sqrt(2.0 * M_PI_F) * sigma);
}

// This approximates the error function, needed for the gaussian integral
fn erf(v: vec2<f32>) -> vec2<f32> {
    let s = sign(v);
    let a = abs(v);
    let r1 = 1.0 + (0.278393 + (0.230389 + (0.000972 + 0.078108 * a) * a) * a) * a;
    let r2 = r1 * r1;
    return s - s / (r2 * r2);
}

fn blur_along_x(x: f32, y: f32, sigma: f32, corner: f32, half_size: vec2<f32>) -> f32 {
  let delta = min(half_size.y - corner - abs(y), 0.0);
  let curved = half_size.x - corner + sqrt(max(0.0, corner * corner - delta * delta));
  let integral = 0.5 + 0.5 * erf((x + vec2<f32>(-curved, curved)) * (sqrt(0.5) / sigma));
  return integral.y - integral.x;
}

// Selects corner radius based on quadrant.
fn pick_corner_radius(center_to_point: vec2<f32>, radii: Corners) -> f32 {
    if (center_to_point.x < 0.0) {
        if (center_to_point.y < 0.0) {
            return radii.top_left;
        } else {
            return radii.bottom_left;
        }
    } else {
        if (center_to_point.y < 0.0) {
            return radii.top_right;
        } else {
            return radii.bottom_right;
        }
    }
}

// Signed distance of the point to the quad's border - positive outside the
// border, and negative inside.
//
// See comments on similar code using `quad_sdf_impl` in `fs_quad` for
// explanation.
fn quad_sdf(point: vec2<f32>, bounds: Bounds, corner_radii: Corners) -> f32 {
    let half_size = bounds.size / 2.0;
    let center = bounds.origin + half_size;
    let center_to_point = point - center;
    let corner_radius = pick_corner_radius(center_to_point, corner_radii);
    let corner_to_point = abs(center_to_point) - half_size;
    let corner_center_to_point = corner_to_point + corner_radius;
    return quad_sdf_impl(corner_center_to_point, corner_radius);
}

fn quad_sdf_impl(corner_center_to_point: vec2<f32>, corner_radius: f32) -> f32 {
    if (corner_radius == 0.0) {
        // Fast path for unrounded corners.
        return max(corner_center_to_point.x, corner_center_to_point.y);
    } else {
        // Signed distance of the point from a quad that is inset by corner_radius.
        // It is negative inside this quad, and positive outside.
        let signed_distance_to_inset_quad =
            // 0 inside the inset quad, and positive outside.
            length(max(vec2<f32>(0.0), corner_center_to_point)) +
            // 0 outside the inset quad, and negative inside.
            min(0.0, max(corner_center_to_point.x, corner_center_to_point.y));

        return signed_distance_to_inset_quad - corner_radius;
    }
}

// Abstract away the final color transformation based on the
// target alpha compositing mode.
fn blend_color(color: vec4<f32>, alpha_factor: f32) -> vec4<f32> {
    let alpha = color.a * alpha_factor;
    let multiplier = select(1.0, alpha, globals.premultiplied_alpha != 0u);
    return vec4<f32>(color.rgb * multiplier, alpha);
}


struct GradientColor {
    solid: vec4<f32>,
    color0: vec4<f32>,
    color1: vec4<f32>,
}

fn prepare_gradient_color(tag: u32, color_space: u32,
    solid: Hsla, colors: array<LinearColorStop, 2>) -> GradientColor {
    var result = GradientColor();

    if (tag == 0u || tag == 2u) {
        result.solid = hsla_to_rgba(solid);
    } else if (tag == 1u) {
        // The hsla_to_rgba is returns a linear sRGB color
        result.color0 = hsla_to_rgba(colors[0].color);
        result.color1 = hsla_to_rgba(colors[1].color);

        // Prepare color space in vertex for avoid conversion
        // in fragment shader for performance reasons
        if (color_space == 0u) {
            // sRGB
            result.color0 = linear_to_srgba(result.color0);
            result.color1 = linear_to_srgba(result.color1);
        } else if (color_space == 1u) {
            // Oklab
            result.color0 = linear_srgb_to_oklab(result.color0);
            result.color1 = linear_srgb_to_oklab(result.color1);
        }
    }

    return result;
}

fn gradient_color(background: Background, position: vec2<f32>, bounds: Bounds,
    solid_color: vec4<f32>, color0: vec4<f32>, color1: vec4<f32>) -> vec4<f32> {
    var background_color = vec4<f32>(0.0);

    switch (background.tag) {
        default: {
            return solid_color;
        }
        case 1u: {
            // Linear gradient background.
            // -90 degrees to match the CSS gradient angle.
            let angle = background.gradient_angle_or_pattern_height;
            let radians = (angle % 360.0 - 90.0) * M_PI_F / 180.0;
            var direction = vec2<f32>(cos(radians), sin(radians));
            let stop0_percentage = background.colors[0].percentage;
            let stop1_percentage = background.colors[1].percentage;

            // Expand the short side to be the same as the long side
            if (bounds.size.x > bounds.size.y) {
                direction.y *= bounds.size.y / bounds.size.x;
            } else {
                direction.x *= bounds.size.x / bounds.size.y;
            }

            // Get the t value for the linear gradient with the color stop percentages.
            let half_size = bounds.size / 2.0;
            let center = bounds.origin + half_size;
            let center_to_point = position - center;
            var t = dot(center_to_point, direction) / length(direction);
            // Check the direct to determine the use x or y
            if (abs(direction.x) > abs(direction.y)) {
                t = (t + half_size.x) / bounds.size.x;
            } else {
                t = (t + half_size.y) / bounds.size.y;
            }

            // Adjust t based on the stop percentages
            t = (t - stop0_percentage) / (stop1_percentage - stop0_percentage);
            t = clamp(t, 0.0, 1.0);

            switch (background.color_space) {
                default: {
                    background_color = srgba_to_linear(mix(color0, color1, t));
                }
                case 1u: {
                    let oklab_color = mix(color0, color1, t);
                    background_color = oklab_to_linear_srgb(oklab_color);
                }
            }
        }
        case 2u: {
            let gradient_angle_or_pattern_height = background.gradient_angle_or_pattern_height;
            let pattern_width = (gradient_angle_or_pattern_height / 65535.0f) / 255.0f;
            let pattern_interval = (gradient_angle_or_pattern_height % 65535.0f) / 255.0f;
            let pattern_height = pattern_width + pattern_interval;
            let stripe_angle = M_PI_F / 4.0;
            let pattern_period = pattern_height * sin(stripe_angle);
            let rotation = mat2x2<f32>(
                cos(stripe_angle), -sin(stripe_angle),
                sin(stripe_angle), cos(stripe_angle)
            );
            let relative_position = position - bounds.origin;
            let rotated_point = rotation * relative_position;
            let pattern = rotated_point.x % pattern_period;
            let distance = min(pattern, pattern_period - pattern) - pattern_period * (pattern_width / pattern_height) /  2.0f;
            background_color = solid_color;
            background_color.a *= saturate(0.5 - distance);
        }
    }

    return background_color;
}

// --- quads --- //
// Using vertex attributes instead of storage buffer

struct QuadVertexInput {
    // Quad fields packed into vertex attributes
    @location(0) order_border_style: vec2<u32>,      // order, border_style
    @location(1) bounds_origin: vec2<f32>,           // bounds.origin
    @location(2) bounds_size: vec2<f32>,             // bounds.size
    @location(3) content_mask_origin: vec2<f32>,     // content_mask.origin
    @location(4) content_mask_size: vec2<f32>,       // content_mask.size
    @location(5) background_tag_colorspace: vec2<u32>, // background.tag, background.color_space
    @location(6) background_solid: vec4<f32>,        // background.solid (h,s,l,a)
    @location(7) background_angle: f32,              // background.gradient_angle_or_pattern_height
    @location(8) background_color0: vec4<f32>,       // background.colors[0].color (h,s,l,a)
    @location(9) background_stop0: f32,              // background.colors[0].percentage
    @location(10) background_color1: vec4<f32>,      // background.colors[1].color (h,s,l,a)
    @location(11) background_stop1: f32,             // background.colors[1].percentage
    @location(12) border_color: vec4<f32>,           // border_color (h,s,l,a)
    @location(13) corner_radii: vec4<f32>,           // corner_radii (tl, tr, br, bl)
    @location(14) border_widths: vec4<f32>,          // border_widths (top, right, bottom, left)
}

struct QuadVarying {
    @builtin(position) position: vec4<f32>,
    @location(0) @interpolate(flat) border_color: vec4<f32>,
    // Pass all data needed by fragment shader as flat varyings
    @location(1) @interpolate(flat) bounds_origin: vec2<f32>,
    @location(2) clip_distances: vec4<f32>,
    @location(3) @interpolate(flat) background_solid: vec4<f32>,
    @location(4) @interpolate(flat) background_color0: vec4<f32>,
    @location(5) @interpolate(flat) background_color1: vec4<f32>,
    @location(6) @interpolate(flat) bounds_size: vec2<f32>,
    @location(7) @interpolate(flat) corner_radii: vec4<f32>,
    @location(8) @interpolate(flat) border_widths: vec4<f32>,
    @location(9) @interpolate(flat) background_tag_colorspace: vec2<u32>,
    @location(10) @interpolate(flat) background_angle: f32,
    @location(11) @interpolate(flat) background_stops: vec2<f32>,
    @location(12) @interpolate(flat) border_style: u32,
}

@vertex
fn vs_quad(@builtin(vertex_index) vertex_id: u32, quad: QuadVertexInput) -> QuadVarying {
    let unit_vertex = vec2<f32>(f32(vertex_id & 1u), 0.5 * f32(vertex_id & 2u));

    var bounds: Bounds;
    bounds.origin = quad.bounds_origin;
    bounds.size = quad.bounds_size;

    var content_mask: Bounds;
    content_mask.origin = quad.content_mask_origin;
    content_mask.size = quad.content_mask_size;



    var background_solid_hsla: Hsla;
    background_solid_hsla.h = quad.background_solid.x;
    background_solid_hsla.s = quad.background_solid.y;
    background_solid_hsla.l = quad.background_solid.z;
    background_solid_hsla.a = quad.background_solid.w;

    var color0_hsla: Hsla;
    color0_hsla.h = quad.background_color0.x;
    color0_hsla.s = quad.background_color0.y;
    color0_hsla.l = quad.background_color0.z;
    color0_hsla.a = quad.background_color0.w;

    var color1_hsla: Hsla;
    color1_hsla.h = quad.background_color1.x;
    color1_hsla.s = quad.background_color1.y;
    color1_hsla.l = quad.background_color1.z;
    color1_hsla.a = quad.background_color1.w;

    var colors: array<LinearColorStop, 2>;
    colors[0].color = color0_hsla;
    colors[0].percentage = quad.background_stop0;
    colors[1].color = color1_hsla;
    colors[1].percentage = quad.background_stop1;

    var out = QuadVarying();
    out.position = to_device_position(unit_vertex, bounds);

    let gradient = prepare_gradient_color(
        quad.background_tag_colorspace.x,
        quad.background_tag_colorspace.y,
        background_solid_hsla,
        colors
    );
    out.background_solid = gradient.solid;
    out.background_color0 = gradient.color0;
    out.background_color1 = gradient.color1;

    var border_color_hsla: Hsla;
    border_color_hsla.h = quad.border_color.x;
    border_color_hsla.s = quad.border_color.y;
    border_color_hsla.l = quad.border_color.z;
    border_color_hsla.a = quad.border_color.w;
    out.border_color = hsla_to_rgba(border_color_hsla);

    // Pass data to fragment shader
    out.bounds_origin = bounds.origin;
    out.bounds_size = bounds.size;
    out.corner_radii = quad.corner_radii;
    out.border_widths = quad.border_widths;
    out.background_tag_colorspace = quad.background_tag_colorspace;
    out.background_angle = quad.background_angle;
    out.background_stops = vec2<f32>(quad.background_stop0, quad.background_stop1);
    out.border_style = quad.order_border_style.y;
    out.clip_distances = distance_from_clip_rect(unit_vertex, bounds, content_mask);
    return out;
}

@fragment
fn fs_quad(input: QuadVarying) -> @location(0) vec4<f32> {
    // Alpha clip first, since we don't have `clip_distance`.
    if (any(input.clip_distances < vec4<f32>(0.0))) {
        return vec4<f32>(0.0);
    }

    // Reconstruct bounds and other data from varyings
    var bounds: Bounds;
    bounds.origin = input.bounds_origin;
    bounds.size = input.bounds_size;

    var corner_radii: Corners;
    corner_radii.top_left = input.corner_radii.x;
    corner_radii.top_right = input.corner_radii.y;
    corner_radii.bottom_right = input.corner_radii.z;
    corner_radii.bottom_left = input.corner_radii.w;

    var border_widths: Edges;
    border_widths.top = input.border_widths.x;
    border_widths.right = input.border_widths.y;
    border_widths.bottom = input.border_widths.z;
    border_widths.left = input.border_widths.w;

    var background: Background;
    background.tag = input.background_tag_colorspace.x;
    background.color_space = input.background_tag_colorspace.y;
    background.gradient_angle_or_pattern_height = input.background_angle;
    background.colors[0].percentage = input.background_stops.x;
    background.colors[1].percentage = input.background_stops.y;

    let background_color = gradient_color(background, input.position.xy, bounds,
        input.background_solid, input.background_color0, input.background_color1);

    let unrounded = corner_radii.top_left == 0.0 &&
        corner_radii.bottom_left == 0.0 &&
        corner_radii.top_right == 0.0 &&
        corner_radii.bottom_right == 0.0;

    // Fast path when the quad is not rounded and doesn't have any border
    if (border_widths.top == 0.0 &&
            border_widths.left == 0.0 &&
            border_widths.right == 0.0 &&
            border_widths.bottom == 0.0 &&
            unrounded) {
        return blend_color(background_color, 1.0);
    }

    let size = bounds.size;
    let half_size = size / 2.0;
    let point = input.position.xy - bounds.origin;
    let center_to_point = point - half_size;

    // Signed distance field threshold for inclusion of pixels. 0.5 is the
    // minimum distance between the center of the pixel and the edge.
    let antialias_threshold = 0.5;

    // Radius of the nearest corner
    let corner_radius = pick_corner_radius(center_to_point, corner_radii);

    // Width of the nearest borders
    let border = vec2<f32>(
        select(
            border_widths.right,
            border_widths.left,
            center_to_point.x < 0.0),
        select(
            border_widths.bottom,
            border_widths.top,
            center_to_point.y < 0.0));

    // 0-width borders are reduced so that `inner_sdf >= antialias_threshold`.
    // The purpose of this is to not draw antialiasing pixels in this case.
    let reduced_border =
        vec2<f32>(select(border.x, -antialias_threshold, border.x == 0.0),
                  select(border.y, -antialias_threshold, border.y == 0.0));

    // Vector from the corner of the quad bounds to the point, after mirroring
    // the point into the bottom right quadrant. Both components are <= 0.
    let corner_to_point = abs(center_to_point) - half_size;

    // Vector from the point to the center of the rounded corner's circle, also
    // mirrored into bottom right quadrant.
    let corner_center_to_point = corner_to_point + corner_radius;

    // Whether the nearest point on the border is rounded
    let is_near_rounded_corner =
            corner_center_to_point.x >= 0 &&
            corner_center_to_point.y >= 0;

    // Vector from straight border inner corner to point.
    let straight_border_inner_corner_to_point = corner_to_point + reduced_border;

    // Whether the point is beyond the inner edge of the straight border.
    let is_beyond_inner_straight_border =
            straight_border_inner_corner_to_point.x > 0 ||
            straight_border_inner_corner_to_point.y > 0;

    // Whether the point is far enough inside the quad, such that the pixels are
    // not affected by the straight border.
    let is_within_inner_straight_border =
        straight_border_inner_corner_to_point.x < -antialias_threshold &&
        straight_border_inner_corner_to_point.y < -antialias_threshold;

    // Fast path for points that must be part of the background.
    //
    // This could be optimized further for large rounded corners by including
    // points in an inscribed rectangle, or some other quick linear check.
    // However, that might negatively impact performance in the case of
    // reasonable sizes for rounded corners.
    if (is_within_inner_straight_border && !is_near_rounded_corner) {
        return blend_color(background_color, 1.0);
    }

    // Signed distance of the point to the outside edge of the quad's border. It
    // is positive outside this edge, and negative inside.
    let outer_sdf = quad_sdf_impl(corner_center_to_point, corner_radius);

    // Approximate signed distance of the point to the inside edge of the quad's
    // border. It is negative outside this edge (within the border), and
    // positive inside.
    //
    // This is not always an accurate signed distance:
    // * The rounded portions with varying border width use an approximation of
    //   nearest-point-on-ellipse.
    // * When it is quickly known to be outside the edge, -1.0 is used.
    var inner_sdf = 0.0;
    if (corner_center_to_point.x <= 0 || corner_center_to_point.y <= 0) {
        // Fast paths for straight borders.
        inner_sdf = -max(straight_border_inner_corner_to_point.x,
                         straight_border_inner_corner_to_point.y);
    } else if (is_beyond_inner_straight_border) {
        // Fast path for points that must be outside the inner edge.
        inner_sdf = -1.0;
    } else if (reduced_border.x == reduced_border.y) {
        // Fast path for circular inner edge.
        inner_sdf = -(outer_sdf + reduced_border.x);
    } else {
        let ellipse_radii = max(vec2<f32>(0.0), corner_radius - reduced_border);
        inner_sdf = quarter_ellipse_sdf(corner_center_to_point, ellipse_radii);
    }

    // Negative when inside the border
    let border_sdf = max(inner_sdf, outer_sdf);

    var color = background_color;
    if (border_sdf < antialias_threshold) {
        var border_color = input.border_color;

        // Dashed border logic when border_style == 1
        if (input.border_style == 1u) {
            // Position along the perimeter in "dash space", where each dash
            // period has length 1
            var t = 0.0;

            // Total number of dash periods, so that the dash spacing can be
            // adjusted to evenly divide it
            var max_t = 0.0;

            // Border width is proportional to dash size. This is the behavior
            // used by browsers, but also avoids dashes from different segments
            // overlapping when dash size is smaller than the border width.
            //
            // Dash pattern: (2 * border width) dash, (1 * border width) gap
            let dash_length_per_width = 2.0;
            let dash_gap_per_width = 1.0;
            let dash_period_per_width = dash_length_per_width + dash_gap_per_width;

            // Since the dash size is determined by border width, the density of
            // dashes varies. Multiplying a pixel distance by this returns a
            // position in dash space - it has units (dash period / pixels). So
            // a dash velocity of (1 / 10) is 1 dash every 10 pixels.
            var dash_velocity = 0.0;

            // Dividing this by the border width gives the dash velocity
            let dv_numerator = 1.0 / dash_period_per_width;

            if (unrounded) {
                // When corners aren't rounded, the dashes are separately laid
                // out on each straight line, rather than around the whole
                // perimeter. This way each line starts and ends with a dash.
                let is_horizontal =
                        corner_center_to_point.x <
                        corner_center_to_point.y;

                // When applying dashed borders to just some, not all, the sides.
                // The way we chose border widths above sometimes comes with a 0 width value.
                // So we choose again to avoid division by zero.
                // TODO: A better solution exists taking a look at the whole file.
                // this does not fix single dashed borders at the corners
                let dashed_border = vec2<f32>(
                        max(
                            border_widths.bottom,
                            border_widths.top,
                        ),
                        max(
                            border_widths.right,
                            border_widths.left,
                        )
                   );

                let border_width = select(dashed_border.y, dashed_border.x, is_horizontal);
                dash_velocity = dv_numerator / border_width;
                t = select(point.y, point.x, is_horizontal) * dash_velocity;
                max_t = select(size.y, size.x, is_horizontal) * dash_velocity;
            } else {
                // When corners are rounded, the dashes are laid out clockwise
                // around the whole perimeter.

                let r_tr = corner_radii.top_right;
                let r_br = corner_radii.bottom_right;
                let r_bl = corner_radii.bottom_left;
                let r_tl = corner_radii.top_left;

                let w_t = border_widths.top;
                let w_r = border_widths.right;
                let w_b = border_widths.bottom;
                let w_l = border_widths.left;

                // Straight side dash velocities
                let dv_t = select(dv_numerator / w_t, 0.0, w_t <= 0.0);
                let dv_r = select(dv_numerator / w_r, 0.0, w_r <= 0.0);
                let dv_b = select(dv_numerator / w_b, 0.0, w_b <= 0.0);
                let dv_l = select(dv_numerator / w_l, 0.0, w_l <= 0.0);

                // Straight side lengths in dash space
                let s_t = (size.x - r_tl - r_tr) * dv_t;
                let s_r = (size.y - r_tr - r_br) * dv_r;
                let s_b = (size.x - r_br - r_bl) * dv_b;
                let s_l = (size.y - r_bl - r_tl) * dv_l;

                let corner_dash_velocity_tr = corner_dash_velocity(dv_t, dv_r);
                let corner_dash_velocity_br = corner_dash_velocity(dv_b, dv_r);
                let corner_dash_velocity_bl = corner_dash_velocity(dv_b, dv_l);
                let corner_dash_velocity_tl = corner_dash_velocity(dv_t, dv_l);

                // Corner lengths in dash space
                let c_tr = r_tr * (M_PI_F / 2.0) * corner_dash_velocity_tr;
                let c_br = r_br * (M_PI_F / 2.0) * corner_dash_velocity_br;
                let c_bl = r_bl * (M_PI_F / 2.0) * corner_dash_velocity_bl;
                let c_tl = r_tl * (M_PI_F / 2.0) * corner_dash_velocity_tl;

                // Cumulative dash space upto each segment
                let upto_tr = s_t;
                let upto_r = upto_tr + c_tr;
                let upto_br = upto_r + s_r;
                let upto_b = upto_br + c_br;
                let upto_bl = upto_b + s_b;
                let upto_l = upto_bl + c_bl;
                let upto_tl = upto_l + s_l;
                max_t = upto_tl + c_tl;

                if (is_near_rounded_corner) {
                    let radians = atan2(corner_center_to_point.y,
                                        corner_center_to_point.x);
                    let corner_t = radians * corner_radius;

                    if (center_to_point.x >= 0.0) {
                        if (center_to_point.y < 0.0) {
                            dash_velocity = corner_dash_velocity_tr;
                            // Subtracted because radians is pi/2 to 0 when
                            // going clockwise around the top right corner,
                            // since the y axis has been flipped
                            t = upto_r - corner_t * dash_velocity;
                        } else {
                            dash_velocity = corner_dash_velocity_br;
                            // Added because radians is 0 to pi/2 when going
                            // clockwise around the bottom-right corner
                            t = upto_br + corner_t * dash_velocity;
                        }
                    } else {
                        if (center_to_point.y >= 0.0) {
                            dash_velocity = corner_dash_velocity_bl;
                            // Subtracted because radians is pi/2 to 0 when
                            // going clockwise around the bottom-left corner,
                            // since the x axis has been flipped
                            t = upto_l - corner_t * dash_velocity;
                        } else {
                            dash_velocity = corner_dash_velocity_tl;
                            // Added because radians is 0 to pi/2 when going
                            // clockwise around the top-left corner, since both
                            // axis were flipped
                            t = upto_tl + corner_t * dash_velocity;
                        }
                    }
                } else {
                    // Straight borders
                    let is_horizontal =
                            corner_center_to_point.x <
                            corner_center_to_point.y;
                    if (is_horizontal) {
                        if (center_to_point.y < 0.0) {
                            dash_velocity = dv_t;
                            t = (point.x - r_tl) * dash_velocity;
                        } else {
                            dash_velocity = dv_b;
                            t = upto_bl - (point.x - r_bl) * dash_velocity;
                        }
                    } else {
                        if (center_to_point.x < 0.0) {
                            dash_velocity = dv_l;
                            t = upto_tl - (point.y - r_tl) * dash_velocity;
                        } else {
                            dash_velocity = dv_r;
                            t = upto_r + (point.y - r_tr) * dash_velocity;
                        }
                    }
                }
            }

            let dash_length = dash_length_per_width / dash_period_per_width;
            let desired_dash_gap = dash_gap_per_width / dash_period_per_width;

            // Straight borders should start and end with a dash, so max_t is
            // reduced to cause this.
            max_t -= select(0.0, dash_length, unrounded);
            if (max_t >= 1.0) {
                // Adjust dash gap to evenly divide max_t.
                let dash_count = floor(max_t);
                let dash_period = max_t / dash_count;
                border_color.a *= dash_alpha(
                    t,
                    dash_period,
                    dash_length,
                    dash_velocity,
                    antialias_threshold);
            } else if (unrounded) {
                // When there isn't enough space for the full gap between the
                // two start / end dashes of a straight border, reduce gap to
                // make them fit.
                let dash_gap = max_t - dash_length;
                if (dash_gap > 0.0) {
                    let dash_period = dash_length + dash_gap;
                    border_color.a *= dash_alpha(
                        t,
                        dash_period,
                        dash_length,
                        dash_velocity,
                        antialias_threshold);
                }
            }
        }

        // Blend the border on top of the background and then linearly interpolate
        // between the two as we slide inside the background.
        let blended_border = over(background_color, border_color);
        color = mix(background_color, blended_border,
                    saturate(antialias_threshold - inner_sdf));
    }

    return blend_color(color, saturate(antialias_threshold - outer_sdf));
}

// Returns the dash velocity of a corner given the dash velocity of the two
// sides, by returning the slower velocity (larger dashes).
//
// Since 0 is used for dash velocity when the border width is 0 (instead of
// +inf), this returns the other dash velocity in that case.
//
// An alternative to this might be to appropriately interpolate the dash
// velocity around the corner, but that seems overcomplicated.
fn corner_dash_velocity(dv1: f32, dv2: f32) -> f32 {
    if (dv1 == 0.0) {
        return dv2;
    } else if (dv2 == 0.0) {
        return dv1;
    } else {
        return min(dv1, dv2);
    }
}

// Returns alpha used to render antialiased dashes.
// `t` is within the dash when `fmod(t, period) < length`.
fn dash_alpha(t: f32, period: f32, length: f32, dash_velocity: f32, antialias_threshold: f32) -> f32 {
    let half_period = period / 2;
    let half_length = length / 2;
    // Value in [-half_period, half_period].
    // The dash is in [-half_length, half_length].
    let centered = fmod(t + half_period - half_length, period) - half_period;
    // Signed distance for the dash, negative values are inside the dash.
    let signed_distance = abs(centered) - half_length;
    // Antialiased alpha based on the signed distance.
    return saturate(antialias_threshold - signed_distance / dash_velocity);
}

// This approximates distance to the nearest point to a quarter ellipse in a way
// that is sufficient for anti-aliasing when the ellipse is not very eccentric.
// The components of `point` are expected to be positive.
//
// Negative on the outside and positive on the inside.
fn quarter_ellipse_sdf(point: vec2<f32>, radii: vec2<f32>) -> f32 {
    // Scale the space to treat the ellipse like a unit circle.
    let circle_vec = point / radii;
    let unit_circle_sdf = length(circle_vec) - 1.0;
    // Approximate up-scaling of the length by using the average of the radii.
    //
    // TODO: A better solution would be to use the gradient of the implicit
    // function for an ellipse to approximate a scaling factor.
    return unit_circle_sdf * (radii.x + radii.y) * -0.5;
}

// Modulus that has the same sign as `a`.
fn fmod(a: f32, b: f32) -> f32 {
    return a - b * trunc(a / b);
}

// --- shadows --- //

struct ShadowVertexInput {
    @location(0) order: u32,                         // order (unused in shader)
    @location(1) blur_radius: f32,                   // blur_radius
    @location(2) bounds_origin: vec2<f32>,           // bounds.origin
    @location(3) bounds_size: vec2<f32>,             // bounds.size
    @location(4) corner_radii: vec4<f32>,            // corner_radii (tl, tr, br, bl)
    @location(5) content_mask_origin: vec2<f32>,     // content_mask.origin
    @location(6) content_mask_size: vec2<f32>,       // content_mask.size
    @location(7) color: vec4<f32>,                   // color (h,s,l,a)
}

struct ShadowVarying {
    @builtin(position) position: vec4<f32>,
    @location(0) @interpolate(flat) color: vec4<f32>,
    @location(1) @interpolate(flat) bounds_origin: vec2<f32>,
    @location(2) @interpolate(flat) bounds_size: vec2<f32>,
    @location(3) clip_distances: vec4<f32>,
    @location(4) @interpolate(flat) corner_radii: vec4<f32>,
    @location(5) @interpolate(flat) blur_radius: f32,
    @location(6) @interpolate(flat) shape_origin: vec2<f32>,
    @location(7) @interpolate(flat) shape_size: vec2<f32>,
}

@vertex
fn vs_shadow(@builtin(vertex_index) vertex_id: u32, shadow_input: ShadowVertexInput) -> ShadowVarying {
    let unit_vertex = vec2<f32>(f32(vertex_id & 1u), 0.5 * f32(vertex_id & 2u));

    let blur_radius = shadow_input.blur_radius;
    let margin = 3.0 * blur_radius;

    // Set the bounds of the shadow and adjust its size based on the shadow's
    // spread radius to achieve the spreading effect
    var bounds: Bounds;
    bounds.origin = shadow_input.bounds_origin - vec2<f32>(margin);
    bounds.size = shadow_input.bounds_size + 2.0 * vec2<f32>(margin);

    var content_mask: Bounds;
    content_mask.origin = shadow_input.content_mask_origin;
    content_mask.size = shadow_input.content_mask_size;

    var color_hsla: Hsla;
    color_hsla.h = shadow_input.color.x;
    color_hsla.s = shadow_input.color.y;
    color_hsla.l = shadow_input.color.z;
    color_hsla.a = shadow_input.color.w;

    var out = ShadowVarying();
    out.position = to_device_position(unit_vertex, bounds);
    out.color = hsla_to_rgba(color_hsla);
    out.bounds_origin = bounds.origin;
    out.bounds_size = bounds.size;
    out.corner_radii = shadow_input.corner_radii;
    out.blur_radius = blur_radius;
    out.clip_distances = distance_from_clip_rect(unit_vertex, bounds, content_mask);
    out.shape_origin = shadow_input.bounds_origin;
    out.shape_size = shadow_input.bounds_size;
    return out;
}

@fragment
fn fs_shadow(input: ShadowVarying) -> @location(0) vec4<f32> {
    // Alpha clip first, since we don't have `clip_distance`.
    if (any(input.clip_distances < vec4<f32>(0.0))) {
        return vec4<f32>(0.0);
    }

    var corner_radii: Corners;
    corner_radii.top_left = input.corner_radii.x;
    corner_radii.top_right = input.corner_radii.y;
    corner_radii.bottom_right = input.corner_radii.z;
    corner_radii.bottom_left = input.corner_radii.w;

    let origin = input.shape_origin;
    let size = input.shape_size;
    let half_size = size / 2.0;
    let center = origin + half_size;
    let point = input.position.xy - center;
    let max_corner = min(half_size.x, half_size.y);
    var clamped_corner_radii = corner_radii;
    clamped_corner_radii.top_left = min(clamped_corner_radii.top_left, max_corner);
    clamped_corner_radii.top_right = min(clamped_corner_radii.top_right, max_corner);
    clamped_corner_radii.bottom_right = min(clamped_corner_radii.bottom_right, max_corner);
    clamped_corner_radii.bottom_left = min(clamped_corner_radii.bottom_left, max_corner);
    let corner_radius = min(pick_corner_radius(point, clamped_corner_radii), max_corner);

    var alpha: f32;
    if (input.blur_radius == 0.0) {
        let bounds = Bounds(origin, size);
        let distance = quad_sdf(input.position.xy, bounds, clamped_corner_radii);
        alpha = saturate(0.5 - distance);
    } else {
        // The signal is only non-zero in a limited range, so don't waste samples
        let low = point.y - half_size.y;
        let high = point.y + half_size.y;
        let start = clamp(-3.0 * input.blur_radius, low, high);
        let end = clamp(3.0 * input.blur_radius, low, high);

        // Accumulate samples (match mac behavior)
        let step = (end - start) / 4.0;
        var y = start + step * 0.5;
        alpha = 0.0;
        for (var i = 0; i < 4; i += 1) {
            alpha += blur_along_x(point.x, point.y - y, input.blur_radius,
                corner_radius, half_size) * gaussian(y, input.blur_radius) * step;
            y += step;
        }
    }

    return blend_color(input.color, alpha);
}

// --- path rasterization --- //

struct PathRasterizationVertexInput {
    @location(0) xy_position: vec2<f32>,
    @location(1) st_position: vec2<f32>,
    @location(2) background_tag_colorspace: vec2<u32>,
    @location(3) background_solid: vec4<f32>,
    @location(4) background_angle: f32,
    @location(5) background_color0: vec4<f32>,
    @location(6) background_stop0: f32,
    @location(7) background_color1: vec4<f32>,
    @location(8) background_stop1: f32,
    @location(9) bounds_origin: vec2<f32>,
    @location(10) bounds_size: vec2<f32>,
}

struct PathRasterizationVarying {
    @builtin(position) position: vec4<f32>,
    @location(0) st_position: vec2<f32>,
    @location(1) @interpolate(flat) background_tag_colorspace: vec2<u32>,
    @location(2) @interpolate(flat) background_solid: vec4<f32>,
    @location(3) @interpolate(flat) background_angle: f32,
    @location(4) @interpolate(flat) background_color0: vec4<f32>,
    @location(5) @interpolate(flat) background_stop0: f32,
    @location(6) @interpolate(flat) background_color1: vec4<f32>,
    @location(7) @interpolate(flat) background_stop1: f32,
    @location(8) @interpolate(flat) bounds_origin: vec2<f32>,
    @location(9) @interpolate(flat) bounds_size: vec2<f32>,
    @location(10) clip_distances: vec4<f32>,
}

@vertex
fn vs_path_rasterization(v: PathRasterizationVertexInput) -> PathRasterizationVarying {
    var bounds: Bounds;
    bounds.origin = v.bounds_origin;
    bounds.size = v.bounds_size;

    var out = PathRasterizationVarying();
    out.position = to_device_position_impl(v.xy_position);
    out.st_position = v.st_position;
    out.background_tag_colorspace = v.background_tag_colorspace;
    out.background_solid = v.background_solid;
    out.background_angle = v.background_angle;
    out.background_color0 = v.background_color0;
    out.background_stop0 = v.background_stop0;
    out.background_color1 = v.background_color1;
    out.background_stop1 = v.background_stop1;
    out.bounds_origin = v.bounds_origin;
    out.bounds_size = v.bounds_size;
    out.clip_distances = distance_from_clip_rect_impl(v.xy_position, bounds);
    return out;
}

@fragment
fn fs_path_rasterization(input: PathRasterizationVarying) -> @location(0) vec4<f32> {
    let dx = dpdx(input.st_position);
    let dy = dpdy(input.st_position);
    if (any(input.clip_distances < vec4<f32>(0.0))) {
        return vec4<f32>(0.0);
    }

    var bounds: Bounds;
    bounds.origin = input.bounds_origin;
    bounds.size = input.bounds_size;

    var background_solid_hsla: Hsla;
    background_solid_hsla.h = input.background_solid.x;
    background_solid_hsla.s = input.background_solid.y;
    background_solid_hsla.l = input.background_solid.z;
    background_solid_hsla.a = input.background_solid.w;

    var color0_hsla: Hsla;
    color0_hsla.h = input.background_color0.x;
    color0_hsla.s = input.background_color0.y;
    color0_hsla.l = input.background_color0.z;
    color0_hsla.a = input.background_color0.w;

    var color1_hsla: Hsla;
    color1_hsla.h = input.background_color1.x;
    color1_hsla.s = input.background_color1.y;
    color1_hsla.l = input.background_color1.z;
    color1_hsla.a = input.background_color1.w;

    var colors: array<LinearColorStop, 2>;
    colors[0].color = color0_hsla;
    colors[0].percentage = input.background_stop0;
    colors[1].color = color1_hsla;
    colors[1].percentage = input.background_stop1;

    var background: Background;
    background.tag = input.background_tag_colorspace.x;
    background.color_space = input.background_tag_colorspace.y;
    background.solid = background_solid_hsla;
    background.gradient_angle_or_pattern_height = input.background_angle;
    background.colors = colors;

    var alpha: f32;
    if (length(vec2<f32>(dx.x, dy.x)) < 0.001) {
        // If the gradient is too small, return a solid color.
        alpha = 1.0;
    } else {
        let gradient = 2.0 * input.st_position.xx * vec2<f32>(dx.x, dy.x) - vec2<f32>(dx.y, dy.y);
        let f = input.st_position.x * input.st_position.x - input.st_position.y;
        let distance = f / length(gradient);
        alpha = saturate(0.5 - distance);
    }
    let gradient_color_result = prepare_gradient_color(
        background.tag,
        background.color_space,
        background.solid,
        background.colors,
    );
    let color = gradient_color(background, input.position.xy, bounds,
        gradient_color_result.solid, gradient_color_result.color0, gradient_color_result.color1);
    return vec4<f32>(color.rgb * color.a * alpha, color.a * alpha);
}

// --- paths --- //

struct PathSpriteVertexInput {
    @location(0) bounds_origin: vec2<f32>,
    @location(1) bounds_size: vec2<f32>,
}

struct PathVarying {
    @builtin(position) position: vec4<f32>,
    @location(0) texture_coords: vec2<f32>,
}

@vertex
fn vs_path(@builtin(vertex_index) vertex_id: u32, sprite: PathSpriteVertexInput) -> PathVarying {
    let unit_vertex = vec2<f32>(f32(vertex_id & 1u), 0.5 * f32(vertex_id & 2u));

    var bounds: Bounds;
    bounds.origin = sprite.bounds_origin;
    bounds.size = sprite.bounds_size;

    // Don't apply content mask because it was already accounted for when rasterizing the path.
    let device_position = to_device_position(unit_vertex, bounds);
    // For screen-space intermediate texture, convert screen position to texture coordinates
    let screen_position = bounds.origin + unit_vertex * bounds.size;
    let texture_coords = screen_position / globals.viewport_size;

    var out = PathVarying();
    out.position = device_position;
    out.texture_coords = texture_coords;

    return out;
}

@fragment
fn fs_path(input: PathVarying) -> @location(0) vec4<f32> {
    let sample = textureSample(t_sprite, s_sprite, input.texture_coords);
    return sample;
}

// --- underlines --- //

struct UnderlineVertexInput {
    @location(0) order: u32,
    @location(1) pad: u32,
    @location(2) bounds_origin: vec2<f32>,
    @location(3) bounds_size: vec2<f32>,
    @location(4) content_mask_origin: vec2<f32>,
    @location(5) content_mask_size: vec2<f32>,
    @location(6) color: vec4<f32>,
    @location(7) thickness: f32,
    @location(8) wavy: u32,
}

struct UnderlineVarying {
    @builtin(position) position: vec4<f32>,
    @location(0) @interpolate(flat) color: vec4<f32>,
    @location(1) @interpolate(flat) bounds_origin: vec2<f32>,
    @location(2) @interpolate(flat) bounds_size: vec2<f32>,
    @location(3) clip_distances: vec4<f32>,
    @location(4) @interpolate(flat) thickness: f32,
    @location(5) @interpolate(flat) wavy: u32,
}

@vertex
fn vs_underline(@builtin(vertex_index) vertex_id: u32, underline: UnderlineVertexInput) -> UnderlineVarying {
    let unit_vertex = vec2<f32>(f32(vertex_id & 1u), 0.5 * f32(vertex_id & 2u));

    var bounds: Bounds;
    bounds.origin = underline.bounds_origin;
    bounds.size = underline.bounds_size;

    var content_mask: Bounds;
    content_mask.origin = underline.content_mask_origin;
    content_mask.size = underline.content_mask_size;

    var color_hsla: Hsla;
    color_hsla.h = underline.color.x;
    color_hsla.s = underline.color.y;
    color_hsla.l = underline.color.z;
    color_hsla.a = underline.color.w;

    var out = UnderlineVarying();
    out.position = to_device_position(unit_vertex, bounds);
    out.color = hsla_to_rgba(color_hsla);
    out.bounds_origin = bounds.origin;
    out.bounds_size = bounds.size;
    out.thickness = underline.thickness;
    out.wavy = underline.wavy;
    out.clip_distances = distance_from_clip_rect(unit_vertex, bounds, content_mask);
    return out;
}

@fragment
fn fs_underline(input: UnderlineVarying) -> @location(0) vec4<f32> {
    const WAVE_FREQUENCY: f32 = 2.0;
    const WAVE_HEIGHT_RATIO: f32 = 0.8;

    // Alpha clip first, since we don't have `clip_distance`.
    if (any(input.clip_distances < vec4<f32>(0.0))) {
        return vec4<f32>(0.0);
    }

    if ((input.wavy & 0xFFu) == 0u)
    {
        return blend_color(input.color, input.color.a);
    }

    let half_thickness = input.thickness * 0.5;

    let st = (input.position.xy - input.bounds_origin) / input.bounds_size.y - vec2<f32>(0.0, 0.5);
    let frequency = M_PI_F * WAVE_FREQUENCY * input.thickness / input.bounds_size.y;
    let amplitude = (input.thickness * WAVE_HEIGHT_RATIO) / input.bounds_size.y;

    let sine = sin(st.x * frequency) * amplitude;
    let dSine = cos(st.x * frequency) * amplitude * frequency;
    let distance = (st.y - sine) / sqrt(1.0 + dSine * dSine);
    let distance_in_pixels = distance * input.bounds_size.y;
    let distance_from_top_border = distance_in_pixels - half_thickness;
    let distance_from_bottom_border = distance_in_pixels + half_thickness;
    let alpha = saturate(0.5 - max(-distance_from_bottom_border, distance_from_top_border));
    return blend_color(input.color, alpha * input.color.a);
}

// --- monochrome sprites --- //

struct MonoSpriteVertexInput {
    @location(0) order_pad: vec2<u32>,
    @location(1) bounds_origin: vec2<f32>,
    @location(2) bounds_size: vec2<f32>,
    @location(3) content_mask_origin: vec2<f32>,
    @location(4) content_mask_size: vec2<f32>,
    @location(5) color: vec4<f32>,
    @location(6) tile_texture_id: vec2<u32>,
    @location(7) tile_id_padding: vec2<u32>,
    @location(8) tile_bounds_origin: vec2<i32>,
    @location(9) tile_bounds_size: vec2<i32>,
    @location(10) transform_row0: vec2<f32>,
    @location(11) transform_row1: vec2<f32>,
    @location(12) transform_translation: vec2<f32>,
}

struct MonoSpriteVarying {
    @builtin(position) position: vec4<f32>,
    @location(0) tile_position: vec2<f32>,
    @location(1) @interpolate(flat) color: vec4<f32>,
    @location(3) clip_distances: vec4<f32>,
}

@vertex
fn vs_mono_sprite(@builtin(vertex_index) vertex_id: u32, sprite: MonoSpriteVertexInput) -> MonoSpriteVarying {
    let unit_vertex = vec2<f32>(f32(vertex_id & 1u), 0.5 * f32(vertex_id & 2u));

    var bounds: Bounds;
    bounds.origin = sprite.bounds_origin;
    bounds.size = sprite.bounds_size;

    var content_mask: Bounds;
    content_mask.origin = sprite.content_mask_origin;
    content_mask.size = sprite.content_mask_size;

    var transform: TransformationMatrix;
    transform.rotation_scale = mat2x2<f32>(sprite.transform_row0, sprite.transform_row1);
    transform.translation = sprite.transform_translation;

    var tile: AtlasTile;
    tile.texture_id.index = sprite.tile_texture_id.x;
    tile.texture_id.kind = sprite.tile_texture_id.y;
    tile.tile_id = sprite.tile_id_padding.x;
    tile.padding = sprite.tile_id_padding.y;
    tile.bounds.origin = sprite.tile_bounds_origin;
    tile.bounds.size = sprite.tile_bounds_size;

    var color_hsla: Hsla;
    color_hsla.h = sprite.color.x;
    color_hsla.s = sprite.color.y;
    color_hsla.l = sprite.color.z;
    color_hsla.a = sprite.color.w;

    var out = MonoSpriteVarying();
    out.position = to_device_position_transformed(unit_vertex, bounds, transform);
    out.tile_position = to_tile_position(unit_vertex, tile);
    out.color = hsla_to_rgba(color_hsla);
    out.clip_distances = distance_from_clip_rect_transformed(unit_vertex, bounds, content_mask, transform);
    return out;
}

@fragment
fn fs_mono_sprite(input: MonoSpriteVarying) -> @location(0) vec4<f32> {
    let sample = textureSample(t_sprite, s_sprite, input.tile_position).r;
    let alpha_corrected = apply_contrast_and_gamma_correction(
        sample,
        input.color.rgb,
        gamma.grayscale_enhanced_contrast,
        gamma.gamma_ratios,
    );

    // Alpha clip after using the derivatives.
    if (any(input.clip_distances < vec4<f32>(0.0))) {
        return vec4<f32>(0.0);
    }

    // convert to srgb space as the rest of the code (output swapchain) expects that
    return blend_color(input.color, alpha_corrected);
}

// --- polychrome sprites --- //

struct PolySpriteVertexInput {
    @location(0) order_pad: vec2<u32>,
    @location(1) grayscale: u32,
    @location(2) opacity: f32,
    @location(3) bounds_origin: vec2<f32>,
    @location(4) bounds_size: vec2<f32>,
    @location(5) content_mask_origin: vec2<f32>,
    @location(6) content_mask_size: vec2<f32>,
    @location(7) corner_radii: vec4<f32>,
    @location(8) tile_texture_id: vec2<u32>,
    @location(9) tile_id_padding: vec2<u32>,
    @location(10) tile_bounds_origin: vec2<i32>,
    @location(11) tile_bounds_size: vec2<i32>,
}

struct PolySpriteVarying {
    @builtin(position) position: vec4<f32>,
    @location(0) tile_position: vec2<f32>,
    @location(1) @interpolate(flat) bounds_origin: vec2<f32>,
    @location(2) @interpolate(flat) bounds_size: vec2<f32>,
    @location(3) clip_distances: vec4<f32>,
    @location(4) @interpolate(flat) corner_radii: vec4<f32>,
    @location(5) @interpolate(flat) grayscale: u32,
    @location(6) @interpolate(flat) opacity: f32,
}

@vertex
fn vs_poly_sprite(@builtin(vertex_index) vertex_id: u32, sprite: PolySpriteVertexInput) -> PolySpriteVarying {
    let unit_vertex = vec2<f32>(f32(vertex_id & 1u), 0.5 * f32(vertex_id & 2u));

    var bounds: Bounds;
    bounds.origin = sprite.bounds_origin;
    bounds.size = sprite.bounds_size;

    var content_mask: Bounds;
    content_mask.origin = sprite.content_mask_origin;
    content_mask.size = sprite.content_mask_size;

    var tile: AtlasTile;
    tile.texture_id.index = sprite.tile_texture_id.x;
    tile.texture_id.kind = sprite.tile_texture_id.y;
    tile.tile_id = sprite.tile_id_padding.x;
    tile.padding = sprite.tile_id_padding.y;
    tile.bounds.origin = sprite.tile_bounds_origin;
    tile.bounds.size = sprite.tile_bounds_size;

    var out = PolySpriteVarying();
    out.position = to_device_position(unit_vertex, bounds);
    out.tile_position = to_tile_position(unit_vertex, tile);
    out.bounds_origin = bounds.origin;
    out.bounds_size = bounds.size;
    out.corner_radii = sprite.corner_radii;
    out.grayscale = sprite.grayscale;
    out.opacity = sprite.opacity;
    out.clip_distances = distance_from_clip_rect(unit_vertex, bounds, content_mask);
    return out;
}

@fragment
fn fs_poly_sprite(input: PolySpriteVarying) -> @location(0) vec4<f32> {
    let sample = textureSample(t_sprite, s_sprite, input.tile_position);
    // Alpha clip after using the derivatives.
    if (any(input.clip_distances < vec4<f32>(0.0))) {
        return vec4<f32>(0.0);
    }

    var bounds: Bounds;
    bounds.origin = input.bounds_origin;
    bounds.size = input.bounds_size;

    var corner_radii: Corners;
    corner_radii.top_left = input.corner_radii.x;
    corner_radii.top_right = input.corner_radii.y;
    corner_radii.bottom_right = input.corner_radii.z;
    corner_radii.bottom_left = input.corner_radii.w;

    let distance = quad_sdf(input.position.xy, bounds, corner_radii);

    var color = sample;
    if ((input.grayscale & 0xFFu) != 0u) {
        let grayscale = dot(color.rgb, GRAYSCALE_FACTORS);
        color = vec4<f32>(vec3<f32>(grayscale), sample.a);
    }
    return blend_color(color, input.opacity * saturate(0.5 - distance));
}

// --- surfaces --- //

struct SurfaceParams {
    bounds: Bounds,
    content_mask: Bounds,
}

@group(2) @binding(0) var<uniform> surface_locals: SurfaceParams;
@group(2) @binding(1) var t_y: texture_2d<f32>;
@group(2) @binding(2) var t_cb_cr: texture_2d<f32>;
@group(2) @binding(3) var s_surface: sampler;

const ycbcr_to_RGB = mat4x4<f32>(
    vec4<f32>( 1.0000f,  1.0000f,  1.0000f, 0.0),
    vec4<f32>( 0.0000f, -0.3441f,  1.7720f, 0.0),
    vec4<f32>( 1.4020f, -0.7141f,  0.0000f, 0.0),
    vec4<f32>(-0.7010f,  0.5291f, -0.8860f, 1.0),
);

struct SurfaceVarying {
    @builtin(position) position: vec4<f32>,
    @location(0) texture_position: vec2<f32>,
    @location(3) clip_distances: vec4<f32>,
}

@vertex
fn vs_surface(@builtin(vertex_index) vertex_id: u32) -> SurfaceVarying {
    let unit_vertex = vec2<f32>(f32(vertex_id & 1u), 0.5 * f32(vertex_id & 2u));

    var out = SurfaceVarying();
    out.position = to_device_position(unit_vertex, surface_locals.bounds);
    out.texture_position = unit_vertex;
    out.clip_distances = distance_from_clip_rect(unit_vertex, surface_locals.bounds, surface_locals.content_mask);
    return out;
}

@fragment
fn fs_surface(input: SurfaceVarying) -> @location(0) vec4<f32> {
    // Alpha clip after using the derivatives.
    if (any(input.clip_distances < vec4<f32>(0.0))) {
        return vec4<f32>(0.0);
    }

    let y_cb_cr = vec4<f32>(
        textureSampleLevel(t_y, s_surface, input.texture_position, 0.0).r,
        textureSampleLevel(t_cb_cr, s_surface, input.texture_position, 0.0).rg,
        1.0);

    return ycbcr_to_RGB * y_cb_cr;
}
