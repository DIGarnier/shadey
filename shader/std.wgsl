struct StdUniform {
    window_size: vec2<u32>;
    mouse_pos: vec2<u32>;
    time: f32;
    toggle_mouse_pos: vec2<u32>;
};

[[group(0), binding(0)]]
var<uniform> _std_uniform: StdUniform;

struct VertexOutput {
    [[builtin(position)]] clip_position: vec4<f32>;
    [[location(0)]] texcoords: vec2<f32>;
};

[[stage(vertex)]]
fn vs_main(
    [[builtin(vertex_index)]] in_vertex_index: u32,
) -> VertexOutput {
    var vertices = array<vec2<f32>,3>(vec2<f32>(-1.,-1.), vec2<f32>(3.,-1.), vec2<f32>(-1., 3.));
    var out: VertexOutput;
    out.clip_position = vec4<f32>(vertices[in_vertex_index], 0.0, 1.0);
    out.texcoords = 0.5 * out.clip_position.xy + vec2<f32>(0.5);
    return out;
}

fn screen_coords(vo: VertexOutput, pos: vec2<f32>) -> vec2<f32> {
    let ratio = f32(_std_uniform.window_size.x)/f32(_std_uniform.window_size.y);
    let p = vo.texcoords - pos;
    return vec2<f32>(p.x*ratio, p.y);
}

fn window_size() -> vec2<u32> {
    return _std_uniform.window_size;
}

fn mouse_pos() -> vec2<f32> {
    var mouse_pos = vec2<f32>(_std_uniform.mouse_pos.xy)/vec2<f32>(_std_uniform.window_size.xy);
    mouse_pos.y = 1. - mouse_pos.y;
    return mouse_pos;
}

fn toggle_mouse_pos() -> vec2<f32> {
    var mouse_pos = vec2<f32>(_std_uniform.toggle_mouse_pos.xy)/vec2<f32>(_std_uniform.window_size.xy);
    mouse_pos.y = 1. - mouse_pos.y;
    return mouse_pos;
}

fn time() -> f32 {
    return _std_uniform.time;
}

fn hex_to_rgba(a: u32) -> vec4<f32> {
    return vec4<f32>(vec4<u32>(
        extractBits(a, 24u, 8u), 
        extractBits(a, 16u, 8u),
        extractBits(a, 8u, 8u), 
        extractBits(a, 0u, 8u))) / 255.0;
}
